use super::*;
use crate::models::{AiModel, Provider, SearchOutcome, SearchQuery, SearchRecord};
use axum::body::to_bytes;
use std::time::Instant;

fn failure(message: &str) -> SearchOutcome {
    SearchOutcome {
        code: 0,
        question: None,
        answer: None,
        source: "none".into(),
        similarity: 0.0,
        explanation: None,
        model: String::new(),
        tokens: 0,
        duration_ms: 0,
        msg: Some(message.into()),
    }
}
fn validate(q: &SearchQuery) -> std::result::Result<(), &'static str> {
    if q.ask_title().trim().is_empty() {
        return Err("题目不能为空");
    }
    if q.ask_title().len() > 50_000 || q.ask_options().len() > 100_000 {
        return Err("题目或选项过长");
    }
    if !matches!(
        q.qtype.as_str(),
        "single" | "multiple" | "judgement" | "completion" | "" | "unknown"
    ) {
        return Err("题型不受支持");
    }
    if q.images.len() > 8 {
        return Err("最多支持 8 张图片");
    }
    use base64::Engine;
    for image in &q.images {
        let Some((prefix, payload)) = image.split_once(',') else {
            return Err("图片必须为 base64 data URL");
        };
        if !matches!(
            prefix,
            "data:image/png;base64"
                | "data:image/jpeg;base64"
                | "data:image/webp;base64"
                | "data:image/gif;base64"
        ) || payload.len() > 2 * 1024 * 1024
            || base64::engine::general_purpose::STANDARD
                .decode(payload)
                .is_err()
        {
            return Err("图片格式或大小无效");
        }
    }
    Ok(())
}
pub(super) async fn answer(
    State(app): State<Arc<App>>,
    request: Request,
) -> Result<Json<SearchOutcome>> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| v.len() <= 128)
        .ok_or_else(ApiError::unauthorized)?
        .to_owned();
    let user = app.store.token_user(&token)?;
    let tenant = app.tenant(user.id)?;
    // No unbounded queue: reject excess concurrent work before reserving daily quota.
    let _user_slot = tenant
        .slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("当前账号正在处理其他题目，请稍后重试"))?;
    let _global_slot = app
        .global
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("服务繁忙，请稍后重试"))?;
    app.store.reserve(user.id, &token)?;
    let started = Instant::now();
    let body = tokio::time::timeout(
        Duration::from_secs(15),
        to_bytes(request.into_body(), 8 * 1024 * 1024),
    )
    .await;
    let parsed = body
        .ok()
        .and_then(std::result::Result::ok)
        .and_then(|bytes| serde_json::from_slice::<SearchQuery>(&bytes).ok());
    let q = parsed.unwrap_or_default();
    let mut record = SearchRecord {
        title: q.ask_title().into(),
        options: q.ask_options().into(),
        qtype: q.qtype.clone(),
        image_count: q.images.len() as i64,
        ..Default::default()
    };
    let mut outcome = match validate(&q) {
        Err(error) => failure(error),
        Ok(()) => resolve(&app, &tenant, &user, &q, &mut record).await,
    };
    outcome.duration_ms = started.elapsed().as_millis() as i64;
    truncate(&mut record.title, 50_000);
    truncate(&mut record.options, 100_000);
    record.answer = outcome.answer.clone().unwrap_or_default();
    record.error = outcome.msg.clone().unwrap_or_default();
    record.status = if outcome.code == 1 {
        "success"
    } else {
        "failure"
    }
    .into();
    record.source = outcome.source.clone();
    record.duration_ms = outcome.duration_ms;
    record.total_tokens = outcome.tokens;
    record.explanation = outcome.explanation.clone().unwrap_or_default();
    if tenant.db.record(&record).is_err() {
        outcome.msg = Some("答题记录保存失败，请联系管理员".into());
    }
    Ok(Json(outcome))
}
fn truncate(value: &mut String, max: usize) {
    let mut end = max.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
}
async fn resolve(
    app: &App,
    tenant: &Tenant,
    user: &User,
    q: &SearchQuery,
    record: &mut SearchRecord,
) -> SearchOutcome {
    let title = q.ask_title().trim();
    let options = q.ask_options();
    if q.images.is_empty() {
        match tenant.db.find_local(title, options, &q.qtype, 1.0) {
            Ok(Some((hit, score))) => {
                let _ = tenant.db.bump_hits(hit.id);
                record.provider = hit.provider.clone();
                record.model = hit.model.clone();
                return SearchOutcome {
                    code: 1,
                    question: Some(title.into()),
                    answer: Some(hit.answer),
                    source: "cache".into(),
                    similarity: score,
                    explanation: None,
                    model: hit.model,
                    tokens: 0,
                    duration_ms: 0,
                    msg: None,
                };
            }
            Ok(None) => {}
            Err(_) => return failure("查询题库失败，请联系管理员"),
        }
    }
    let config = match app.store.model(user.id) {
        Ok(Some(c)) => c,
        _ => return failure("请先在网页端配置模型及 API Key"),
    };
    let preset = match app
        .config
        .providers
        .iter()
        .find(|p| p.id == config.provider_id)
    {
        Some(p) => p,
        None => return failure("配置的供应商已被管理员移除"),
    };
    let secret = match crypto::open(&app.config.master_key, user.id, &config.encrypted_key) {
        Ok(v) => v,
        Err(_) => return failure("无法读取模型凭据，请联系管理员"),
    };
    let provider = Provider {
        id: 0,
        name: preset.name.clone(),
        base_url: preset.base_url.clone(),
        api_key: secret.clone(),
        enabled: true,
        created_at: String::new(),
    };
    let model = AiModel {
        id: 0,
        provider_id: 0,
        model: config.model.clone(),
        display_name: config.model.clone(),
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        enabled: true,
        is_default: true,
    };
    record.provider = preset.name.clone();
    record.model = config.model.clone();
    let mut outcome = failure("供应商调用失败，请检查网页中的 Key、模型名称及账户额度");
    outcome.source = "ai".into();
    outcome.model = config.model;
    // Two internal format attempts in ai::ask are bounded by this overall hosted deadline.
    let response = tokio::time::timeout(
        Duration::from_secs(150),
        crate::ai::ask(
            &app.client,
            &provider,
            &model,
            &q.qtype,
            title,
            options,
            "",
            &q.images,
        ),
    )
    .await;
    match response {
        Ok(Ok(answer)) => {
            record.prompt_tokens = answer.prompt_tokens;
            record.completion_tokens = answer.completion_tokens;
            outcome.tokens = answer.total_tokens;
            if answer.answer.trim().is_empty() {
                outcome.msg = Some("模型未能确定答案，已记录此次失败请求".into());
                return outcome;
            }
            // Never pass echoed provider credentials through answers/explanations to OCS or records.
            if answer.answer.contains(&secret) || answer.explanation.contains(&secret) {
                outcome.msg = Some("模型回复包含敏感信息，已拦截".into());
                return outcome;
            }
            outcome.code = 1;
            outcome.question = Some(title.into());
            outcome.answer = Some(answer.answer.clone());
            outcome.explanation = Some(answer.explanation);
            outcome.msg = None;
            if q.images.is_empty()
                && tenant
                    .db
                    .cache_question(
                        title,
                        options,
                        &q.qtype,
                        &answer.answer,
                        &preset.name,
                        &model.model,
                    )
                    .is_err()
            {
                outcome.msg = Some("答案已返回，题库缓存保存失败".into());
            }
        }
        Ok(Err(_)) => {} // Upstream error bodies can echo credentials; do not persist or expose them.
        Err(_) => outcome.msg = Some("模型请求超时，已记录此次失败请求".into()),
    }
    outcome
}
