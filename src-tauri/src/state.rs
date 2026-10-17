use crate::db::Db;
use crate::server::ServerHandle;
use std::sync::{Arc, Mutex};

/// 全局共享状态：HTTP 服务与 Tauri 命令都通过它访问同一个数据库
pub struct AppState {
    pub db: Arc<Db>,
    pub client: reqwest::Client,
    pub server: Mutex<ServerHandle>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db),
            client: crate::ai::build_client(),
            server: Mutex::new(ServerHandle::default()),
        }
    }
}
