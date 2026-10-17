use crate::models::*;
use crate::state::AppState;
use std::time::Instant;

/// 核心答题流程：本地题库优先，未命中再走 AI，并把结果写回缓存
pub async fn answer(state: &AppState, title: &str, qtype: &str, options: &str) -> SearchOutcome {
    let started = Instant::now();
    let title = title.trim();
    if title.is_empty() {
        return fail("题目为空，无法搜索", started);
    }

    let settings = match state.db.settings() {
        Ok(s) => s,
        Err(e) => return fail(&format!("读取设置失败：{}", e), started),
    };

    // 1. 本地题库匹配
    match state.db.find_local(title, settings.match_threshold) {
        Ok(Some((q, score))) => {
            let _ = state.db.bump_hits(q.id);
            let _ = state.db.add_record(title, &q.answer, "cache", "", "", 0, 0, elapsed(started));
            return SearchOutcome {
                code: 1,
                question: Some(q.title),
                answer: Some(q.answer),
                source: "cache".into(),
                similarity: (score * 1000.0).round() / 1000.0,
                explanation: None,
                model: if q.model.is_empty() {
                    "本地题库".into()
                } else {
                    q.model
                },
                tokens: 0,
                duration_ms: elapsed(started),
                msg: None,
            };
        }
        Ok(None) => {}
        Err(e) => return fail(&format!("查询本地题库失败：{}", e), started),
    }

    // 2. AI 兜底
    if !settings.ai_fallback {
        return fail("本地题库未命中（AI 兜底已关闭）", started);
    }
    let active = match state.db.active_model() {
        Ok(Some(v)) => v,
        Ok(None) => return fail("本地题库未命中，且没有启用中的 AI 模型，请先到「AI 供应商」页配置", started),
        Err(e) => return fail(&format!("读取 AI 配置失败：{}", e), started),
    };
    let (provider, model) = active;

    match crate::ai::ask(
        &state.client,
        &provider,
        &model,
        qtype,
        title,
        options,
        &settings.system_prompt,
    )
    .await
    {
        Ok(res) => {
            if res.answer.trim().is_empty() {
                let _ = state.db.add_record(
                    title,
                    "",
                    "ai",
                    &provider.name,
                    &model.model,
                    res.prompt_tokens,
                    res.completion_tokens,
                    elapsed(started),
                );
                return SearchOutcome {
                    code: 0,
                    question: None,
                    answer: None,
                    source: "ai".into(),
                    similarity: 0.0,
                    explanation: None,
                    model: model.model.clone(),
                    tokens: res.total_tokens,
                    duration_ms: elapsed(started),
                    msg: Some("AI 未能确定答案".into()),
                };
            }
            if settings.auto_cache {
                let _ = state.db.cache_question(
                    title,
                    options,
                    if qtype.is_empty() { "unknown" } else { qtype },
                    &res.answer,
                    &provider.name,
                    &model.model,
                );
            }
            let _ = state.db.add_record(
                title,
                &res.answer,
                "ai",
                &provider.name,
                &model.model,
                res.prompt_tokens,
                res.completion_tokens,
                elapsed(started),
            );
            SearchOutcome {
                code: 1,
                question: Some(title.to_string()),
                answer: Some(res.answer),
                source: "ai".into(),
                similarity: 0.0,
                explanation: if res.explanation.is_empty() {
                    None
                } else {
                    Some(res.explanation)
                },
                model: model.model,
                tokens: res.total_tokens,
                duration_ms: elapsed(started),
                msg: None,
            }
        }
        Err(e) => fail(&e, started),
    }
}

fn elapsed(started: Instant) -> i64 {
    started.elapsed().as_millis() as i64
}

fn fail(msg: &str, started: Instant) -> SearchOutcome {
    SearchOutcome {
        code: 0,
        question: None,
        answer: None,
        source: "none".into(),
        similarity: 0.0,
        explanation: None,
        model: String::new(),
        tokens: 0,
        duration_ms: elapsed(started),
        msg: Some(msg.to_string()),
    }
}
