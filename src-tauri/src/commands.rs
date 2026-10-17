use crate::models::*;
use crate::state::SharedState;
use serde_json::json;
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
    crate::ai::test_provider(&state.client, &provider).await
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
    let url = format!("{}/api/search", crate::server::address(settings.port));
    let config = json!([
        {
            "name": "ai2tiku (本地 AI 题库)",
            "homepage": crate::server::address(settings.port),
            "url": url,
            "method": "post",
            "contentType": "json",
            "type": "GM_xmlhttpRequest",
            "headers": { "Content-Type": "application/json" },
            "data": {
                "title": "${title}",
                "type": "${type}",
                "options": "${options}"
            },
            "handler": "return (res)=> res.code === 1 ? [res.question, res.answer] : [res.msg, undefined]"
        }
    ]);
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

/// 在软件内直接跑一遍搜题链路，方便验证配置是否生效
#[tauri::command]
pub async fn test_search(
    state: State<'_, SharedState>,
    title: String,
    qtype: Option<String>,
    options: Option<String>,
) -> R<SearchOutcome> {
    let outcome = crate::search::answer(
        state.inner(),
        &title,
        qtype.as_deref().unwrap_or("single"),
        options.as_deref().unwrap_or(""),
    )
    .await;
    Ok(outcome)
}
