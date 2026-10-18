use crate::db::Db;
use crate::server::ServerHandle;
use std::sync::{Arc, Mutex, RwLock};

/// 全局共享状态：HTTP 服务与 Tauri 命令都通过它访问同一个数据库
pub struct AppState {
    pub db: Arc<Db>,
    /// HTTP 客户端。代理设置变更时需要重建，所以放在 RwLock 里；
    /// reqwest::Client 内部是 Arc，取用时克隆一份很廉价。
    client: RwLock<reqwest::Client>,
    pub server: Mutex<ServerHandle>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(db: Db) -> Self {
        let proxy = db.settings().map(|s| s.proxy_url).unwrap_or_default();
        Self {
            db: Arc::new(db),
            client: RwLock::new(crate::ai::build_client(&proxy)),
            server: Mutex::new(ServerHandle::default()),
        }
    }

    pub fn client(&self) -> reqwest::Client {
        self.client.read().expect("client 锁中毒").clone()
    }

    /// 代理等设置变更后重建客户端
    pub fn rebuild_client(&self, proxy_url: &str) {
        *self.client.write().expect("client 锁中毒") = crate::ai::build_client(proxy_url);
    }
}
