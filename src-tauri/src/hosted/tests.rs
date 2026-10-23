use super::*;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use tower::ServiceExt;

struct TestDirectory(PathBuf);
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Fixture {
    app: Arc<App>,
    admin: String,
    user: String,
    user_id: i64,
    _directory: TestDirectory,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ai2tiku-hosted-tests-{}", crypto::random()));
        let app = App::open(Config {
            data_dir: dir.clone(),
            public_url: "http://localhost:4185".into(),
            bind: "127.0.0.1:0".into(),
            master_key: [9; 32],
            admin_email: "owner@example.com".into(),
            admin_password: "test-owner-password".into(),
            registration_open: false,
            providers: vec![ProviderPreset {
                id: "deepseek".into(),
                name: "DeepSeek".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                models: vec![],
            }],
        })
        .unwrap();
        let admin_id = app
            .store
            .credentials("owner@example.com")
            .unwrap()
            .unwrap()
            .0;
        let user = app
            .store
            .create_user(Some(admin_id), "test@example.com", "test-user-password", 2)
            .unwrap();
        let admin = app.store.new_session(admin_id).unwrap();
        let session = app.store.new_session(user.id).unwrap();
        Self {
            app,
            admin,
            user: session,
            user_id: user.id,
            _directory: TestDirectory(dir),
        }
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        session: Option<&str>,
        data: Value,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("origin", "http://localhost:4185")
            .header("content-type", "application/json");
        if let Some(session) = session {
            builder = builder.header("cookie", format!("ai2tiku_session={session}"));
        }
        let response = router(self.app.clone())
            .oneshot(builder.body(Body::from(data.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }
    async fn token(&self) -> String {
        let (status,_)=self.request("PUT","/api/console/model",Some(&self.user),json!({"providerId":"deepseek","model":"deepseek-chat","apiKey":"sk-not-a-real-key"})).await;
        assert_eq!(status, StatusCode::OK);
        let (_, token) = self
            .request(
                "POST",
                "/api/console/tokens",
                Some(&self.user),
                json!({"name":"test"}),
            )
            .await;
        assert_eq!(
            token["ocsConfig"][0]["headers"]["Authorization"],
            format!("Bearer {}", token["token"].as_str().unwrap())
        );
        assert_eq!(
            token["ocsConfig"][0]["data"]["images"]["handler"],
            "return (env) => env.images"
        );
        assert_eq!(
            token["ocsConfig"][0]["url"],
            "http://localhost:4185/api/search"
        );
        token["token"].as_str().unwrap().into()
    }
}

#[tokio::test]
async fn bearer_is_one_time_and_encrypted_model_is_not_leaked() {
    let f = Fixture::new();
    let token = f.token().await;
    let (_, list) = f
        .request("GET", "/api/console/tokens", Some(&f.user), json!({}))
        .await;
    assert!(!list.to_string().contains(&token));
    assert!(list["items"][0].get("token").is_none());
    let (_, model) = f
        .request("GET", "/api/console/model", Some(&f.user), json!({}))
        .await;
    assert_eq!(model["hasKey"], true);
    assert!(!model.to_string().contains("sk-not-a-real-key"));
    let encrypted = f.app.store.model(f.user_id).unwrap().unwrap().encrypted_key;
    assert!(!encrypted.contains("sk-not-a-real-key"));
    let conn = f.app.store.lock().unwrap();
    let hash: String = conn
        .query_row("SELECT hash FROM tokens", [], |r| r.get(0))
        .unwrap();
    assert_eq!(hash, crypto::hash(&token));
    assert_ne!(hash, token);
}
#[tokio::test]
async fn csrf_roles_and_disabled_account_are_enforced() {
    let f = Fixture::new();
    let token = f.token().await;
    let response = router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/auth/logout")
                .header("origin", "https://evil.example")
                .header("cookie", format!("ai2tiku_session={}", f.user))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let (status, _) = f
        .request("GET", "/api/console/admin/users", Some(&f.user), json!({}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = f
        .request(
            "PATCH",
            &format!("/api/console/admin/users/{}", f.user_id),
            Some(&f.admin),
            json!({"enabled":false}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(f.app.store.session(&f.user).is_err());
    assert!(f.app.store.token_user(&token).is_err());
    f.app
        .store
        .update_user(1, f.user_id, Some(true), None, None)
        .unwrap();
    assert!(f.app.store.token_user(&token).is_err());
}
#[tokio::test]
async fn tenants_cannot_read_or_revoke_each_others_data() {
    let f = Fixture::new();
    f.token().await;
    let tenant = f.app.tenant(f.user_id).unwrap();
    tenant
        .db
        .save_question(&crate::models::QuestionInput {
            id: None,
            title: "private-question".into(),
            answer: "私有答案".into(),
            options: None,
            qtype: Some("completion".into()),
        })
        .unwrap();
    let (_, own) = f
        .request("GET", "/api/console/questions", Some(&f.user), json!({}))
        .await;
    assert_eq!(own["total"], 1);
    let (_, other) = f
        .request("GET", "/api/console/questions", Some(&f.admin), json!({}))
        .await;
    assert_eq!(other["total"], 0);
    let (status, _) = f
        .request("DELETE", "/api/console/tokens/1", Some(&f.admin), json!({}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn invalid_requests_count_against_quota_and_failures_are_recorded() {
    let f = Fixture::new();
    let token = f.token().await;
    for _ in 0..2 {
        let response = router(f.app.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/search")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from("{bad-json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/search")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(f.app.store.user(f.user_id).unwrap().used_today, 2);
    assert_eq!(
        f.app
            .tenant(f.user_id)
            .unwrap()
            .db
            .list_records(1, 20)
            .unwrap()
            .total,
        2
    );
}
#[tokio::test]
async fn cache_search_never_uses_another_tenant_key_or_records() {
    let f = Fixture::new();
    let token = f.token().await;
    let tenant = f.app.tenant(f.user_id).unwrap();
    tenant
        .db
        .cache_question("计算 1+1", "A. 2\nB. 3", "single", "A", "test", "test")
        .unwrap();
    let response = router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/search")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({"title":"计算 1+1","type":"single","options":"A. 2\nB. 3"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(value["answer"], "A");
    assert_eq!(value["source"], "cache");
    assert_eq!(tenant.db.list_records(1, 20).unwrap().total, 1);
    assert_eq!(
        f.app
            .tenant(1)
            .unwrap()
            .db
            .list_records(1, 20)
            .unwrap()
            .total,
        0
    );
}
#[test]
fn quota_reservation_is_atomic_across_concurrent_requests() {
    let f = Fixture::new();
    let id = f.user_id;
    let token = f.app.store.create_token(id, "test").unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut workers = vec![];
    for _ in 0..12 {
        let a = f.app.clone();
        let t = token.clone();
        workers.push(std::thread::spawn(move || a.store.reserve(id, &t).is_ok()));
    }
    let count = workers
        .into_iter()
        .map(|w| usize::from(w.join().unwrap()))
        .sum::<usize>();
    assert_eq!(count, 2);
}
#[tokio::test]
async fn login_logout_and_session_expiry_work_over_http() {
    let f = Fixture::new();
    let (status, _) = f
        .request(
            "POST",
            "/api/console/auth/login",
            None,
            json!({"email":"test@example.com","password":"wrong-password-test"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let response = router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/auth/login")
                .header("origin", "http://localhost:4185")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email":"test@example.com","password":"test-user-password"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    let secret = cookie
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("ai2tiku_session=")
        .unwrap()
        .to_string();
    let (status, _) = f
        .request("GET", "/api/console/me", Some(&secret), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    f.request("POST", "/api/console/auth/logout", Some(&secret), json!({}))
        .await;
    let (status, _) = f
        .request("GET", "/api/console/me", Some(&secret), json!({}))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    f.app
        .store
        .lock()
        .unwrap()
        .execute("UPDATE sessions SET expires=0", [])
        .unwrap();
    assert!(f.app.store.session(&f.user).is_err());
}
#[tokio::test]
async fn revoked_tokens_return_ocs_compatible_errors() {
    let f = Fixture::new();
    let token = f.token().await;
    f.app.store.revoke_token(1, f.user_id, None).unwrap();
    let response = router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/search")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(value["code"], 0);
    assert!(value["msg"].as_str().unwrap().contains("令牌"));
}
#[tokio::test]
async fn registration_is_closed_and_secrets_are_absent_from_audit() {
    let f = Fixture::new();
    f.token().await;
    let (status, _) = f
        .request(
            "POST",
            "/api/console/auth/register",
            None,
            json!({"email":"other@example.com","password":"test-user-password"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, audit) = f
        .request("GET", "/api/console/admin/audit", Some(&f.admin), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let serialized = audit.to_string();
    assert!(!serialized.contains("sk-not-a-real-key"));
    assert!(!serialized.contains("test-user-password"));
    assert!(!serialized.contains(&f.user));
}
