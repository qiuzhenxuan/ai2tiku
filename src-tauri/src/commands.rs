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

// ---------- 一键部署 OCS ----------

/// 拉取 OCS 脚本的最新 Release：安装按钮永远指向 GitHub 上的最新版
#[tauri::command]
pub async fn latest_ocs_release(state: State<'_, SharedState>) -> R<OcsRelease> {
    let res = state
        .client()
        .get("https://api.github.com/repos/ocsjs/ocsjs/releases/latest")
        .header("User-Agent", "ai2tiku")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("访问 GitHub 失败：{}", e))?;

    if !res.status().is_success() {
        return Err(format!("GitHub 返回 {}", res.status()));
    }

    let value: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let version = value["tag_name"].as_str().unwrap_or_default().to_string();
    let published_at = value["published_at"].as_str().unwrap_or_default().to_string();
    let script_url = value["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str() == Some("ocs.user.js"))
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "https://github.com/ocsjs/ocsjs/releases/download/{}/ocs.user.js",
                version
            )
        });

    Ok(OcsRelease {
        version,
        published_at,
        script_url,
    })
}

/// 检测本机浏览器环境（当前只支持 Edge，其它仅用于提示）
#[tauri::command]
pub fn browser_info() -> BrowserInfo {
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    let edge_installed = edge_paths.iter().any(|p| std::path::Path::new(p).exists())
        || reg_value(
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
            "",
        )
        .is_some();

    let prog_id = reg_value(
        r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
        "ProgId",
    )
    .unwrap_or_default();
    let edge_default = prog_id.starts_with("MSEdge");
    let default_browser = if edge_default {
        "Edge"
    } else if prog_id == "ChromeHTML" {
        "Chrome"
    } else if prog_id.starts_with("Firefox") {
        "Firefox"
    } else if prog_id.is_empty() {
        "未知"
    } else {
        "其他浏览器"
    }
    .to_string();

    BrowserInfo {
        edge_installed,
        edge_default,
        default_browser,
    }
}

/// 读注册表值。value 传空则读默认值；读不到返回 None
#[cfg(windows)]
fn reg_value(path: &str, value: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("reg");
    cmd.args(["query", path]);
    if !value.is_empty() {
        cmd.args(["/v", value]);
    }
    let out = cmd.creation_flags(0x0800_0000).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.contains("REG_SZ"))
        .and_then(|l| l.split("REG_SZ").nth(1))
        .map(|s| s.trim().to_string())
}

#[cfg(not(windows))]
fn reg_value(_path: &str, _value: &str) -> Option<String> {
    None
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
