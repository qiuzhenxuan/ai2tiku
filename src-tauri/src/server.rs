use crate::models::{ActiveModelInfo, DeploymentStatus, SearchQuery, ServerStatus, Settings};
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
    generation: u64,
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

/// 返回题库 JSON 内容；新版 OCS 需要粘贴内容，不接受此地址作为订阅链接。
async fn ocs_config(State(state): State<SharedState>) -> Json<Value> {
    let port = status(&state).port;
    Json(crate::ocs::config_value(port))
}

async fn status_handler(State(state): State<SharedState>) -> Json<ServerStatus> {
    Json(status(&state))
}

/// GET /api/search?title=...&type=...&options=...
async fn search_get(State(state): State<SharedState>, Query(q): Query<SearchQuery>) -> Json<Value> {
    let outcome = crate::search::answer(&state, &q).await;
    Json(to_ocs_payload(&outcome))
}

/// POST /api/search
/// body: {"title":"...","type":"single","options":"A. xx\nB. yy"}
/// 图片题时另带 images（base64 数组）/ suggestion_title / suggestion_options
async fn search_post(State(state): State<SharedState>, body: Bytes) -> Json<Value> {
    // 解析失败时要说清楚，不能静默当成「空题目」——那样只会白白丢一道题
    let q: SearchQuery = match serde_json::from_slice(&body) {
        Ok(q) => q,
        Err(e) => {
            return Json(json!({
                "code": 0,
                "source": "none",
                "durationMs": 0,
                "msg": format!("请求体解析失败：{}", e),
            }))
        }
    };
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
    let running = handle.running && handle.shutdown.as_ref().is_some_and(|tx| !tx.is_closed());
    let port = if running {
        handle.port
    } else {
        state
            .db
            .settings()
            .map(|s| s.port)
            .ok()
            .filter(|p| *p > 0)
            .unwrap_or(3100)
    };
    ServerStatus {
        running,
        port,
        address: address(port),
    }
}

pub fn stop(state: &SharedState) -> ServerStatus {
    let _operation = state.lifecycle.lock().unwrap();
    let mut handle = state.server.lock().unwrap();
    if let Some(tx) = handle.shutdown.take() {
        let _ = tx.send(());
    }
    handle.running = false;
    handle.generation = handle.generation.wrapping_add(1);
    drop(handle);
    status(state)
}

/// 候选服务已绑定端口、创建运行时和线程，等待配置提交后激活。
struct PreparedServer {
    shutdown: oneshot::Sender<()>,
    activate: oneshot::Sender<u64>,
    port: u16,
}

fn prepare(state: &SharedState, port: u16) -> Result<PreparedServer, String> {
    if port == 0 {
        return Err("监听端口必须在 1–65535 之间".into());
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("端口 {} 无法监听：{}", port, e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("设置非阻塞失败：{}", e))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .map_err(|e| format!("创建服务运行时失败：{e}"))?;
    let listener = {
        let _entered = runtime.enter();
        tokio::net::TcpListener::from_std(listener).map_err(|e| format!("创建监听失败：{e}"))?
    };
    let (tx, rx) = oneshot::channel();
    let (activate, activation) = oneshot::channel::<u64>();
    let shared = Arc::clone(state);
    std::thread::Builder::new()
        .name(format!("ocs-{port}"))
        .spawn(move || {
            let generation = runtime.block_on(async {
                let generation = activation.await.ok()?;
                let app = router(Arc::clone(&shared));
                if let Err(e) = axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = rx.await;
                    })
                    .await
                {
                    eprintln!("[ai2tiku] 服务异常退出：{}", e);
                }
                Some(generation)
            });
            // 旧线程的退出不能将新服务标成停止。
            if let Some(generation) = generation {
                let mut handle = shared.server.lock().unwrap();
                if handle.generation == generation {
                    handle.running = false;
                    handle.shutdown = None;
                }
            }
        })
        .map_err(|e| format!("创建服务线程失败：{e}"))?;
    Ok(PreparedServer {
        shutdown: tx,
        activate,
        port,
    })
}

fn install(state: &SharedState, prepared: PreparedServer) -> Result<(), String> {
    let mut handle = state.server.lock().unwrap();
    let generation = handle.generation.wrapping_add(1);
    prepared
        .activate
        .send(generation)
        .map_err(|_| "候选服务已退出，原服务保持运行".to_string())?;
    let old_shutdown = handle.shutdown.replace(prepared.shutdown);
    handle.generation = generation;
    handle.port = prepared.port;
    handle.running = true;
    drop(handle);
    if let Some(tx) = old_shutdown {
        let _ = tx.send(());
    }
    Ok(())
}

pub fn start(state: &SharedState, port: u16) -> Result<ServerStatus, String> {
    let _operation = state.lifecycle.lock().unwrap();
    let current = status(state);
    if current.running && current.port == port {
        return Ok(current);
    }
    install(state, prepare(state, port)?)?;
    Ok(status(state))
}

/// 新端口准备成功后才持久化并切换。失败时旧设置、旧监听保持可用。
pub fn apply_settings(state: &SharedState, settings: Settings) -> Result<Settings, String> {
    let _operation = state.lifecycle.lock().unwrap();
    if settings.port == 0 {
        return Err("监听端口必须在 1–65535 之间".into());
    }
    let before = state.db.settings()?;
    let current = status(state);
    let prepared = if current.running && current.port != settings.port {
        Some(prepare(state, settings.port)?)
    } else {
        None
    };
    state.db.save_settings(&settings)?;
    if let Some(prepared) = prepared {
        if let Err(error) = install(state, prepared) {
            state
                .db
                .save_settings(&before)
                .map_err(|rollback| format!("{error}；恢复配置失败：{rollback}"))?;
            return Err(error);
        }
    }
    if before.proxy_url != settings.proxy_url {
        state.rebuild_client(&settings.proxy_url);
    }
    Ok(settings)
}

pub fn deployment_status(state: &SharedState) -> Result<DeploymentStatus, String> {
    let _operation = state.lifecycle.lock().unwrap();
    let settings = state.db.settings()?;
    let server = status(state);
    let config = serde_json::to_string_pretty(&crate::ocs::config_value(server.port))
        .map_err(|e| e.to_string())?;
    let ai_model = state
        .db
        .active_model()?
        .map(|(provider, model)| ActiveModelInfo {
            provider: provider.name,
            model: model.model,
        });
    Ok(DeploymentStatus {
        server,
        config,
        ai_model,
        ai_fallback: settings.ai_fallback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Db,
        models::{ModelInput, ProviderInput},
        state::AppState,
    };
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn state() -> SharedState {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai2tiku-service-test-{id}.db"));
        Arc::new(AppState::new(Db::open(&path).unwrap()))
    }

    fn free_port() -> u16 {
        TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn get(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        response
    }

    #[test]
    fn occupied_new_port_preserves_original_settings_and_live_service() {
        let state = state();
        let original_port = free_port();
        let mut settings = state.db.settings().unwrap();
        settings.port = original_port;
        apply_settings(&state, settings.clone()).unwrap();
        start(&state, original_port).unwrap();
        let blocker = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let new_port = blocker.local_addr().unwrap().port();
        let mut next = settings.clone();
        next.port = new_port;
        next.proxy_url = "http://127.0.0.1:9".into();
        assert!(apply_settings(&state, next).is_err());
        assert_eq!(state.db.settings().unwrap().port, original_port);
        assert_eq!(state.db.settings().unwrap().proxy_url, settings.proxy_url);
        assert!(status(&state).running);
        assert_eq!(status(&state).port, original_port);
        assert!(get(original_port, "/").contains("题库服务运行中"));
        drop(blocker);

        settings.port = new_port;
        apply_settings(&state, settings).unwrap();
        assert_eq!(state.db.settings().unwrap().port, new_port);
        assert_eq!(status(&state).port, new_port);
        assert!(get(new_port, "/ocs").contains(&format!("127.0.0.1:{new_port}/api/search")));
        // 给旧线程足够机会退出，核对它不会覆盖新服务状态。
        std::thread::sleep(Duration::from_millis(30));
        assert!(status(&state).running);
        assert!(get(new_port, "/").contains("题库服务运行中"));
        stop(&state);
    }

    #[test]
    fn failed_initial_start_reports_configured_port_not_zero() {
        let state = state();
        let blocker = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = blocker.local_addr().unwrap().port();
        let mut settings = state.db.settings().unwrap();
        settings.port = port;
        apply_settings(&state, settings).unwrap();
        assert!(start(&state, port).is_err());
        let snapshot = deployment_status(&state).unwrap();
        assert!(!snapshot.server.running);
        assert_eq!(snapshot.server.port, port);
        assert!(!snapshot.config.contains(":0/"));
        let config: Value = serde_json::from_str(&snapshot.config).unwrap();
        assert_eq!(
            config[0]["url"],
            format!("http://127.0.0.1:{port}/api/search")
        );
    }

    #[test]
    fn same_port_start_is_idempotent_and_zero_cannot_replace_live_service() {
        let state = state();
        let port = free_port();
        let mut settings = state.db.settings().unwrap();
        settings.port = port;
        apply_settings(&state, settings.clone()).unwrap();
        start(&state, port).unwrap();
        start(&state, port).unwrap();
        settings.port = 0;
        assert!(apply_settings(&state, settings).is_err());
        assert!(start(&state, 0).is_err());
        assert_eq!(state.db.settings().unwrap().port, port);
        assert!(get(port, "/").contains("题库服务运行中"));
        stop(&state);
        assert!(!status(&state).running);
        assert_eq!(status(&state).port, port);
    }

    #[test]
    fn deployment_uses_backend_model_selection_and_respects_disabled_provider() {
        let state = state();
        let mut provider = ProviderInput {
            id: None,
            name: "test".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            api_key: "test-only".into(),
            enabled: Some(false),
        };
        let id = state.db.save_provider(&provider).unwrap();
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
        assert!(deployment_status(&state).unwrap().ai_model.is_none());
        provider.id = Some(id);
        provider.enabled = Some(true);
        state.db.save_provider(&provider).unwrap();
        assert_eq!(
            deployment_status(&state).unwrap().ai_model.unwrap().model,
            "test-model"
        );
        let mut settings = state.db.settings().unwrap();
        settings.ai_fallback = false;
        apply_settings(&state, settings).unwrap();
        let snapshot = deployment_status(&state).unwrap();
        assert!(snapshot.ai_model.is_some());
        assert!(!snapshot.ai_fallback);
    }
}
