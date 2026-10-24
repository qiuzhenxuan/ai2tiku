use super::{crypto, ApiError};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};

pub type Result<T> = std::result::Result<T, ApiError>;
pub fn epoch() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn day() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

pub struct Store(Mutex<Connection>);
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub email: String,
    pub role: String,
    pub enabled: bool,
    pub daily_limit: i64,
    pub used_today: i64,
    #[serde(skip)]
    pub auth_version: i64,
}
impl User {
    pub fn admin(&self) -> Result<()> {
        if self.role == "admin" {
            Ok(())
        } else {
            Err(ApiError::forbidden("需要管理员权限"))
        }
    }
}
#[derive(Clone)]
pub struct ModelConfig {
    pub provider_id: String,
    pub provider_base_url: String,
    pub model: String,
    pub encrypted_key: String,
    pub temperature: f64,
    pub max_tokens: i64,
}

fn user(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        email: row.get(1)?,
        role: row.get(2)?,
        enabled: row.get(3)?,
        daily_limit: row.get(4)?,
        used_today: row.get(5)?,
        auth_version: row.get(6)?,
    })
}
const USERS:&str="SELECT u.id,u.email,u.role,u.enabled,u.daily_limit,coalesce((SELECT count FROM quota WHERE user_id=u.id AND day=date('now')),0),u.auth_version FROM users u";
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY,email TEXT NOT NULL UNIQUE,password TEXT NOT NULL,role TEXT NOT NULL CHECK(role IN ('admin','user')),enabled INTEGER NOT NULL DEFAULT 1,daily_limit INTEGER NOT NULL DEFAULT 1000,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
          CREATE TABLE IF NOT EXISTS models(user_id INTEGER PRIMARY KEY REFERENCES users(id),provider_id TEXT NOT NULL,model TEXT NOT NULL,encrypted_key TEXT NOT NULL,temperature REAL NOT NULL DEFAULT 0,max_tokens INTEGER NOT NULL DEFAULT 2048);
          CREATE TABLE IF NOT EXISTS sessions(hash TEXT PRIMARY KEY,user_id INTEGER NOT NULL REFERENCES users(id),expires INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS tokens(id INTEGER PRIMARY KEY,user_id INTEGER NOT NULL REFERENCES users(id),hash TEXT NOT NULL UNIQUE,prefix TEXT NOT NULL,name TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,last_used_at TEXT,revoked INTEGER NOT NULL DEFAULT 0);
          CREATE TABLE IF NOT EXISTS quota(user_id INTEGER NOT NULL REFERENCES users(id),day TEXT NOT NULL,count INTEGER NOT NULL,PRIMARY KEY(user_id,day));
          CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY,actor_id INTEGER,action TEXT NOT NULL,target_id INTEGER,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for (table, column, declaration) in [
            ("users", "auth_version", "INTEGER NOT NULL DEFAULT 0"),
            ("models", "provider_base_url", "TEXT NOT NULL DEFAULT ''"),
        ] {
            let exists = tx
                .prepare(&format!("PRAGMA table_info({table})"))?
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .iter()
                .any(|name| name == column);
            if !exists {
                tx.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
                    [],
                )?;
            }
        }
        tx.commit()?;
        Ok(Self(Mutex::new(conn)))
    }
    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.0.lock().map_err(|_| ApiError::internal())
    }
    pub fn bootstrap(&self, email: &str, password: &str) -> Result<()> {
        let mut conn = self.lock()?;
        let transaction =
            conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let count: i64 =
            transaction.query_row("SELECT count(*) FROM users WHERE role='admin'", [], |r| {
                r.get(0)
            })?;
        if count == 0 {
            let encoded = crypto::password(password).map_err(|_| ApiError::internal())?;
            transaction.execute(
                "INSERT INTO users(email,password,role) VALUES(?1,?2,'admin')",
                params![email, encoded],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn user(&self, id: i64) -> Result<User> {
        self.lock()?
            .query_row(&format!("{USERS} WHERE u.id=?1"), [id], user)
            .optional()?
            .ok_or_else(ApiError::unauthorized)
    }
    pub fn credentials(&self, email: &str) -> Result<Option<(i64, String, i64)>> {
        Ok(self
            .lock()?
            .query_row(
                "SELECT id,password,auth_version FROM users WHERE email=?1",
                [email],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }
    pub fn create_user(
        &self,
        actor: Option<i64>,
        email: &str,
        password: &str,
        limit: i64,
    ) -> Result<User> {
        let encoded = crypto::password(password).map_err(|_| ApiError::internal())?;
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        if tx.query_row("SELECT count(*) FROM users WHERE email=?1", [email], |r| {
            r.get::<_, i64>(0)
        })? > 0
        {
            return Err(ApiError::bad("邮箱已存在"));
        }
        tx.execute(
            "INSERT INTO users(email,password,role,daily_limit) VALUES(?1,?2,'user',?3)",
            params![email, encoded, limit],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO audit(actor_id,action,target_id) VALUES(?1,'user.create',?2)",
            params![actor, id],
        )?;
        tx.commit()?;
        drop(conn);
        self.user(id)
    }
    pub fn session(&self, secret: &str) -> Result<User> {
        let id = self
            .lock()?
            .query_row(
                "SELECT user_id FROM sessions WHERE hash=?1 AND expires>?2",
                params![crypto::hash(secret), epoch()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(ApiError::unauthorized)?;
        let u = self.user(id)?;
        if !u.enabled {
            return Err(ApiError::forbidden("账号已停用"));
        }
        Ok(u)
    }
    pub fn new_session(&self, id: i64, expected_version: i64) -> Result<String> {
        let token = crypto::random();
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Commit authentication only if the credentials verified before Argon2
        // work are still current. Reset/disable and insertion share one lock.
        let (enabled, version): (bool, i64) = tx
            .query_row(
                "SELECT enabled,auth_version FROM users WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(ApiError::unauthorized)?;
        if !enabled || version != expected_version {
            return Err(ApiError::unauthorized());
        }
        tx.execute("DELETE FROM sessions WHERE expires<=?1", [epoch()])?;
        // Bound live sessions per user while allowing several browsers.
        tx.execute("DELETE FROM sessions WHERE user_id=?1 AND hash NOT IN (SELECT hash FROM sessions WHERE user_id=?1 ORDER BY expires DESC LIMIT 9)",[id])?;
        tx.execute(
            "INSERT INTO sessions(hash,user_id,expires) VALUES(?1,?2,?3)",
            params![crypto::hash(&token), id, epoch() + 86400],
        )?;
        tx.execute(
            "INSERT INTO audit(actor_id,action,target_id) VALUES(?1,'session.login',?1)",
            [id],
        )?;
        tx.commit()?;
        Ok(token)
    }
    pub fn logout(&self, secret: &str) -> Result<()> {
        self.lock()?
            .execute("DELETE FROM sessions WHERE hash=?1", [crypto::hash(secret)])?;
        Ok(())
    }
    pub fn token_user(&self, secret: &str) -> Result<User> {
        let id = self
            .lock()?
            .query_row(
                "SELECT user_id FROM tokens WHERE hash=?1 AND revoked=0",
                [crypto::hash(secret)],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(ApiError::unauthorized)?;
        let u = self.user(id)?;
        if !u.enabled {
            return Err(ApiError::forbidden("账号已停用"));
        }
        Ok(u)
    }
    pub fn model(&self, id: i64) -> Result<Option<ModelConfig>> {
        Ok(self.lock()?.query_row("SELECT provider_id,model,encrypted_key,temperature,max_tokens,provider_base_url FROM models WHERE user_id=?1",[id],|r|Ok(ModelConfig{provider_id:r.get(0)?,model:r.get(1)?,encrypted_key:r.get(2)?,temperature:r.get(3)?,max_tokens:r.get(4)?,provider_base_url:r.get(5)?})).optional()?)
    }
    pub fn bind_legacy_builtin_models(&self) -> Result<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let done: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM metadata WHERE key='provider-binding-v1')",
            [],
            |r| r.get(0),
        )?;
        if !done {
            // Only the fixed built-in destinations from the previous release
            // have known historical identities. Never guess legacy custom-N.
            for preset in crate::presets::PRESETS
                .iter()
                .filter(|p| p.base_url.starts_with("https://"))
            {
                tx.execute("UPDATE models SET provider_base_url=?2 WHERE provider_id=?1 AND provider_base_url=''", params![preset.id,preset.base_url])?;
            }
            tx.execute(
                "INSERT INTO metadata(key,value) VALUES('provider-binding-v1','1')",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn save_model(&self, id: i64, c: &ModelConfig) -> Result<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO models(user_id,provider_id,model,encrypted_key,temperature,max_tokens,provider_base_url) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(user_id) DO UPDATE SET provider_id=excluded.provider_id,model=excluded.model,encrypted_key=excluded.encrypted_key,temperature=excluded.temperature,max_tokens=excluded.max_tokens,provider_base_url=excluded.provider_base_url",params![id,c.provider_id,c.model,c.encrypted_key,c.temperature,c.max_tokens,c.provider_base_url])?;
        tx.execute(
            "INSERT INTO audit(actor_id,action,target_id) VALUES(?1,'model.update',?1)",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn create_token(&self, id: i64, name: &str) -> Result<serde_json::Value> {
        let token = format!("at_{}", crypto::random());
        let prefix = token[..11].to_string();
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM tokens WHERE user_id=?1 AND revoked=0",
            [id],
            |r| r.get(0),
        )?;
        if count >= 10 {
            return Err(ApiError::bad("每个账号最多 10 个有效令牌，请先撤销旧令牌"));
        }
        tx.execute(
            "INSERT INTO tokens(user_id,hash,prefix,name) VALUES(?1,?2,?3,?4)",
            params![id, crypto::hash(&token), prefix, name],
        )?;
        let token_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO audit(actor_id,action,target_id) VALUES(?1,'token.create',?2)",
            params![id, token_id],
        )?;
        tx.commit()?;
        Ok(serde_json::json!({"id":token_id,"name":name,"prefix":prefix,"token":token}))
    }
    pub fn tokens(&self, id: i64) -> Result<serde_json::Value> {
        let conn = self.lock()?;
        let mut stmt=conn.prepare("SELECT id,name,prefix,created_at,last_used_at,revoked FROM tokens WHERE user_id=?1 ORDER BY id DESC LIMIT 500")?;
        let items=stmt.query_map([id],|r|Ok(serde_json::json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"prefix":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?,"lastUsedAt":r.get::<_,Option<String>>(4)?,"revoked":r.get::<_,bool>(5)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(serde_json::json!({"items":items}))
    }
    pub fn revoke_token(&self, actor: i64, user_id: i64, token_id: Option<i64>) -> Result<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE tokens SET revoked=1 WHERE user_id=?1 AND (?2 IS NULL OR id=?2)",
            params![user_id, token_id],
        )?;
        if changed == 0 && token_id.is_some() {
            return Err(ApiError::not_found());
        }
        tx.execute(
            "INSERT INTO audit(actor_id,action,target_id) VALUES(?1,'token.revoke',?2)",
            params![actor, user_id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn reserve(&self, id: i64, token: &str) -> Result<()> {
        let current_day = day();
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (enabled, limit): (bool, i64) = tx.query_row(
            "SELECT enabled,daily_limit FROM users WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if !enabled {
            return Err(ApiError::forbidden("账号已停用"));
        }
        let valid: i64 = tx.query_row(
            "SELECT count(*) FROM tokens WHERE user_id=?1 AND hash=?2 AND revoked=0",
            params![id, crypto::hash(token)],
            |r| r.get(0),
        )?;
        if valid == 0 {
            return Err(ApiError::unauthorized());
        }
        tx.execute(
            "INSERT INTO quota(user_id,day,count) VALUES(?1,?2,0) ON CONFLICT DO NOTHING",
            params![id, current_day],
        )?;
        // Zero removes the daily cap while keeping usage, authentication and
        // account-state checks in the same atomic transaction.
        let count = tx.execute(
            "UPDATE quota SET count=count+1 WHERE user_id=?1 AND day=?2 AND (?3=0 OR count<?3)",
            params![id, current_day, limit],
        )?;
        if count == 0 {
            return Err(ApiError::limited("今日请求额度已用完（UTC 00:00 重置）"));
        }
        tx.execute(
            "UPDATE tokens SET last_used_at=CURRENT_TIMESTAMP WHERE hash=?1",
            [crypto::hash(token)],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn users(&self) -> Result<serde_json::Value> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!("{USERS} ORDER BY u.id DESC LIMIT 10000"))?;
        let items = stmt
            .query_map([], user)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({"items":items}))
    }
    pub fn update_user(
        &self,
        actor: i64,
        id: i64,
        enabled: Option<bool>,
        limit: Option<i64>,
        password: Option<&str>,
    ) -> Result<User> {
        let encoded = password
            .map(crypto::password)
            .transpose()
            .map_err(|_| ApiError::internal())?;
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let role: String = tx
            .query_row("SELECT role FROM users WHERE id=?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(ApiError::not_found)?;
        if role == "admin" && enabled == Some(false) {
            return Err(ApiError::bad("不能停用管理员账号"));
        }
        tx.execute("UPDATE users SET enabled=coalesce(?2,enabled),daily_limit=coalesce(?3,daily_limit),password=coalesce(?4,password),auth_version=auth_version+?5 WHERE id=?1",params![id,enabled,limit,encoded,i64::from(password.is_some() || enabled == Some(false))])?;
        if password.is_some() || enabled == Some(false) {
            tx.execute("DELETE FROM sessions WHERE user_id=?1", [id])?;
        }
        if enabled == Some(false) {
            tx.execute("UPDATE tokens SET revoked=1 WHERE user_id=?1", [id])?;
        }
        tx.execute(
            "INSERT INTO audit(actor_id,action,target_id) VALUES(?1,'user.update',?2)",
            params![actor, id],
        )?;
        tx.commit()?;
        drop(conn);
        self.user(id)
    }
    pub fn audit(&self) -> Result<serde_json::Value> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id,actor_id,action,target_id,created_at FROM audit ORDER BY id DESC LIMIT 200",
        )?;
        let items=stmt.query_map([],|r|Ok(serde_json::json!({"id":r.get::<_,i64>(0)?,"actorId":r.get::<_,Option<i64>>(1)?,"action":r.get::<_,String>(2)?,"targetId":r.get::<_,Option<i64>>(3)?,"createdAt":r.get::<_,String>(4)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(serde_json::json!({"items":items}))
    }
}
