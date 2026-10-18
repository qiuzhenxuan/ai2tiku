use crate::models::*;
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_SYSTEM_PROMPT: &str = r#"你是一个严谨的答题助手。请根据题目和选项直接给出正确答案。
只输出一个 JSON 对象，不要输出 Markdown 代码块、注释或任何额外说明。格式如下：
{"answer": <答案>, "explanation": "<不超过 100 字的简要说明>"}

answer 的填写规则：
- 单选题、判断题：填选项字母，例如 "A"
- 多选题：填选项字母数组，例如 ["A","C"]（按字母顺序，不要混入其它字符）
- 填空题：只有一个空时填文本；有多个空时返回数组，按顺序对应每个空，例如 ["张三","李四"]
- 如果确实无法确定答案，answer 填 ""（空字符串）"#;

const TYPE_LABELS: &[(&str, &str)] = &[
    ("single", "单选题"),
    ("multiple", "多选题"),
    ("judgement", "判断题"),
    ("completion", "填空题"),
];

pub fn type_label(qtype: &str) -> String {
    TYPE_LABELS
        .iter()
        .find(|(k, _)| *k == qtype)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "未知题型".to_string())
}

pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

fn endpoint(base_url: &str, path: &str) -> String {
    // 用户从网页复制 base_url 时很容易带上空格或换行，这里统一清掉
    let base = base_url.trim().trim_end_matches('/');
    format!("{}/{}", base, path.trim_start_matches('/'))
}

/// 把 reqwest 错误的整条原因链拼出来。
/// reqwest::Error 的 Display 只有最外层那句话（例如 error sending request for url (...)),
/// 真正的根因（dns / 连接被拒 / 超时 / 证书 / 连接被重置）藏在 source 里。
fn describe(e: &reqwest::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        parts.push(s.to_string());
        source = s.source();
    }
    parts.join(" → ")
}

/// 去掉 Markdown 代码块围栏，尽力从回复中取出 JSON 对象
fn extract_json(content: &str) -> Option<Value> {
    let cleaned = content.trim();
    if let Ok(v) = serde_json::from_str::<Value>(cleaned) {
        return Some(v);
    }
    let unfenced = cleaned
        .trim_start_matches("```json")
        .trim_start_matches("```JSON")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(v) = serde_json::from_str::<Value>(unfenced) {
        return Some(v);
    }
    let start = unfenced.find('{')?;
    let end = unfenced.rfind('}')?;
    if end > start {
        return serde_json::from_str::<Value>(&unfenced[start..=end]).ok();
    }
    None
}

/// 把 AI 返回的 answer 字段转成 OCS 能识别的答案字符串。
/// OCS 官方文档明确要求：多个答案用「特殊符号」连接，而不是写成数组
/// （文档示例是 join('#')，FQA 里填空题多空也用 `张三#李四#王五`）。
/// 所以这里多选字母、填空多空一律用 '#' 拼接。
fn to_ocs_answer(v: Option<&Value>) -> String {
    match v {
        Some(Value::Array(items)) => {
            let list: Vec<String> = items
                .iter()
                .map(|x| match x {
                    Value::String(s) => s.trim().to_string(),
                    other => other.to_string(),
                })
                .filter(|s| !s.is_empty())
                .collect();
            list.join("#")
        }
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// 兜底：从纯文本回复里抠出选项字母
fn guess_letters(content: &str) -> String {
    let letters: Vec<char> = content
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if letters.is_empty() || letters.len() > 4 {
        return String::new();
    }
    letters
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("")
}

#[allow(clippy::too_many_arguments)]
pub async fn ask(
    client: &reqwest::Client,
    provider: &Provider,
    model: &AiModel,
    qtype: &str,
    title: &str,
    options: &str,
    system_override: &str,
) -> Result<AiAnswer, String> {
    if provider.base_url.trim().is_empty() {
        return Err("供应商未配置 base_url".into());
    }
    let system = if system_override.trim().is_empty() {
        DEFAULT_SYSTEM_PROMPT
    } else {
        system_override
    };

    let mut user = format!("题型：{}\n题干：{}", type_label(qtype), title);
    if !options.trim().is_empty() {
        user.push_str(&format!("\n选项：\n{}", options.trim()));
    }

    let body = json!({
        "model": model.model,
        "temperature": model.temperature,
        "max_tokens": model.max_tokens,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });

    let resp = client
        .post(endpoint(&provider.base_url, "chat/completions"))
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求 AI 失败：{}", describe(&e)))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取 AI 响应失败：{}", describe(&e)))?;

    if !status.is_success() {
        let brief: String = text.chars().take(300).collect();
        return Err(format!("AI 返回 {}：{}", status.as_u16(), brief));
    }

    let value: Value = serde_json::from_str(&text).map_err(|e| {
        let brief: String = text.chars().take(300).collect();
        format!("AI 响应不是合法 JSON（{}）：{}", e, brief)
    })?;

    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let usage = &value["usage"];
    let prompt_tokens = usage["prompt_tokens"].as_i64().unwrap_or(0);
    let completion_tokens = usage["completion_tokens"].as_i64().unwrap_or(0);
    let total_tokens = usage["total_tokens"]
        .as_i64()
        .unwrap_or(prompt_tokens + completion_tokens);

    let parsed = extract_json(&content);
    let mut answer = to_ocs_answer(parsed.as_ref().and_then(|v| v.get("answer")));
    let explanation = parsed
        .as_ref()
        .and_then(|v| v.get("explanation"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    // 模型没按格式返回时兜底处理
    if answer.is_empty() {
        if qtype == "completion" {
            answer = content.trim().chars().take(500).collect();
        } else if qtype == "multiple" {
            let letters = guess_letters(&content);
            if !letters.is_empty() {
                answer = letters
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join("#");
            }
        } else {
            answer = guess_letters(&content);
        }
    }

    Ok(AiAnswer {
        answer,
        explanation,
        prompt_tokens,
        completion_tokens,
        total_tokens,
    })
}

/// 拉取供应商的可用模型列表（OpenAI 兼容的 `GET /models`）。
/// 同时兼容 Ollama 的原生格式（`{"models":[{"name":...}]}`）。
pub async fn fetch_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
    if base_url.trim().is_empty() {
        return Err("base_url 不能为空".into());
    }
    let mut req = client.get(endpoint(base_url, "models"));
    // Ollama 等本地服务不需要 Key
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key.trim());
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("无法连接：{}", describe(&e)))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败：{}", describe(&e)))?;
    if !status.is_success() {
        let brief: String = text.chars().take(200).collect();
        return Err(format!("供应商返回 {}：{}", status.as_u16(), brief));
    }

    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("响应不是合法 JSON：{}", e))?;

    let mut models: Vec<String> = Vec::new();
    if let Some(arr) = value.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.push(id.to_string());
            }
        }
    }
    if models.is_empty() {
        if let Some(arr) = value.get("models").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(name) = item
                    .get("name")
                    .or_else(|| item.get("id"))
                    .and_then(|v| v.as_str())
                {
                    models.push(name.to_string());
                }
            }
        }
    }
    models.sort();
    models.dedup();
    if models.is_empty() {
        return Err("未获取到任何模型，请确认 base_url 与 API Key 是否正确".into());
    }
    Ok(models)
}

/// 测试供应商连通性
pub async fn test_provider(client: &reqwest::Client, provider: &Provider) -> Result<String, String> {
    let resp = client
        .get(endpoint(&provider.base_url, "models"))
        .bearer_auth(&provider.api_key)
        .send()
        .await
        .map_err(|e| format!("无法连接：{}", describe(&e)))?;

    if resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        let count = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v["data"].as_array().map(|a| a.len()))
            .unwrap_or(0);
        return Ok(format!("连接成功，可用模型 {} 个", count));
    }

    Err(format!("连接失败，HTTP {}", resp.status().as_u16()))
}
