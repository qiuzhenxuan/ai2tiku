use crate::state::SharedState;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticItem {
    pub name: String,
    pub status: String,
    pub detail: String,
    pub duration_ms: i64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub checked_at: String,
    pub items: Vec<DiagnosticItem>,
    pub model_called: bool,
    pub tokens: i64,
}

fn item(name: &str, status: &str, detail: impl Into<String>, started: Instant) -> DiagnosticItem {
    DiagnosticItem {
        name: name.into(),
        status: status.into(),
        detail: detail.into(),
        duration_ms: started.elapsed().as_millis() as i64,
    }
}

pub async fn run(state: &SharedState, include_model: bool) -> DiagnosticReport {
    let mut report = DiagnosticReport {
        checked_at: crate::db::now(),
        items: Vec::new(),
        model_called: false,
        tokens: 0,
    };
    let start = Instant::now();
    match state.db.stats() {
        Ok(stats) => report.items.push(item(
            "题库与数据库",
            "pass",
            format!(
                "可读取，共 {} 道题、{} 条请求记录",
                stats.question_total,
                stats.success_count + stats.failure_count
            ),
            start,
        )),
        Err(_) => report.items.push(item(
            "题库与数据库",
            "fail",
            "读取失败，请检查数据目录的访问权限和磁盘空间",
            start,
        )),
    }
    let start = Instant::now();
    let status = crate::server::status(state);
    if !status.running {
        report.items.push(item(
            "本地接口",
            "fail",
            format!("服务未启动，请先启动 {} 端口的题库服务", status.port),
            start,
        ));
    } else {
        // Local checks must work even when the configured external proxy is broken.
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build();
        let verified = async {
            let client = client.map_err(|e| e.to_string())?;
            let response = client
                .get(format!("{}/api/status", status.address))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err("本地接口返回错误状态".into());
            }
            let body: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
            if body["running"] != true || body["port"] != status.port {
                return Err("本地接口状态不一致".into());
            }
            let root = client
                .get(&status.address)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .text()
                .await
                .map_err(|e| e.to_string())?;
            if root != "ai2tiku OCS 题库服务运行中" {
                return Err("端口响应并非预期题库服务".into());
            }
            Ok::<_, String>(())
        }
        .await;
        report.items.push(match verified {
            Ok(()) => item(
                "本地接口",
                "pass",
                format!("{} 可访问，服务响应正常", status.address),
                start,
            ),
            Err(_) => item(
                "本地接口",
                "fail",
                "无法验证题库接口，请检查端口占用并尝试重启服务",
                start,
            ),
        });
    }
    let settings = match state.db.settings() {
        Ok(value) => value,
        Err(_) => {
            report
                .items
                .push(item("运行设置", "fail", "设置读取失败", Instant::now()));
            return report;
        }
    };
    report.items.push(item(
        "AI 兜底",
        if settings.ai_fallback { "pass" } else { "warn" },
        if settings.ai_fallback {
            "本地未命中时允许调用 AI"
        } else {
            "当前仅使用本地题库，未命中时不会调用 AI"
        },
        Instant::now(),
    ));
    report.items.push(item(
        "网络方式",
        if settings.proxy_url.trim().is_empty() {
            "pass"
        } else {
            "warn"
        },
        if settings.proxy_url.trim().is_empty() {
            "外部请求使用直连；默认诊断不访问模型服务"
        } else {
            "已配置代理；实际连通性需通过模型调用检查，默认诊断不访问外部服务"
        },
        Instant::now(),
    ));
    let active = match state.db.active_model() {
        Ok(value) => value,
        Err(_) => {
            report
                .items
                .push(item("默认模型", "fail", "模型配置读取失败", Instant::now()));
            return report;
        }
    };
    let Some((provider, model)) = active else {
        report.items.push(item(
            "默认模型",
            if settings.ai_fallback { "fail" } else { "warn" },
            "没有可用模型，请检查供应商与模型是否启用",
            Instant::now(),
        ));
        return report;
    };
    let valid_url = reqwest::Url::parse(&provider.base_url)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
    report.items.push(item(
        "默认模型",
        if valid_url { "pass" } else { "fail" },
        if valid_url {
            format!(
                "{} · {} 已启用（仅配置检查，尚未验证调用权限）",
                provider.name, model.model
            )
        } else {
            "供应商 API 地址格式无效".into()
        },
        Instant::now(),
    ));
    if !include_model || !valid_url {
        report.items.push(item(
            "模型实际调用",
            "skip",
            "未调用模型，不产生模型 Token；可单独执行调用检查",
            Instant::now(),
        ));
        return report;
    }
    let start = Instant::now();
    report.model_called = true;
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        crate::ai::ask(
            &state.client(),
            &provider,
            &model,
            "single",
            "1 + 1 = ?",
            "A. 1\nB. 2\nC. 3\nD. 4",
            "",
            &[],
        ),
    )
    .await;
    let (status, detail) = match response {
        Ok(Ok(answer)) => {
            report.tokens = answer.total_tokens;
            if answer.answer == "B" {
                (
                    "pass",
                    format!(
                        "模型调用和答案解析正常，本次返回用量 {} Token；诊断不写入题库和答题统计",
                        answer.total_tokens
                    ),
                )
            } else if answer.answer.is_empty() {
                ("fail", answer.explanation)
            } else {
                (
                    "warn",
                    format!("模型可访问，但诊断题答案不符合预期；返回 {}", answer.answer),
                )
            }
        }
        Ok(Err(error)) => ("fail", error),
        Err(_) => (
            "fail",
            "30 秒内未完成调用；上游可能仍产生用量，请检查供应商账单".into(),
        ),
    };
    let detail = if provider.api_key.is_empty() {
        detail
    } else {
        detail.replace(&provider.api_key, "[已隐藏]")
    };
    report
        .items
        .push(item("模型实际调用", status, detail, start));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{configure, mock, state};
    use std::sync::atomic::Ordering;
    #[tokio::test]
    async fn local_diagnosis_checks_real_service_without_model_calls_or_stats_changes() {
        let state = state();
        let (url, calls) = mock(serde_json::json!({}), axum::http::StatusCode::OK).await;
        configure(&state, &url);
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        crate::server::start(&state, port).unwrap();
        let report = run(&state, false).await;
        assert!(report
            .items
            .iter()
            .any(|item| item.name == "本地接口" && item.status == "pass"));
        assert!(!report.model_called);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(state.db.list_records(1, 10).unwrap().total, 0);
        crate::server::stop(&state);
    }
    #[tokio::test]
    async fn optional_model_check_reports_usage_without_caching_diagnostic_question() {
        let state = state();
        let (url,calls)=mock(serde_json::json!({"choices":[{"message":{"content":"B"},"finish_reason":"stop"}],"usage":{"prompt_tokens":4,"completion_tokens":1,"total_tokens":5}}),axum::http::StatusCode::OK).await;
        configure(&state, &url);
        let report = run(&state, true).await;
        assert!(report.model_called);
        assert_eq!(report.tokens, 5);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(report
            .items
            .iter()
            .any(|item| item.name == "模型实际调用" && item.status == "pass"));
        assert_eq!(state.db.stats().unwrap().question_total, 0);
        assert_eq!(state.db.list_records(1, 10).unwrap().total, 0);
    }
    #[tokio::test]
    async fn stopped_service_and_missing_model_have_actionable_failures() {
        let report = run(&state(), false).await;
        assert!(report
            .items
            .iter()
            .any(|item| item.name == "本地接口" && item.status == "fail"));
        assert!(report
            .items
            .iter()
            .any(|item| item.name == "默认模型" && item.status == "fail"));
    }
}
