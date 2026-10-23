use crate::{
    db::Db,
    models::*,
    state::{AppState, SharedState},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub fn state() -> SharedState {
    let dir = std::env::temp_dir().join(format!(
        "ai2tiku-v025-{}-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    Arc::new(AppState::new(Db::open(&dir.join("test.db")).unwrap()))
}
pub fn configure(state: &SharedState, url: &str) {
    let id = state
        .db
        .save_provider(&ProviderInput {
            id: None,
            name: "测试".into(),
            base_url: url.into(),
            api_key: "secret-test-key".into(),
            enabled: Some(true),
        })
        .unwrap();
    state
        .db
        .save_model(&ModelInput {
            id: None,
            provider_id: id,
            model: "test-model".into(),
            display_name: None,
            temperature: None,
            max_tokens: None,
            enabled: Some(true),
            is_default: Some(true),
        })
        .unwrap();
}
pub fn question(title: &str, answer: &str) -> QuestionInput {
    QuestionInput {
        id: None,
        title: title.into(),
        options: Some("A. 甲\nB. 乙".into()),
        qtype: Some("single".into()),
        answer: answer.into(),
    }
}
pub async fn mock(
    body: serde_json::Value,
    status: axum::http::StatusCode,
) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = count.clone();
    let router = axum::Router::new().route(
        "/chat/completions",
        axum::routing::post(move || {
            let body = body.clone();
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                (status, axum::Json(body))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, count)
}
