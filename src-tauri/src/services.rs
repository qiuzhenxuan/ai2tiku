//! UI-independent operations shared by the native desktop and compatibility entry point.
use crate::{models::*, state::SharedState};

pub fn save_provider(state: &SharedState, input: &ProviderInput) -> Result<i64, String> {
    if input.name.trim().is_empty() {
        return Err("供应商名称不能为空".into());
    }
    let url = reqwest::Url::parse(input.base_url.trim()).map_err(|_| "API 地址格式无效")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("API 地址须为 HTTP 或 HTTPS".into());
    }
    state.db.save_provider(input)
}

pub fn save_model(state: &SharedState, input: &ModelInput) -> Result<i64, String> {
    if input.model.trim().is_empty() {
        return Err("模型名不能为空".into());
    }
    if !state
        .db
        .list_providers()?
        .iter()
        .any(|p| p.id == input.provider_id)
    {
        return Err("请先选择有效的供应商".into());
    }
    state.db.save_model(input)
}

pub fn save_question(state: &SharedState, input: &QuestionInput) -> Result<i64, String> {
    if input.title.trim().is_empty() {
        return Err("题干不能为空".into());
    }
    if input.answer.trim().is_empty() {
        return Err("答案不能为空".into());
    }
    state.db.save_question(input)
}

pub async fn sync_models(state: &SharedState, id: i64) -> Result<SyncModelsResult, String> {
    let provider = state
        .db
        .list_providers()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("供应商不存在")?;
    let models =
        crate::ai::fetch_models(&state.client(), &provider.base_url, &provider.api_key).await?;
    let added = state.db.upsert_models(id, &models)?;
    Ok(SyncModelsResult {
        provider: provider.name,
        total: models.len(),
        added,
        models,
    })
}

pub async fn latest_release(state: &SharedState) -> Result<OcsRelease, String> {
    let response = state
        .client()
        .get("https://api.github.com/repos/ocsjs/ocsjs/releases/latest")
        .header("User-Agent", "ai2tiku")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("访问 GitHub 失败：{e}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub 返回 {}", response.status()));
    }
    let value: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let version = value["tag_name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("未获取到版本号")?
        .to_string();
    let script_url = value["assets"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|a| a["name"].as_str() == Some("ocs.user.js"))
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or("当前版本没有可安装的 ocs.user.js 资源")?
        .to_string();
    Ok(OcsRelease {
        version,
        published_at: value["published_at"].as_str().unwrap_or_default().into(),
        script_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Db, state::AppState};
    use std::sync::Arc;
    fn state() -> SharedState {
        let path = std::env::temp_dir().join(format!(
            "ai2tiku-service-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Arc::new(AppState::new(Db::open(&path).unwrap()))
    }
    #[test]
    fn forms_reject_invalid_input_and_model_import_cannot_restore_deleted_provider() {
        let state = state();
        let mut provider = ProviderInput {
            id: None,
            name: "本地模型".into(),
            base_url: "not-a-url".into(),
            api_key: String::new(),
            enabled: Some(true),
        };
        assert!(save_provider(&state, &provider).is_err());
        assert!(state.db.list_providers().unwrap().is_empty());
        provider.base_url = "http://127.0.0.1:11434/v1".into();
        let id = save_provider(&state, &provider).unwrap();
        let model = ModelInput {
            id: None,
            provider_id: id,
            model: "local-model".into(),
            display_name: None,
            temperature: None,
            max_tokens: None,
            enabled: Some(true),
            is_default: Some(true),
        };
        let model_id = save_model(&state, &model).unwrap();
        assert!(model_id > 0);
        state.db.delete_provider(id).unwrap();
        assert!(save_model(&state, &model).is_err());
        assert!(state
            .db
            .upsert_models(id, &["late-network-response".into()])
            .is_err());
        assert!(state.db.list_models().unwrap().is_empty());
        assert!(save_question(
            &state,
            &QuestionInput {
                id: None,
                title: String::new(),
                options: None,
                qtype: None,
                answer: "A".into()
            }
        )
        .is_err());
    }
    #[tokio::test]
    async fn model_sync_uses_real_http_and_preserves_default_and_deduplication() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router=axum::Router::new().route("/v1/models",axum::routing::get(||async{axum::Json(serde_json::json!({"data":[{"id":"model-b"},{"id":"model-a"},{"id":"model-a"}]}))}));
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let state = state();
        let id = save_provider(
            &state,
            &ProviderInput {
                id: None,
                name: "测试供应商".into(),
                base_url: format!("http://127.0.0.1:{port}/v1"),
                api_key: String::new(),
                enabled: Some(true),
            },
        )
        .unwrap();
        let first = sync_models(&state, id).await.unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(first.added, 2);
        let second = sync_models(&state, id).await.unwrap();
        assert_eq!(second.added, 0);
        let (_, model) = state.db.active_model().unwrap().unwrap();
        assert!(model.is_default);
        assert_eq!(model.model, "model-a");
        server.abort();
    }
}
