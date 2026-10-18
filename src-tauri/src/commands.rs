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

#[tauri::command]
pub fn save_settings(state: State<'_, SharedState>, settings: Settings) -> R<Settings> {
    let before = state.db.settings()?;
    state.db.save_settings(&settings)?;
    // 代理变更后需要重建 HTTP 客户端才能生效
    if before.proxy_url != settings.proxy_url {
        state.inner().rebuild_client(&settings.proxy_url);
    }
    if before.port != settings.port {
        let running = crate::server::status(state.inner()).running;
        if running {
            crate::server::start(state.inner(), settings.port)
                .map_err(|e| format!("设置已保存，但服务重启失败：{}", e))?;
        }
    }
    state.db.settings()
}

#[tauri::command]
pub fn server_status(state: State<'_, SharedState>) -> R<ServerStatus> {
    Ok(crate::server::status(state.inner()))
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
    if input.name.trim().is_empty() {
        return Err("供应商名称不能为空".into());
    }
    if input.base_url.trim().is_empty() {
        return Err("base_url 不能为空".into());
    }
    state.db.save_provider(&input)
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
    if input.model.trim().is_empty() {
        return Err("模型名不能为空".into());
    }
    state.db.save_model(&input)
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
    if input.title.trim().is_empty() {
        return Err("题干不能为空".into());
    }
    if input.answer.trim().is_empty() {
        return Err("答案不能为空".into());
    }
    state.db.save_question(&input)
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
pub fn list_records(state: State<'_, SharedState>, page: i64, page_size: i64) -> R<Page<SearchRecord>> {
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

// ---------- OCS 题库配置 ----------

#[tauri::command]
pub fn build_ocs_config(state: State<'_, SharedState>) -> R<String> {
    let settings = state.db.settings()?;
    serde_json::to_string_pretty(&crate::ocs::config_value(settings.port))
        .map_err(|e| e.to_string())
}

/// 内置的常用供应商预设（前端选择后只需填 Key）
#[tauri::command]
pub fn list_provider_presets() -> Vec<ProviderPreset> {
    crate::presets::list()
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
pub async fn sync_provider_models(
    state: State<'_, SharedState>,
    id: i64,
) -> R<SyncModelsResult> {
    let provider = state
        .db
        .list_providers()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "供应商不存在".to_string())?;
    let client = state.client();
    let models = crate::ai::fetch_models(&client, &provider.base_url, &provider.api_key).await?;
    let added = state.db.upsert_models(id, &models)?;
    Ok(SyncModelsResult {
        provider: provider.name,
        total: models.len(),
        added,
        models,
    })
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
