use super::*;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use tower::ServiceExt;

struct TestDirectory(PathBuf);

#[test]
fn newly_created_account_snapshot_cannot_adopt_a_subsequent_password_reset() {
    let f = Fixture::new();
    let created = f
        .app
        .store
        .create_user(
            Some(1),
            "new-user@example.com",
            "creation-test-password",
            100,
        )
        .unwrap();
    f.app
        .store
        .update_user(1, created.id, None, None, Some("later-reset-password"))
        .unwrap();
    assert!(f
        .app
        .store
        .new_session(created.id, created.auth_version)
        .is_err());
    let fresh = f.app.store.user(created.id).unwrap();
    assert_ne!(fresh.auth_version, created.auth_version);
}

#[test]
fn full_source_cache_evicts_a_bucket_instead_of_blocking_new_sources() {
    let f = Fixture::new();
    for index in 0..4097_u32 {
        let source = IpAddr::V4(std::net::Ipv4Addr::from(0xc0000000_u32 + index));
        f.app.throttle(source, "test@example.com").unwrap();
    }
    assert_eq!(f.app.auth_window.lock().unwrap().sources.len(), 4096);
}

#[tokio::test]
async fn session_creation_atomically_rejects_credentials_verified_before_reset_or_disable() {
    let f = Fixture::new();
    let (_, _, old_version) = f
        .app
        .store
        .credentials("test@example.com")
        .unwrap()
        .unwrap();
    let user = f
        .app
        .store
        .update_user(1, f.user_id, None, None, Some("replacement-test-password"))
        .unwrap();
    assert_eq!(user.auth_version, old_version + 1);
    assert_eq!(
        f.app
            .store
            .new_session(f.user_id, old_version)
            .unwrap_err()
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(f.app.store.session(&f.user).is_err());
    let current = f
        .app
        .store
        .new_session(f.user_id, user.auth_version)
        .unwrap();
    assert!(f.app.store.session(&current).is_ok());
    f.app
        .store
        .update_user(1, f.user_id, Some(false), None, None)
        .unwrap();
    f.app
        .store
        .update_user(1, f.user_id, Some(true), None, None)
        .unwrap();
    assert!(
        f.app
            .store
            .new_session(f.user_id, user.auth_version)
            .is_err(),
        "Re-enabling must not revive an old verification"
    );
    let (status, _) = f
        .request(
            "POST",
            "/api/console/auth/login",
            None,
            json!({"email":"test@example.com","password":"replacement-test-password"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[test]
fn login_limits_are_isolated_by_source_and_ignore_untrusted_forwarded_headers() {
    let mut f = Fixture::new();
    let first: IpAddr = "192.0.2.1".parse().unwrap();
    let second: IpAddr = "192.0.2.2".parse().unwrap();
    for _ in 0..5 {
        f.app.throttle(first, "owner@example.com").unwrap();
    }
    assert!(f.app.throttle(first, "owner@example.com").is_err());
    f.app.throttle(second, "owner@example.com").unwrap();
    for i in 0..25 {
        f.app
            .throttle(first, &format!("other-{i}@example.com"))
            .unwrap();
    }
    assert!(f.app.throttle(first, "new@example.com").is_err());
    f.app.throttle(second, "new@example.com").unwrap();
    let mut request = Request::builder()
        .header("x-real-ip", "192.0.2.99")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::new(first, 1234)));
    assert_eq!(auth_source(&f.app, &request), first);
    Arc::get_mut(&mut f.app)
        .unwrap()
        .config
        .trusted_source_header = Some("x-real-ip".into());
    assert_eq!(
        auth_source(&f.app, &request),
        "192.0.2.99".parse::<IpAddr>().unwrap()
    );
    request.headers_mut().insert(
        "x-real-ip",
        HeaderValue::from_static("192.0.2.99, 192.0.2.98"),
    );
    assert_eq!(auth_source(&f.app, &request), first);
}

#[test]
fn custom_provider_identity_does_not_depend_on_position_and_normalizes_base_urls() {
    let first = provider_url("https://ONE.invalid:443/v1/").unwrap();
    let same = provider_url("https://one.invalid/v1").unwrap();
    let second = provider_url("https://two.invalid/v1").unwrap();
    assert_eq!(custom_provider_id(&first), custom_provider_id(&same));
    assert_ne!(custom_provider_id(&first), custom_provider_id(&second));
}

#[tokio::test]
async fn a_saved_key_cannot_follow_a_changed_provider_destination() {
    let mut f = Fixture::new();
    let token = f.token().await;
    let ciphertext = f.app.store.model(f.user_id).unwrap().unwrap().encrypted_key;
    Arc::get_mut(&mut f.app).unwrap().config.providers[0].base_url =
        "https://different.invalid/v1".into();
    assert!(f.app.bound_model(f.user_id).unwrap().is_none());
    let (status, model) = f
        .request("GET", "/api/console/model", Some(&f.user), json!(null))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(model["hasKey"], false);
    assert_eq!(
        f.request(
            "POST",
            "/api/console/models/discover",
            Some(&f.user),
            json!({"providerId":"deepseek"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "PUT",
            "/api/console/model",
            Some(&f.user),
            json!({"providerId":"deepseek","model":"chat"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/console/tokens",
            Some(&f.user),
            json!({"name":"blocked"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let response = router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/search")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({"title":"unique uncached title","type":"completion"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let data: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(data["code"], 0);
    assert_eq!(
        f.app.store.model(f.user_id).unwrap().unwrap().encrypted_key,
        ciphertext
    );
}

#[test]
fn legacy_builtin_keys_are_migrated_once_but_custom_position_keys_are_never_rebound() {
    let f = Fixture::new();
    {
        let conn = f.app.store.lock().unwrap();
        conn.execute("DELETE FROM metadata WHERE key='provider-binding-v1'", [])
            .unwrap();
        conn.execute("INSERT INTO models(user_id,provider_id,model,encrypted_key) VALUES(?1,'deepseek','chat','encrypted-test')", [f.user_id]).unwrap();
        conn.execute("INSERT INTO models(user_id,provider_id,model,encrypted_key) VALUES(1,'custom-1','chat','encrypted-test')", []).unwrap();
    }
    f.app.store.bind_legacy_builtin_models().unwrap();
    assert_eq!(
        f.app
            .store
            .model(f.user_id)
            .unwrap()
            .unwrap()
            .provider_base_url,
        "https://api.deepseek.com/v1"
    );
    assert_eq!(f.app.store.model(1).unwrap().unwrap().provider_base_url, "");
    assert!(f.app.bound_model(1).unwrap().is_none());
    f.app
        .store
        .lock()
        .unwrap()
        .execute(
            "UPDATE models SET provider_base_url='' WHERE user_id=?1",
            [f.user_id],
        )
        .unwrap();
    f.app.store.bind_legacy_builtin_models().unwrap();
    assert!(
        f.app.bound_model(f.user_id).unwrap().is_none(),
        "A later empty binding must not be silently inferred again"
    );
}

struct ModelMetadataServer(tokio::task::JoinHandle<()>, String);
impl Drop for ModelMetadataServer {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn metadata_server(
    status: StatusCode,
    body: String,
    expected_key: &'static str,
) -> ModelMetadataServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = Router::new().route(
        "/v1/models",
        get(move |headers: HeaderMap| {
            let body = body.clone();
            async move {
                assert_eq!(headers["authorization"], format!("Bearer {expected_key}"));
                (status, body)
            }
        }),
    );
    ModelMetadataServer(
        tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        }),
        base,
    )
}

#[tokio::test]
async fn discovery_uses_draft_or_same_provider_saved_key_without_changing_settings_or_quota() {
    let server = metadata_server(
        StatusCode::OK,
        json!({"data":[{"id":"reasoner"},{"id":"chat"},{"id":"chat"}]}).to_string(),
        "sk-not-a-real-key",
    )
    .await;
    let f = Fixture::with_provider_base(&server.1);
    let (status, data) = f
        .request(
            "POST",
            "/api/console/models/discover",
            Some(&f.user),
            json!({"providerId":"deepseek","apiKey":"sk-not-a-real-key"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["models"], json!(["chat", "reasoner"]));
    assert!(
        f.app.store.model(f.user_id).unwrap().is_none(),
        "Draft Key must not be saved"
    );
    let _token = f.token().await;
    let previous = f.app.store.model(f.user_id).unwrap().unwrap();
    let (status, data) = f
        .request(
            "POST",
            "/api/console/models/discover",
            Some(&f.user),
            json!({"providerId":"deepseek"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["models"], json!(["chat", "reasoner"]));
    assert_eq!(
        f.app.store.model(f.user_id).unwrap().unwrap().encrypted_key,
        previous.encrypted_key
    );
    assert_eq!(f.app.store.user(f.user_id).unwrap().used_today, 0);
}

#[tokio::test]
async fn discovery_rejects_missing_sessions_unapproved_providers_and_invalid_keys() {
    let f = Fixture::new();
    for (session, data, expected) in [
        (
            None,
            json!({"providerId":"deepseek","apiKey":"draft-key"}),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Some(f.user.as_str()),
            json!({"providerId":"http://127.0.0.1","apiKey":"draft-key"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Some(f.user.as_str()),
            json!({"providerId":"deepseek"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Some(f.user.as_str()),
            json!({"providerId":"deepseek","apiKey":"bad\nkey"}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        assert_eq!(
            f.request("POST", "/api/console/models/discover", session, data)
                .await
                .0,
            expected
        );
    }
}

#[tokio::test]
async fn discovery_never_echoes_upstream_errors_and_enforces_response_limit() {
    for (status, body) in [
        (StatusCode::UNAUTHORIZED, "sk-not-a-real-key".into()),
        (StatusCode::METHOD_NOT_ALLOWED, "sk-not-a-real-key".into()),
        (StatusCode::OK, "x".repeat(2 * 1024 * 1024 + 1)),
        (
            StatusCode::OK,
            json!({"data":[{"id":"sk-not-a-real-key"}]}).to_string(),
        ),
    ] {
        let server = metadata_server(status, body, "sk-not-a-real-key").await;
        let f = Fixture::with_provider_base(&server.1);
        let (status, data) = f
            .request(
                "POST",
                "/api/console/models/discover",
                Some(&f.user),
                json!({"providerId":"deepseek","apiKey":"sk-not-a-real-key"}),
            )
            .await;
        assert!(!status.is_success());
        assert!(!data.to_string().contains("sk-not-a-real-key"));
        assert!(f.app.store.model(f.user_id).unwrap().is_none());
    }
}

#[tokio::test]
async fn ocs_head_and_get_probes_are_public_read_only_and_do_not_reserve_quota() {
    let f = Fixture::new();
    let token = f.token().await;
    for method in ["HEAD", "GET"] {
        for bearer in [None, Some(token.as_str()), Some("invalid-token")] {
            let mut request = Request::builder()
                .method(method)
                .uri("/api/search?t=1792771022940&title=must-not-answer");
            if let Some(bearer) = bearer {
                request = request.header("authorization", format!("Bearer {bearer}"));
            }
            let response = router(f.app.clone())
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            if method == "HEAD" {
                assert!(body.is_empty());
            } else {
                let data: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(data["code"], 0);
                assert!(data["answer"].is_null());
                assert!(data["msg"].as_str().unwrap().contains("POST"));
            }
        }
    }
    assert_eq!(f.app.store.user(f.user_id).unwrap().used_today, 0);
    assert!(f.app.tenants.lock().unwrap().is_empty());
    let (status, _) = f
        .request("POST", "/api/search", None, json!({"title":""}))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unlimited_quota_accepts_requests_beyond_finite_caps_and_keeps_usage() {
    let f = Fixture::new();
    let token = f.token().await;
    let (status, user) = f
        .request(
            "PATCH",
            &format!("/api/console/admin/users/{}", f.user_id),
            Some(&f.admin),
            json!({"dailyLimit":0}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["dailyLimit"], 0);
    f.app
        .store
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO quota(user_id,day,count) VALUES(?1,?2,1000001)",
            rusqlite::params![f.user_id, store::day()],
        )
        .unwrap();
    for _ in 0..3 {
        f.app.store.reserve(f.user_id, &token).unwrap();
    }
    assert_eq!(f.app.store.user(f.user_id).unwrap().used_today, 1000004);
    let (status, _) = f
        .request(
            "PATCH",
            &format!("/api/console/admin/users/{}", f.user_id),
            Some(&f.admin),
            json!({"dailyLimit":-1}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(f.app.store.user(f.user_id).unwrap().daily_limit, 0);
    f.app.store.revoke_token(1, f.user_id, None).unwrap();
    assert_eq!(
        f.app.store.reserve(f.user_id, &token).unwrap_err().0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn changing_unlimited_to_finite_preserves_used_quota_and_disabled_checks() {
    let f = Fixture::new();
    let token = f.token().await;
    let path = format!("/api/console/admin/users/{}", f.user_id);
    let (status, _) = f
        .request("PATCH", &path, Some(&f.admin), json!({"dailyLimit":0}))
        .await;
    assert_eq!(status, StatusCode::OK);
    for _ in 0..5 {
        f.app.store.reserve(f.user_id, &token).unwrap();
    }
    let (status, _) = f
        .request("PATCH", &path, Some(&f.admin), json!({"dailyLimit":2}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        f.app.store.reserve(f.user_id, &token).unwrap_err().0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(f.app.store.user(f.user_id).unwrap().used_today, 5);
    f.request("PATCH", &path, Some(&f.admin), json!({"dailyLimit":0}))
        .await;
    f.app.store.reserve(f.user_id, &token).unwrap();
    assert_eq!(f.app.store.user(f.user_id).unwrap().used_today, 6);
    f.request("PATCH", &path, Some(&f.admin), json!({"enabled":false}))
        .await;
    assert_eq!(
        f.app.store.reserve(f.user_id, &token).unwrap_err().0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn only_admin_can_assign_or_create_unlimited_accounts() {
    let f = Fixture::new();
    let path = format!("/api/console/admin/users/{}", f.user_id);
    let (status, _) = f
        .request("PATCH", &path, Some(&f.user), json!({"dailyLimit":0}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(f.app.store.user(f.user_id).unwrap().daily_limit, 2);
    let (status, user) = f
        .request(
            "POST",
            "/api/console/admin/users",
            Some(&f.admin),
            json!({"email":"unlimited@example.com","password":"test-user-password","dailyLimit":0}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["dailyLimit"], 0);
    assert_eq!(user["role"], "user");
}
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
        Self::with_provider_base("https://api.deepseek.com/v1")
    }
    fn with_provider_base(base_url: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ai2tiku-hosted-tests-{}", crypto::random()));
        let app = App::open(Config {
            data_dir: dir.clone(),
            public_url: "http://localhost:4185".into(),
            bind: "127.0.0.1:0".into(),
            master_key: [9; 32],
            admin_email: "owner@example.com".into(),
            admin_password: "test-owner-password".into(),
            registration_open: false,
            trusted_source_header: None,
            providers: vec![ProviderPreset {
                id: "deepseek".into(),
                name: "DeepSeek".into(),
                base_url: base_url.into(),
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
        let admin = app.store.new_session(admin_id, 0).unwrap();
        let session = app.store.new_session(user.id, user.auth_version).unwrap();
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
