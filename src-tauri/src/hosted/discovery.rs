//! Key-scoped model discovery. Draft keys are used only for this metadata request.
use super::*;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiscoveryInput {
    provider_id: String,
    api_key: Option<String>,
}

pub(super) async fn models(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<DiscoveryInput>,
) -> Result<Json<Value>> {
    let user = app.user(&headers)?;
    // Resolve only the administrator-approved base URL, never a tenant-supplied URL.
    let provider = app
        .config
        .providers
        .iter()
        .find(|p| p.id == input.provider_id)
        .ok_or_else(|| ApiError::bad("只能选择管理员允许的供应商"))?;
    let key = match input.api_key.filter(|key| !key.is_empty()) {
        Some(key) => {
            if key.len() > 4096 || key.trim() != key || key.chars().any(char::is_control) {
                return Err(ApiError::bad("API Key 格式无效，请检查后重试"));
            }
            key
        }
        None => {
            let stored = app
                .bound_model(user.id)?
                .filter(|c| c.provider_id == input.provider_id)
                .ok_or_else(|| ApiError::bad("请先填写当前供应商的 API Key，再获取模型列表"))?;
            crypto::open(&app.config.master_key, user.id, &stored.encrypted_key)
                .map_err(|_| ApiError::internal())?
        }
    };
    // Share the existing per-account/global bounds without reserving answer quota.
    let tenant = app.tenant(user.id)?;
    let _user_slot = tenant
        .slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("当前账号正在处理其他请求，请稍后重新获取模型列表"))?;
    let _global_slot = app
        .global
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("服务繁忙，请稍后重新获取模型列表"))?;
    let names = tokio::time::timeout(
        DISCOVERY_TIMEOUT,
        fetch_models(&app.client, &provider.base_url, &key),
    )
    .await
    .map_err(|_| upstream("获取模型列表超时，请稍后重试或手动填写模型名称"))??;
    if names.iter().any(|name| name.contains(&key)) {
        return Err(upstream("供应商返回了无效的模型列表，请手动填写模型名称"));
    }
    Ok(Json(json!({"models": names})))
}

fn upstream(message: &str) -> ApiError {
    ApiError(StatusCode::BAD_GATEWAY, message.into())
}

async fn fetch_models(client: &reqwest::Client, base_url: &str, key: &str) -> Result<Vec<String>> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut response = client
        .get(url)
        .bearer_auth(key)
        .timeout(DISCOVERY_TIMEOUT)
        .send()
        .await
        // Never surface a reqwest URL/error chain or an upstream response body.
        .map_err(|_| upstream("无法连接供应商获取模型列表，请稍后重试或手动填写模型名称"))?;
    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => {
            return Err(ApiError::bad(
                "供应商拒绝了此 API Key，请检查 Key 是否有效及模型访问权限后重试",
            ));
        }
        429 => {
            return Err(ApiError::limited(
                "供应商模型列表请求过于频繁，请稍后重试或手动填写模型名称",
            ));
        }
        404 | 405 | 501 => {
            return Err(upstream(
                "此供应商未提供兼容的模型列表接口，请手动填写模型名称或接入点 ID",
            ));
        }
        _ => {
            return Err(upstream(
                "供应商暂时无法返回模型列表，请稍后重试或手动填写模型名称",
            ));
        }
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(upstream("供应商模型列表过大，请手动填写模型名称"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| upstream("模型列表读取失败，请重试或手动填写模型名称"))?
    {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
            return Err(upstream("供应商模型列表过大，请手动填写模型名称"));
        }
        body.extend_from_slice(&chunk);
    }
    parse_models(&body)
}

fn parse_models(body: &[u8]) -> Result<Vec<String>> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| upstream("供应商模型列表格式不正确，请重试或手动填写模型名称"))?;
    let entries = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("models").and_then(Value::as_array))
        .ok_or_else(|| upstream("供应商未返回兼容的模型列表，请手动填写模型名称"))?;
    let mut names = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = entry
            .get("id")
            .or_else(|| entry.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| upstream("供应商模型列表格式不正确，请手动填写模型名称"))?;
        // Match the model-name limits used by save_model, without truncating identifiers.
        if name.trim().is_empty() || name.len() > 200 || name.chars().any(char::is_control) {
            return Err(upstream("供应商返回了无效的模型名称，请手动填写模型名称"));
        }
        names.push(name.to_owned());
    }
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(upstream(
            "此 Key 未返回任何可用模型，请检查模型权限、重试或手动填写模型名称",
        ));
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_are_complete_sorted_and_deduplicated() {
        assert_eq!(
            parse_models(br#"{"data":[{"id":"org/reasoner-v2"},{"id":"chat"},{"id":"chat"}]}"#)
                .unwrap(),
            ["chat", "org/reasoner-v2"]
        );
        assert_eq!(
            parse_models(br#"{"models":[{"name":"custom/model"}]}"#).unwrap(),
            ["custom/model"]
        );
    }

    #[test]
    fn empty_invalid_or_oversized_identifiers_are_not_reported_as_available() {
        for body in [
            br#"{"data":[]}"#.as_slice(),
            br#"{"error":"secret-key"}"#,
            br#"{"data":[{"id":3}]}"#,
            br#"{"data":[{"id":""}]}"#,
            b"not-json",
        ] {
            let error = parse_models(body).unwrap_err();
            assert!(!error.1.contains("secret-key"));
        }
        let long = json!({"data":[{"id":"x".repeat(201)}]});
        assert!(parse_models(&serde_json::to_vec(&long).unwrap()).is_err());
    }
}
