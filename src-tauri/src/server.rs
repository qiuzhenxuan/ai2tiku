use crate::models::{SearchQuery, ServerStatus};
use crate::state::SharedState;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

/// 运行时句柄：保留关闭通道，用于停止本地服务
#[derive(Default)]
pub struct ServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    port: u16,
    running: bool,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/api/status", get(status_handler))
        .route("/api/search", get(search_get).post(search_post))
        .route("/ocs", get(ocs_config))
        // 图片题的 base64 图片随请求体提交，axum 默认 2MB 上限会被撑爆，这里放宽到 32MB
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn root() -> &'static str {
    "ai2tiku OCS 题库服务运行中"
}

/// GET /ocs —— OCS「订阅链接」端点：直接返回题库配置 JSON。
/// OCS 的题库配置除了粘贴 JSON 字符串，也支持填一个会返回该 JSON 的链接。
async fn ocs_config(State(state): State<SharedState>) -> Json<Value> {
    let port = state.db.settings().map(|s| s.port).unwrap_or(3100);
    Json(crate::ocs::config_value(port))
}

async fn status_handler(State(state): State<SharedState>) -> Json<ServerStatus> {
    Json(status(&state))
}

/// GET /api/search?title=...&type=...&options=...
async fn search_get(
    State(state): State<SharedState>,
    Query(q): Query<SearchQuery>,
) -> Json<Value> {
    let outcome = crate::search::answer(&state, &q).await;
    Json(to_ocs_payload(&outcome))
}

/// POST /api/search
/// body: {"title":"...","type":"single","options":"A. xx\nB. yy"}
/// 图片题时另带 images（base64 数组）/ suggestion_title / suggestion_options
async fn search_post(State(state): State<SharedState>, body: Bytes) -> Json<Value> {
    let q: SearchQuery = serde_json::from_slice(&body).unwrap_or_default();
    let outcome = crate::search::answer(&state, &q).await;
    Json(to_ocs_payload(&outcome))
}

/// 统一的 OCS 响应格式：命中返回题目与答案，未命中返回提示信息
fn to_ocs_payload(outcome: &crate::models::SearchOutcome) -> Value {
    if outcome.code == 1 {
        json!({
            "code": 1,
            "question": outcome.question,
            "answer": outcome.answer,
            "source": outcome.source,
            "similarity": outcome.similarity,
            "model": outcome.model,
            "tokens": outcome.tokens,
            "durationMs": outcome.duration_ms,
            "explanation": outcome.explanation,
        })
    } else {
        json!({
            "code": 0,
            "msg": outcome.msg.clone().unwrap_or_else(|| "未找到答案".into()),
            "source": outcome.source,
            "durationMs": outcome.duration_ms,
        })
    }
}

pub fn address(port: u16) -> String {
    format!("http://127.0.0.1:{}", port)
}

pub fn status(state: &SharedState) -> ServerStatus {
    let handle = state.server.lock().unwrap();
    ServerStatus {
        running: handle.running,
        port: handle.port,
        address: address(handle.port),
    }
}

pub fn stop(state: &SharedState) -> ServerStatus {
    let mut handle = state.server.lock().unwrap();
    if let Some(tx) = handle.shutdown.take() {
        let _ = tx.send(());
    }
    handle.running = false;
    let port = handle.port;
    drop(handle);
    ServerStatus {
        running: false,
        port,
        address: address(port),
    }
}

/// 启动本地 HTTP 服务；先同步绑定端口，端口被占用时能立刻反馈给调用方
pub fn start(state: &SharedState, port: u16) -> Result<ServerStatus, String> {
    stop(state);

    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("端口 {} 无法监听：{}", port, e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("设置非阻塞失败：{}", e))?;

    let (tx, rx) = oneshot::channel::<()>();
    let shared = Arc::clone(state);

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[ai2tiku] 创建运行时失败：{}", e);
                return;
            }
        };
        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[ai2tiku] 监听失败：{}", e);
                    return;
                }
            };
            println!("[ai2tiku] OCS 题库服务已启动：{}", address(port));
            let app = router(shared);
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await
            {
                eprintln!("[ai2tiku] 服务异常退出：{}", e);
            }
        });
    });

    let mut handle = state.server.lock().unwrap();
    handle.shutdown = Some(tx);
    handle.port = port;
    handle.running = true;
    drop(handle);

    Ok(ServerStatus {
        running: true,
        port,
        address: address(port),
    })
}
