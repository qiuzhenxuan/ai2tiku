use crate::models::*;
use crate::presets::ProviderPreset;
use crate::state::SharedState;
use tauri::State;

type R<T> = Result<T, String>;

// ---------- 设置与服务 ----------

#[tauri::command]
pub fn get_settings(state: State<'_, SharedState>) -> R<Settings> {
    state.db.settings()
}

/// 内置默认提示词原文：设置页把它显示出来，用户才能看到并在此基础上修改
#[tauri::command]
pub fn default_system_prompt() -> String {
    crate::ai::default_system_prompt().to_string()
}

#[tauri::command]
pub fn save_settings(state: State<'_, SharedState>, settings: Settings) -> R<Settings> {
    crate::server::apply_settings(state.inner(), settings)
}

#[tauri::command]
pub fn server_status(state: State<'_, SharedState>) -> R<ServerStatus> {
    Ok(crate::server::status(state.inner()))
}

// ---------- 界面偏好 ----------

/// 读取落库的界面偏好（主题色等），没有就返回 null
#[tauri::command]
pub fn get_ui_pref(state: State<'_, SharedState>, key: String) -> R<Option<String>> {
    state.db.ui_pref(&key)
}

#[tauri::command]
pub fn set_ui_pref(state: State<'_, SharedState>, key: String, value: String) -> R<()> {
    state.db.set_ui_pref(&key, &value)
}

#[tauri::command]
pub fn start_server(state: State<'_, SharedState>) -> R<ServerStatus> {
    let port = state.db.settings()?.port;
    crate::server::start(state.inner(), port)
}

#[tauri::command]
pub fn stop_server(state: State<'_, SharedState>) -> R<ServerStatus> {
    Ok(crate::server::stop(state.inner()))
}

// ---------- AI 供应商 ----------

#[tauri::command]
pub fn list_providers(state: State<'_, SharedState>) -> R<Vec<Provider>> {
    state.db.list_providers()
}

#[tauri::command]
pub fn save_provider(state: State<'_, SharedState>, input: ProviderInput) -> R<i64> {
    crate::services::save_provider(state.inner(), &input)
}

#[tauri::command]
pub fn delete_provider(state: State<'_, SharedState>, id: i64) -> R<()> {
    state.db.delete_provider(id)
}

#[tauri::command]
pub async fn test_provider(state: State<'_, SharedState>, id: i64) -> R<String> {
    let provider = state
        .db
        .list_providers()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "供应商不存在".to_string())?;
    let client = state.client();
    crate::ai::test_provider(&client, &provider).await
}

// ---------- 模型 ----------

#[tauri::command]
pub fn list_models(state: State<'_, SharedState>) -> R<Vec<AiModel>> {
    state.db.list_models()
}

#[tauri::command]
pub fn save_model(state: State<'_, SharedState>, input: ModelInput) -> R<i64> {
    crate::services::save_model(state.inner(), &input)
}

#[tauri::command]
pub fn delete_model(state: State<'_, SharedState>, id: i64) -> R<()> {
    state.db.delete_model(id)
}

// ---------- 题库 ----------

#[tauri::command]
pub fn list_questions(
    state: State<'_, SharedState>,
    keyword: Option<String>,
    source: Option<String>,
    page: i64,
    page_size: i64,
) -> R<Page<Question>> {
    state
        .db
        .list_questions(keyword, source, page.max(1), page_size.clamp(1, 200))
}

#[tauri::command]
pub fn save_question(state: State<'_, SharedState>, input: QuestionInput) -> R<i64> {
    crate::services::save_question(state.inner(), &input)
}

#[tauri::command]
pub fn delete_question(state: State<'_, SharedState>, id: i64) -> R<()> {
    state.db.delete_question(id)
}

#[tauri::command]
pub fn clear_questions(state: State<'_, SharedState>) -> R<()> {
    state.db.clear_questions()
}

// ---------- 记录与统计 ----------

#[tauri::command]
pub fn list_records(
    state: State<'_, SharedState>,
    page: i64,
    page_size: i64,
) -> R<Page<SearchRecord>> {
    state.db.list_records(page.max(1), page_size.clamp(1, 200))
}

#[tauri::command]
pub fn clear_records(state: State<'_, SharedState>) -> R<()> {
    state.db.clear_records()
}

#[tauri::command]
pub fn get_stats(state: State<'_, SharedState>) -> R<Stats> {
    state.db.stats()
}

#[tauri::command]
pub fn data_revision(state: State<'_, SharedState>) -> u64 {
    state.db.revision()
}

#[tauri::command]
pub async fn export_question_file(
    app: tauri::AppHandle,
    state: State<'_, SharedState>,
) -> R<Option<String>> {
    let content = export_questions_json(state).await?;
    crate::transfer::save_dialog(app, content).await
}
#[tauri::command]
pub async fn export_questions_json(state: State<'_, SharedState>) -> R<String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || shared.db.export_questions())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn pick_question_file(app: tauri::AppHandle) -> R<Option<(String, String)>> {
    crate::transfer::open_dialog(app).await
}
#[tauri::command]
pub async fn preview_question_import(
    state: State<'_, SharedState>,
    content: String,
) -> R<crate::transfer::ImportPreview> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || shared.db.preview_questions(&content))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn apply_question_import(
    state: State<'_, SharedState>,
    content: String,
    mode: String,
    snapshot: String,
) -> R<crate::transfer::ImportResult> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        shared.db.import_questions(&content, &mode, &snapshot)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn run_diagnostics(
    state: State<'_, SharedState>,
    include_model: bool,
) -> R<crate::diagnostics::DiagnosticReport> {
    Ok(crate::diagnostics::run(state.inner(), include_model).await)
}

// ---------- OCS 题库配置 ----------

#[tauri::command]
pub fn build_ocs_config(state: State<'_, SharedState>) -> R<String> {
    Ok(crate::server::deployment_status(state.inner())?.config)
}

#[tauri::command]
pub fn deployment_status(state: State<'_, SharedState>) -> R<DeploymentStatus> {
    crate::server::deployment_status(state.inner())
}

#[tauri::command]
pub fn open_in_edge(url: String) -> R<()> {
    crate::browser::open_url(&url)
}

/// 内置的常用供应商预设（前端选择后只需填 Key）
#[tauri::command]
pub fn list_provider_presets() -> Vec<ProviderPreset> {
    crate::presets::list()
}

// ---------- 一键部署 OCS ----------

/// 拉取 OCS 脚本的最新 Release：安装按钮永远指向 GitHub 上的最新版
#[tauri::command]
pub async fn latest_ocs_release(state: State<'_, SharedState>) -> R<OcsRelease> {
    crate::services::latest_release(state.inner()).await
}

/// 检测本机浏览器环境（当前只支持 Edge，其它仅用于提示）
#[tauri::command]
pub fn browser_info() -> BrowserInfo {
    crate::browser::info()
}

/// 用 base_url + api_key 试探供应商的可选模型列表
#[tauri::command]
pub async fn fetch_models(
    state: State<'_, SharedState>,
    base_url: String,
    api_key: String,
) -> R<Vec<String>> {
    let client = state.client();
    crate::ai::fetch_models(&client, &base_url, &api_key).await
}

/// 拉取某供应商的模型并自动入库；新库第一次会顺便设好默认模型
#[tauri::command]
pub async fn sync_provider_models(state: State<'_, SharedState>, id: i64) -> R<SyncModelsResult> {
    crate::services::sync_models(state.inner(), id).await
}

/// 在软件内直接跑一遍搜题链路，方便验证配置是否生效
#[tauri::command]
pub async fn test_search(
    state: State<'_, SharedState>,
    title: String,
    qtype: Option<String>,
    options: Option<String>,
) -> R<SearchOutcome> {
    let query = SearchQuery {
        title,
        qtype: qtype.unwrap_or_else(|| "single".into()),
        options: options.unwrap_or_default(),
        ..Default::default()
    };
    Ok(crate::search::answer(state.inner(), &query).await)
}
