use crate::models::*;
use crate::state::AppState;
use std::time::Instant;

#[derive(Default)]
struct Trace {
    provider: String,
    model: String,
    secret: String,
    prompt_tokens: i64,
    completion_tokens: i64,
}

/// Every completed request, including early failures, passes through one writer.
pub async fn answer(state: &AppState, q: &SearchQuery) -> SearchOutcome {
    let started = Instant::now();
    let mut trace = Trace::default();
    let mut outcome = resolve(state, q, &mut trace, started).await;
    if let Some(message) = outcome.msg.as_mut() {
        if !trace.secret.is_empty() {
            *message = message.replace(&trace.secret, "[已隐藏]");
        }
    }
    let record = SearchRecord {
        title: q.ask_title().trim().into(),
        options: q.ask_options().into(),
        qtype: q.qtype.clone(),
        answer: outcome.answer.clone().unwrap_or_default(),
        source: outcome.source.clone(),
        provider: trace.provider,
        model: trace.model,
        prompt_tokens: trace.prompt_tokens,
        completion_tokens: trace.completion_tokens,
        total_tokens: outcome.tokens,
        duration_ms: elapsed(started),
        status: if outcome.code == 1 {
            "success"
        } else {
            "failure"
        }
        .into(),
        error: outcome.msg.clone().unwrap_or_default(),
        explanation: outcome.explanation.clone().unwrap_or_default(),
        image_count: q.images.len() as i64,
        ..Default::default()
    };
    if let Err(error) = state.db.record(&record) {
        eprintln!("[ai2tiku] 保存答题记录失败：{error}");
        let message = format!("答题记录保存失败：{error}");
        outcome.msg = Some(match outcome.msg {
            Some(previous) => format!("{previous}；{message}"),
            None => message,
        });
    }
    outcome.duration_ms = elapsed(started);
    outcome
}

async fn resolve(
    state: &AppState,
    q: &SearchQuery,
    trace: &mut Trace,
    started: Instant,
) -> SearchOutcome {
    let title = q.ask_title().trim();
    let options = q.ask_options();
    if title.is_empty() {
        return fail("题目为空，无法搜索", started);
    }
    let settings = match state.db.settings() {
        Ok(settings) => settings,
        Err(error) => return fail(&format!("读取设置失败：{error}"), started),
    };
    // Image placeholders cannot safely identify a question without its image.
    if q.images.is_empty() {
        match state
            .db
            .find_local(title, options, &q.qtype, settings.match_threshold)
        {
            Ok(Some((hit, score))) => {
                let _ = state.db.bump_hits(hit.id);
                trace.model = hit.model.clone();
                trace.provider = hit.provider;
                return SearchOutcome {
                    code: 1,
                    question: Some(hit.title),
                    answer: Some(hit.answer),
                    source: "cache".into(),
                    similarity: (score * 1000.0).round() / 1000.0,
                    explanation: None,
                    model: if hit.model.is_empty() {
                        "本地题库".into()
                    } else {
                        hit.model
                    },
                    tokens: 0,
                    duration_ms: elapsed(started),
                    msg: None,
                };
            }
            Ok(None) => {}
            Err(error) => return fail(&format!("查询本地题库失败：{error}"), started),
        }
    }
    if !settings.ai_fallback {
        return fail("本地题库未命中（AI 兜底已关闭）", started);
    }
    let (provider, model) = match state.db.active_model() {
        Ok(Some(active)) => active,
        Ok(None) => {
            return fail(
                "本地题库未命中，且没有启用中的 AI 模型，请先到「AI 供应商」页配置",
                started,
            )
        }
        Err(error) => return fail(&format!("读取 AI 配置失败：{error}"), started),
    };
    trace.provider = provider.name.clone();
    trace.model = model.model.clone();
    trace.secret = provider.api_key.clone();
    let response = crate::ai::ask(
        &state.client(),
        &provider,
        &model,
        &q.qtype,
        title,
        options,
        &settings.system_prompt,
        &q.images,
    )
    .await;
    let mut result = fail("AI 请求失败", started);
    result.source = "ai".into();
    result.model = model.model.clone();
    match response {
        Err(error) => result.msg = Some(error),
        Ok(response) => {
            trace.prompt_tokens = response.prompt_tokens;
            trace.completion_tokens = response.completion_tokens;
            result.tokens = response.total_tokens;
            if response.answer.trim().is_empty() {
                result.msg = Some(if response.explanation.is_empty() {
                    "AI 未能确定答案".into()
                } else {
                    response.explanation
                });
            } else {
                result.code = 1;
                result.question = Some(title.into());
                result.answer = Some(response.answer.clone());
                result.msg = None;
                if !response.explanation.is_empty() {
                    result.explanation = Some(response.explanation);
                }
                if settings.auto_cache && q.images.is_empty() {
                    if let Err(error) = state.db.cache_question(
                        title,
                        options,
                        if q.qtype.is_empty() {
                            "unknown"
                        } else {
                            &q.qtype
                        },
                        &response.answer,
                        &provider.name,
                        &model.model,
                    ) {
                        result.msg = Some(format!("答案已返回，但题库缓存保存失败：{error}"));
                    }
                }
            }
        }
    }
    result
}

/// Malformed requests are recorded without storing their raw body or image data.
pub fn invalid_request(state: &AppState, message: &str) -> SearchOutcome {
    let mut result = fail(message, Instant::now());
    if let Err(error) = state.db.record(&SearchRecord {
        title: "[请求格式错误]".into(),
        status: "failure".into(),
        source: "none".into(),
        error: message.into(),
        qtype: "unknown".into(),
        ..Default::default()
    }) {
        result.msg = Some(format!("{message}；答题记录保存失败：{error}"));
    }
    result
}

fn elapsed(started: Instant) -> i64 {
    started.elapsed().as_millis() as i64
}
fn fail(message: &str, started: Instant) -> SearchOutcome {
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
        msg: Some(message.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{configure, mock, question, state};
    use axum::http::StatusCode;
    use serde_json::json;
    fn query() -> SearchQuery {
        SearchQuery {
            title: "测试题".into(),
            qtype: "single".into(),
            options: "A. 甲\nB. 乙".into(),
            ..Default::default()
        }
    }

    #[test]
    fn legacy_record_schema_migrates_idempotently_without_losing_history() {
        let state = state();
        let path = state.db.path.clone();
        drop(state);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("DROP TABLE records; CREATE TABLE records(id INTEGER PRIMARY KEY,title TEXT NOT NULL,answer TEXT NOT NULL,source TEXT NOT NULL,provider TEXT NOT NULL DEFAULT '',model TEXT NOT NULL DEFAULT '',prompt_tokens INTEGER DEFAULT 0,completion_tokens INTEGER DEFAULT 0,total_tokens INTEGER DEFAULT 0,duration_ms INTEGER DEFAULT 0,created_at TEXT NOT NULL); INSERT INTO records(id,title,answer,source,created_at) VALUES(1,'旧成功','A','ai','2026-10-23'),(2,'旧失败','','ai','2026-10-23');").unwrap();
        drop(conn);
        let db = crate::db::Db::open(&path).unwrap();
        let rows = db.list_records(1, 10).unwrap();
        assert_eq!(rows.total, 2);
        assert_eq!(rows.items[0].status, "failure");
        assert!(!rows.items[0].error.is_empty());
        assert_eq!(rows.items[1].status, "success");
        drop(db);
        let db = crate::db::Db::open(&path).unwrap();
        assert_eq!(
            (
                db.stats().unwrap().success_count,
                db.stats().unwrap().failure_count
            ),
            (1, 1)
        );
    }

    #[tokio::test]
    async fn failed_record_write_is_visible_without_discarding_valid_answer() {
        let state = state();
        state.db.save_question(&question("测试题", "A")).unwrap();
        state.db.lock().unwrap().execute_batch("CREATE TRIGGER reject_record BEFORE INSERT ON records BEGIN SELECT RAISE(ABORT,'write rejected'); END;").unwrap();
        let result = answer(&state, &query()).await;
        assert_eq!(result.code, 1);
        assert_eq!(result.answer.as_deref(), Some("A"));
        assert!(result.msg.unwrap().contains("记录保存失败"));
    }
    #[tokio::test]
    async fn every_early_failure_is_recorded_once_with_full_question() {
        let state = state();
        let q = query();
        assert_eq!(answer(&state, &SearchQuery::default()).await.code, 0);
        assert_eq!(answer(&state, &q).await.code, 0);
        let mut settings = state.db.settings().unwrap();
        settings.ai_fallback = false;
        state.db.save_settings(&settings).unwrap();
        assert_eq!(answer(&state, &q).await.code, 0);
        let rows = state.db.list_records(1, 10).unwrap();
        assert_eq!(rows.total, 3);
        assert!(rows
            .items
            .iter()
            .all(|r| r.status == "failure" && !r.error.is_empty()));
        assert_eq!(rows.items[0].options, q.options);
        assert_eq!(rows.items[0].qtype, "single");
        let stats = state.db.stats().unwrap();
        assert_eq!(
            (stats.success_count, stats.failure_count, stats.ai_calls),
            (0, 3, 0)
        );
    }
    #[tokio::test]
    async fn network_and_http_failures_retain_model_but_redact_api_key() {
        let state = state();
        let (url, _) = mock(
            json!({"error":"secret-test-key invalid"}),
            StatusCode::UNAUTHORIZED,
        )
        .await;
        configure(&state, &url);
        let result = answer(&state, &query()).await;
        assert_eq!(result.code, 0);
        assert!(!result.msg.unwrap().contains("secret-test-key"));
        let record = &state.db.list_records(1, 10).unwrap().items[0];
        assert_eq!(
            (&*record.source, &*record.model, &*record.status),
            ("ai", "test-model", "failure")
        );
        assert!(!record.error.contains("secret-test-key"));
        let state = crate::test_support::state();
        configure(&state, "http://127.0.0.1:1");
        answer(&state, &query()).await;
        assert_eq!(state.db.list_records(1, 10).unwrap().total, 1);
    }
    #[tokio::test]
    async fn invalid_ai_answers_keep_usage_and_cannot_enter_cache() {
        let state = state();
        let (url,calls)=mock(json!({"choices":[{"message":{"content":"无法判断，A和B都有可能"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}),StatusCode::OK).await;
        configure(&state, &url);
        let result = answer(&state, &query()).await;
        assert_eq!((result.code, result.tokens), (0, 30));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        let rows = state.db.list_records(1, 10).unwrap();
        assert_eq!(rows.total, 1);
        assert_eq!(rows.items[0].total_tokens, 30);
        assert_eq!(state.db.stats().unwrap().question_total, 0);
    }
    #[tokio::test]
    async fn successes_and_cache_hits_have_one_record_and_correct_statistics() {
        let state = state();
        state.db.save_question(&question("测试题", "A")).unwrap();
        let revision = state.db.revision();
        assert_eq!(answer(&state, &query()).await.code, 1);
        assert!(state.db.revision() > revision);
        let stats = state.db.stats().unwrap();
        assert_eq!(
            (stats.success_count, stats.failure_count, stats.cache_hits),
            (1, 0, 1)
        );
        assert_eq!(stats.success_rate, 1.0);
        invalid_request(&state, "请求无效");
        let stats = state.db.stats().unwrap();
        assert_eq!(stats.hit_rate, 0.5);
        assert_eq!(stats.success_rate, 0.5);
    }
    #[tokio::test]
    async fn malformed_http_body_is_recorded_without_storing_raw_secrets() {
        let state = state();
        let router = crate::server::router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let response: serde_json::Value = reqwest::Client::new()
            .post(format!("http://{address}/api/search"))
            .body("secret-api-key malformed JSON")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["code"], 0);
        let rows = state.db.list_records(1, 10).unwrap();
        assert_eq!(rows.total, 1);
        assert!(!rows.items[0].error.contains("secret-api-key"));
    }
}
