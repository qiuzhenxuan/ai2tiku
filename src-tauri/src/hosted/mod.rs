//! Opt-in hosted deployment. Desktop routes and plaintext provider storage are never exposed.
mod crypto;
mod discovery;
mod input;
mod search;
mod store;
#[cfg(test)]
mod tests;

use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use store::{ModelConfig, Store, User};
use tokio::sync::Semaphore;

#[derive(Debug)]
pub(crate) struct ApiError(StatusCode, String);
impl ApiError {
    fn bad(message: &str) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }
    fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "请登录或提供有效的题库访问令牌".into(),
        )
    }
    fn forbidden(message: &str) -> Self {
        Self(StatusCode::FORBIDDEN, message.into())
    }
    fn limited(message: &str) -> Self {
        Self(StatusCode::TOO_MANY_REQUESTS, message.into())
    }
    fn internal() -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "服务暂不可用，请联系管理员".into(),
        )
    }
    fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "未找到对应资源".into())
    }
}
impl From<rusqlite::Error> for ApiError {
    fn from(_: rusqlite::Error) -> Self {
        Self::internal()
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"code":0,"error":self.1,"msg":self.1}))).into_response()
    }
}
type Result<T> = std::result::Result<T, ApiError>;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    id: String,
    name: String,
    base_url: String,
    models: Vec<String>,
}
pub struct Config {
    pub data_dir: PathBuf,
    pub public_url: String,
    pub bind: String,
    pub master_key: [u8; 32],
    pub admin_email: String,
    pub admin_password: String,
    pub registration_open: bool,
    pub trusted_source_header: Option<String>,
    pub providers: Vec<ProviderPreset>,
}
fn env_secret(name: &str) -> std::result::Result<String, String> {
    if let Ok(path) = std::env::var(format!("{name}_FILE")) {
        return std::fs::read_to_string(path)
            .map(|s| s.trim().into())
            .map_err(|_| format!("Cannot read {name}_FILE"));
    }
    std::env::var(name).map_err(|_| format!("{name} or {name}_FILE is required"))
}
fn provider_url(value: &str) -> std::result::Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid provider HTTPS base URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Provider base URLs must be HTTPS without credentials/query/fragment".into());
    }
    // Arbitrary destinations are never accepted from tenants. Extra bases are administrator-controlled.
    Ok(url.to_string().trim_end_matches('/').to_string())
}
fn custom_provider_id(base_url: &str) -> String {
    format!("custom-{}", crypto::hash(base_url))
}
impl Config {
    pub fn from_env() -> std::result::Result<Self, String> {
        let public_url = std::env::var("PUBLIC_URL").map_err(|_| "PUBLIC_URL is required")?;
        let url = reqwest::Url::parse(&public_url).map_err(|_| "Invalid PUBLIC_URL")?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "PUBLIC_URL must be an HTTP(S) origin with no path, credentials, query or fragment"
                    .into(),
            );
        }
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if url.scheme() != "https" && !local {
            return Err("PUBLIC_URL requires HTTPS except localhost development".into());
        }
        let mut providers = crate::presets::PRESETS
            .iter()
            .filter(|p| p.base_url.starts_with("https://"))
            .map(|p| ProviderPreset {
                id: p.id.into(),
                name: p.name.into(),
                base_url: p.base_url.into(),
                models: p.models.iter().map(|v| v.to_string()).collect(),
            })
            .collect::<Vec<_>>();
        if let Ok(extra) = std::env::var("ALLOWED_PROVIDER_URLS") {
            for (index, value) in extra
                .split(',')
                .filter(|v| !v.trim().is_empty())
                .enumerate()
            {
                let base_url = provider_url(value.trim())?;
                providers.push(ProviderPreset {
                    id: custom_provider_id(&base_url),
                    name: format!("管理员服务 {}", index + 1),
                    base_url,
                    models: vec![],
                });
            }
        }
        let email =
            valid_email(&std::env::var("ADMIN_EMAIL").map_err(|_| "ADMIN_EMAIL is required")?)
                .map_err(|e| e.1)?;
        let password = env_secret("ADMIN_PASSWORD")?;
        valid_password(&password).map_err(|e| e.1)?;
        let trusted_source_header = match std::env::var("HOSTED_TRUSTED_SOURCE_HEADER")
            .ok()
            .filter(|v| !v.is_empty())
        {
            Some(value) if value == "x-real-ip" => Some(value),
            Some(_) => return Err("HOSTED_TRUSTED_SOURCE_HEADER must be empty or x-real-ip".into()),
            None => None,
        };
        Ok(Self {
            data_dir: std::env::var_os("DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("hosted-data")),
            public_url: url.origin().ascii_serialization(),
            bind: std::env::var("HOSTED_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            master_key: crypto::key(&env_secret("MASTER_KEY")?)?,
            admin_email: email,
            admin_password: password,
            registration_open: std::env::var("REGISTRATION_OPEN").is_ok_and(|v| v == "true"),
            trusted_source_header,
            providers,
        })
    }
}
struct Tenant {
    db: Arc<crate::db::Db>,
    slots: Arc<Semaphore>,
}
struct AuthWindow {
    started: i64,
    sources: HashMap<IpAddr, SourceWindow>,
}
#[derive(Default)]
struct SourceWindow {
    last_seen: i64,
    count: u32,
    emails: HashMap<String, u32>,
}
#[derive(Clone, Copy)]
struct AuthSource(IpAddr);
pub struct App {
    config: Config,
    store: Store,
    client: reqwest::Client,
    tenants: Mutex<HashMap<i64, Arc<Tenant>>>,
    global: Arc<Semaphore>,
    password_slots: Arc<Semaphore>,
    auth_window: Mutex<AuthWindow>,
    dummy_password: String,
}
impl App {
    pub fn open(mut config: Config) -> std::result::Result<Arc<Self>, String> {
        std::fs::create_dir_all(config.data_dir.join("tenants"))
            .map_err(|_| "Cannot create DATA_DIR")?;
        let store = Store::open(&config.data_dir.join("hosted.sqlite3")).map_err(|e| e.1)?;
        {
            let conn = store.lock().map_err(|e| e.1)?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL)",
            )
            .map_err(|_| "Cannot initialize metadata")?;
            use rusqlite::OptionalExtension;
            let check: Option<String> = conn
                .query_row(
                    "SELECT value FROM metadata WHERE key='master-check'",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|_| "Cannot verify master key")?;
            if let Some(check) = check {
                if crypto::open(&config.master_key, 0, &check)? != "ai2tiku-hosted" {
                    return Err("Invalid MASTER_KEY".into());
                }
            } else {
                let check = crypto::seal(&config.master_key, 0, "ai2tiku-hosted")?;
                conn.execute("INSERT INTO metadata VALUES('master-check',?1)", [check])
                    .map_err(|_| "Cannot save master key check")?;
            }
        }
        store
            .bootstrap(&config.admin_email, &config.admin_password)
            .map_err(|e| e.1)?;
        store.bind_legacy_builtin_models().map_err(|e| e.1)?;
        config.admin_password.clear();
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| "Cannot build AI client")?;
        let dummy_password = crypto::password(&crypto::random())?;
        Ok(Arc::new(Self {
            config,
            store,
            client,
            tenants: Mutex::new(HashMap::new()),
            global: Arc::new(Semaphore::new(32)),
            password_slots: Arc::new(Semaphore::new(2)),
            auth_window: Mutex::new(AuthWindow {
                started: store::epoch(),
                sources: HashMap::new(),
            }),
            dummy_password,
        }))
    }
    fn tenant(&self, id: i64) -> Result<Arc<Tenant>> {
        let mut tenants = self.tenants.lock().map_err(|_| ApiError::internal())?;
        if let Some(t) = tenants.get(&id) {
            return Ok(t.clone());
        }
        if tenants.len() >= 128 {
            let removable: Vec<_> = tenants
                .iter()
                .filter(|(_, t)| Arc::strong_count(t) == 1)
                .map(|(id, _)| *id)
                .take(64)
                .collect();
            for id in removable {
                tenants.remove(&id);
            }
        }
        let db = crate::db::Db::open(
            &self
                .config
                .data_dir
                .join("tenants")
                .join(format!("{id}.sqlite3")),
        )
        .map_err(|_| ApiError::internal())?;
        let tenant = Arc::new(Tenant {
            db: Arc::new(db),
            slots: Arc::new(Semaphore::new(2)),
        });
        tenants.insert(id, tenant.clone());
        Ok(tenant)
    }
    fn user(&self, headers: &HeaderMap) -> Result<User> {
        self.store
            .session(session_cookie(self, headers).ok_or_else(ApiError::unauthorized)?)
    }
    fn bound_model(&self, id: i64) -> Result<Option<ModelConfig>> {
        Ok(self.store.model(id)?.filter(|model| {
            self.config.providers.iter().any(|provider| {
                provider.id == model.provider_id && provider.base_url == model.provider_base_url
            })
        }))
    }
    fn throttle(&self, source: IpAddr, email: &str) -> Result<()> {
        let mut window = self.auth_window.lock().map_err(|_| ApiError::internal())?;
        if store::epoch() - window.started >= 60 {
            *window = AuthWindow {
                started: store::epoch(),
                sources: HashMap::new(),
            };
        }
        if !window.sources.contains_key(&source) && window.sources.len() >= 4096 {
            // A bounded cache must not become a global login deny switch.
            if let Some(oldest) = window
                .sources
                .iter()
                .min_by_key(|(_, bucket)| bucket.last_seen)
                .map(|(ip, _)| *ip)
            {
                window.sources.remove(&oldest);
            }
        }
        let bucket = window.sources.entry(source).or_default();
        bucket.last_seen = store::epoch();
        if bucket.count >= 30 || bucket.emails.get(email).copied().unwrap_or(0) >= 5 {
            return Err(ApiError::limited(
                "当前来源登录或注册请求过于频繁，请一分钟后再试",
            ));
        }
        bucket.count += 1;
        *bucket.emails.entry(email.into()).or_default() += 1;
        Ok(())
    }
}
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { Json(json!({"ok":true})) }))
        .route("/api/console/bootstrap", get(bootstrap))
        .route("/api/console/auth/login", post(login))
        .route("/api/console/auth/register", post(register))
        .route("/api/console/auth/logout", post(logout))
        .route("/api/console/me", get(me))
        .route("/api/console/model", get(get_model).put(save_model))
        .route("/api/console/models/discover", post(discovery::models))
        .route("/api/console/tokens", get(tokens).post(create_token))
        .route("/api/console/tokens/{id}", delete(revoke_token))
        .route("/api/console/questions", get(questions))
        .route("/api/console/records", get(records))
        .route("/api/console/stats", get(stats))
        .route("/api/console/admin/users", get(users).post(create_user))
        .route("/api/console/admin/users/{id}", patch(update_user))
        .route(
            "/api/console/admin/users/{id}/tokens",
            get(admin_tokens).delete(admin_revoke_tokens),
        )
        .route("/api/console/admin/audit", get(audit))
        .route("/api/search", get(search::probe).post(search::answer))
        .fallback(|| async { ApiError::not_found() })
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(app.clone(), security))
        .with_state(app)
}
fn auth_source(app: &App, request: &Request) -> IpAddr {
    // Optional forwarding trust is operator-controlled and disabled by default.
    // The public proxy must overwrite this header; never use arbitrary XFF.
    if let Some(name) = &app.config.trusted_source_header {
        let mut values = request.headers().get_all(name).iter();
        if let Some(value) = values.next().filter(|_| values.next().is_none()) {
            if let Some(ip) = value.to_str().ok().and_then(|v| v.parse::<IpAddr>().ok()) {
                return ip;
            }
        }
    }
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
}
async fn security(State(app): State<Arc<App>>, mut request: Request, next: Next) -> Response {
    let source = auth_source(&app, &request);
    request.extensions_mut().insert(AuthSource(source));
    let mutation = matches!(
        *request.method(),
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::PATCH
            | axum::http::Method::DELETE
    );
    let mut response = if mutation
        && request.uri().path().starts_with("/api/console/")
        && request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            != Some(app.config.public_url.as_str())
    {
        ApiError::forbidden("请求来源不匹配，请使用配置的 PUBLIC_URL 打开网页").into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}
fn cookie_name(app: &App) -> &'static str {
    if app.config.public_url.starts_with("https://") {
        "__Host-ai2tiku_session"
    } else {
        "ai2tiku_session"
    }
}
fn session_cookie<'a>(app: &App, headers: &'a HeaderMap) -> Option<&'a str> {
    let prefix = format!("{}=", cookie_name(app));
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|c| c.strip_prefix(&prefix))
}
fn set_cookie(app: &App, value: &str, clear: bool) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let secure = if app.config.public_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "{}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{secure}",
        cookie_name(app),
        if clear { 0 } else { 86400 }
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("generated cookie"),
    );
    headers
}
fn valid_email(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() > 254
        || value.len() < 3
        || !value.is_ascii()
        || value.chars().any(char::is_whitespace)
        || value.split('@').count() != 2
        || value.starts_with('@')
        || value.ends_with('@')
    {
        Err(ApiError::bad("请输入有效邮箱"))
    } else {
        Ok(value)
    }
}
fn valid_password(value: &str) -> Result<()> {
    if !(12..=128).contains(&value.len()) {
        Err(ApiError::bad("密码长度须为 12–128 字节"))
    } else {
        Ok(())
    }
}
fn valid_limit(value: i64) -> Result<i64> {
    if !(0..=1_000_000).contains(&value) {
        Err(ApiError::bad("每日限额须为 0–1000000，0 表示无上限"))
    } else {
        Ok(value)
    }
}
async fn bootstrap(State(app): State<Arc<App>>) -> Json<Value> {
    Json(
        json!({"registrationOpen":app.config.registration_open,"providers":app.config.providers,"publicUrl":app.config.public_url}),
    )
}
#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}
async fn login(
    State(app): State<Arc<App>>,
    Extension(source): Extension<AuthSource>,
    Json(input): Json<Credentials>,
) -> Result<impl IntoResponse> {
    let email = valid_email(&input.email)?;
    valid_password(&input.password)?;
    app.throttle(source.0, &email)?;
    let _slot = app
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("登录处理中，请稍后重试"))?;
    let credentials = app.store.credentials(&email)?;
    let encoded = credentials
        .as_ref()
        .map(|v| v.1.clone())
        .unwrap_or_else(|| app.dummy_password.clone());
    let valid = tokio::task::spawn_blocking(move || crypto::verify(&input.password, &encoded))
        .await
        .map_err(|_| ApiError::internal())?;
    if !valid || credentials.is_none() {
        return Err(ApiError(StatusCode::UNAUTHORIZED, "邮箱或密码错误".into()));
    }
    let (id, _, version) = credentials.unwrap();
    let user = app.store.user(id)?;
    if !user.enabled {
        return Err(ApiError::forbidden("账号已停用"));
    }
    let secret = app.store.new_session(user.id, version)?;
    Ok((set_cookie(&app, &secret, false), Json(user)))
}
async fn register(
    State(app): State<Arc<App>>,
    Extension(source): Extension<AuthSource>,
    Json(input): Json<Credentials>,
) -> Result<impl IntoResponse> {
    if !app.config.registration_open {
        return Err(ApiError::forbidden("当前不开放自行注册，请联系管理员"));
    }
    let email = valid_email(&input.email)?;
    valid_password(&input.password)?;
    app.throttle(source.0, &email)?;
    let _slot = app
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("注册处理中，请稍后重试"))?;
    let a = app.clone();
    let user = tokio::task::spawn_blocking(move || {
        a.store.create_user(None, &email, &input.password, 100)
    })
    .await
    .map_err(|_| ApiError::internal())??;
    let secret = app.store.new_session(user.id, user.auth_version)?;
    Ok((set_cookie(&app, &secret, false), Json(user)))
}
async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<impl IntoResponse> {
    if let Some(value) = session_cookie(&app, &headers) {
        app.store.logout(value)?;
    }
    Ok((set_cookie(&app, "", true), Json(json!({"ok":true}))))
}
async fn me(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<User>> {
    Ok(Json(app.user(&headers)?))
}
async fn get_model(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = app.user(&headers)?;
    Ok(Json(match app.bound_model(user.id)? {
        Some(c) => {
            json!({"providerId":c.provider_id,"model":c.model,"hasKey":true,"temperature":c.temperature,"maxTokens":c.max_tokens})
        }
        None => json!({"providerId":"","model":"","hasKey":false,"temperature":0,"maxTokens":2048}),
    }))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelInput {
    provider_id: String,
    model: String,
    api_key: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<i64>,
}
async fn save_model(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<ModelInput>,
) -> Result<Json<Value>> {
    let user = app.user(&headers)?;
    let preset = app
        .config
        .providers
        .iter()
        .find(|p| p.id == input.provider_id)
        .ok_or_else(|| ApiError::bad("只能选择管理员允许的供应商"))?;
    if input.model.trim().is_empty()
        || input.model.len() > 200
        || input.model.chars().any(char::is_control)
    {
        return Err(ApiError::bad("模型名无效"));
    }
    let temperature = input.temperature.unwrap_or(0.0);
    let max_tokens = input.max_tokens.unwrap_or(2048);
    if !(0.0..=2.0).contains(&temperature) || !(64..=8192).contains(&max_tokens) {
        return Err(ApiError::bad("温度或最大输出长度超出允许范围"));
    }
    let encrypted_key = if let Some(key) = input.api_key.filter(|s| !s.is_empty()) {
        if key.len() > 4096 || key.trim() != key || key.chars().any(char::is_control) {
            return Err(ApiError::bad("API Key 格式无效"));
        }
        crypto::seal(&app.config.master_key, user.id, &key).map_err(|_| ApiError::internal())?
    } else {
        app.bound_model(user.id)?
            .filter(|c| c.provider_id == input.provider_id)
            .ok_or_else(|| ApiError::bad("首次配置或更换供应商时需要填写 API Key"))?
            .encrypted_key
    };
    app.store.save_model(
        user.id,
        &ModelConfig {
            provider_id: input.provider_id,
            provider_base_url: preset.base_url.clone(),
            model: input.model.trim().into(),
            encrypted_key,
            temperature,
            max_tokens,
        },
    )?;
    Ok(Json(json!({"ok":true})))
}
async fn tokens(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    let u = app.user(&headers)?;
    Ok(Json(app.store.tokens(u.id)?))
}
#[derive(Deserialize)]
struct TokenInput {
    name: String,
}
async fn create_token(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<TokenInput>,
) -> Result<Json<Value>> {
    let u = app.user(&headers)?;
    if input.name.trim().is_empty()
        || input.name.len() > 80
        || input.name.chars().any(char::is_control)
    {
        return Err(ApiError::bad("令牌名称须为 1–80 字节"));
    }
    if app.bound_model(u.id)?.is_none() {
        return Err(ApiError::bad("请先保存模型及 API Key"));
    }
    let mut token = app.store.create_token(u.id, input.name.trim())?;
    token["ocsConfig"] = ocs_config(&app.config.public_url, token["token"].as_str().unwrap());
    Ok(Json(token))
}
fn ocs_config(base: &str, token: &str) -> Value {
    json!([{"name":"ai2tiku 云端题库","homepage":base,"url":format!("{base}/api/search"),"method":"post","contentType":"json","responseType":"json","type":"GM_xmlhttpRequest","headers":{"Content-Type":"application/json","Authorization":format!("Bearer {token}")},"data":{"title":"${title}","type":"${type}","options":"${options}","images":{"handler":"return (env) => env.images"},"suggestion_title":"${suggestion_title}","suggestion_options":"${suggestion_options}"},"handler":"return (res)=> res.code === 1 ? [res.question, res.answer] : [res.msg, undefined]"}])
}
async fn revoke_token(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let u = app.user(&headers)?;
    app.store.revoke_token(u.id, u.id, Some(id))?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Paging {
    page: Option<i64>,
    page_size: Option<i64>,
    q: Option<String>,
}
impl Paging {
    fn values(&self) -> (i64, i64) {
        (
            self.page.unwrap_or(1).clamp(1, 1_000_000),
            self.page_size.unwrap_or(20).clamp(1, 100),
        )
    }
}
async fn questions(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(p): Query<Paging>,
) -> Result<Json<Value>> {
    let u = app.user(&headers)?;
    let (page, size) = p.values();
    let db = app.tenant(u.id)?;
    let items = db
        .db
        .list_questions(p.q, None, page, size)
        .map_err(|_| ApiError::internal())?;
    Ok(Json(
        serde_json::to_value(items).map_err(|_| ApiError::internal())?,
    ))
}
async fn records(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(p): Query<Paging>,
) -> Result<Json<Value>> {
    let u = app.user(&headers)?;
    let (page, size) = p.values();
    let db = app.tenant(u.id)?;
    Ok(Json(
        serde_json::to_value(
            db.db
                .list_hosted_records(page, size)
                .map_err(|_| ApiError::internal())?,
        )
        .map_err(|_| ApiError::internal())?,
    ))
}
async fn stats(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    let u = app.user(&headers)?;
    let db = app.tenant(u.id)?;
    Ok(Json(
        serde_json::to_value(db.db.stats().map_err(|_| ApiError::internal())?)
            .map_err(|_| ApiError::internal())?,
    ))
}
async fn users(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    app.user(&headers)?.admin()?;
    Ok(Json(app.store.users()?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewUser {
    email: String,
    password: String,
    daily_limit: Option<i64>,
}
async fn create_user(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<NewUser>,
) -> Result<Json<User>> {
    let actor = app.user(&headers)?;
    actor.admin()?;
    let email = valid_email(&input.email)?;
    valid_password(&input.password)?;
    let limit = valid_limit(input.daily_limit.unwrap_or(1000))?;
    let _slot = app
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("账号操作处理中"))?;
    let u = tokio::task::spawn_blocking(move || {
        app.store
            .create_user(Some(actor.id), &email, &input.password, limit)
    })
    .await
    .map_err(|_| ApiError::internal())??;
    Ok(Json(u))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserUpdate {
    enabled: Option<bool>,
    daily_limit: Option<i64>,
    password: Option<String>,
}
async fn update_user(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<UserUpdate>,
) -> Result<Json<User>> {
    let actor = app.user(&headers)?;
    actor.admin()?;
    if let Some(v) = input.daily_limit {
        valid_limit(v)?;
    }
    if let Some(ref p) = input.password {
        valid_password(p)?;
    }
    let _slot = app
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("账号操作处理中"))?;
    let u = tokio::task::spawn_blocking(move || {
        app.store.update_user(
            actor.id,
            id,
            input.enabled,
            input.daily_limit,
            input.password.as_deref(),
        )
    })
    .await
    .map_err(|_| ApiError::internal())??;
    Ok(Json(u))
}
async fn admin_tokens(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    app.user(&headers)?.admin()?;
    app.store.user(id)?;
    Ok(Json(app.store.tokens(id)?))
}
async fn admin_revoke_tokens(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let actor = app.user(&headers)?;
    actor.admin()?;
    app.store.user(id)?;
    app.store.revoke_token(actor.id, id, None)?;
    Ok(Json(json!({"ok":true})))
}
async fn audit(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>> {
    app.user(&headers)?.admin()?;
    Ok(Json(app.store.audit()?))
}

pub async fn run() -> std::result::Result<(), String> {
    let config = Config::from_env()?;
    let bind = config.bind.clone();
    let app = App::open(config)?;
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .map_err(|_| "Cannot bind HOSTED_BIND")?;
    eprintln!("ai2tiku hosted listening on {bind}");
    axum::serve(
        listener,
        router(app).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|_| "HTTP server failed".into())
}
