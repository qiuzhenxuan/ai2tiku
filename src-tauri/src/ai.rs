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
- 如果确实无法确定答案，answer 填 ""（空字符串）
- 不要在 JSON 之外输出任何内容，也不要先写分析过程再给答案"#;

/// 第一次没取到答案时的重试提示。
///
/// 之所以需要它：模型偶尔会先长篇分析再给答案，回复在 max_tokens 上限处被截断，
/// 整段文本既解析不出 JSON、也抠不出选项字母，最后返回「空答案」导致漏题。
/// 这里改成「只要答案」的极短输出，就不会再撞上限。
const RETRY_SYSTEM_PROMPT: &str = r#"你是答题助手。只输出答案本身，不要任何解释、分析、过程描述、序号或多余文字。
- 单选题：只输出选项字母，例如 A
- 多选题：按字母顺序输出，字母之间用 # 连接，例如 A#C
- 判断题：只输出 对 或 错
- 填空题：只输出要填入的内容；有多个空时按顺序用 # 连接，例如 张三#李四"#;

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

/// 构造 HTTP 客户端。
///
/// `proxy_url` 为空时**强制直连**（`no_proxy`）：reqwest 默认会读取系统代理，
/// 而国内常见的代理软件（Clash 等）会把 Windows 系统代理指向自己的端口；
/// 一旦该端口没有服务，所有请求都会失败在 `tunnel error → os error 10061`。
/// 国内模型（智谱/DeepSeek/通义等）本来就能直连，所以默认不走代理更稳。
/// 需要访问 OpenAI 等被墙服务时，在「设置」里填代理地址即可。
pub fn build_client(proxy_url: &str) -> reqwest::Client {
    let proxy = proxy_url.trim();
    let builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15));

    let builder = if proxy.is_empty() {
        builder.no_proxy()
    } else {
        match reqwest::Proxy::all(proxy) {
            Ok(p) => builder.proxy(p),
            Err(_) => builder.no_proxy(),
        }
    };

    builder.build().unwrap_or_default()
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

/// 填空题：模型没给 JSON、直接吐了纯文本答案时的兜底。
/// 只有短且没有换行的才算答案 —— 长篇大论（尤其被 max_tokens 截断的那种）不是答案，
/// 返回空让上层去重试，也好过把一段议论文交给 OCS。
fn loose_text_answer(content: &str) -> String {
    let t = content.trim();
    if t.is_empty() || t.contains('\n') || t.chars().count() > 50 {
        String::new()
    } else {
        t.to_string()
    }
}

/// 单次模型调用的结果；`answer` 为空表示这次没拿到可用答案
struct Completion {
    answer: String,
    explanation: String,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    /// finish_reason == "length"，即回复被 max_tokens 截断了
    truncated: bool,
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
    // 图片题优化带来的图片（data:image/png;base64,... 形式）；
    // 非空时按 OpenAI 多模态格式提交，需选用支持视觉的模型
    images: &[String],
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

    let first = complete(
        client,
        provider,
        model,
        qtype,
        &user,
        images,
        system,
        model.temperature,
    )
    .await?;

    if !first.answer.trim().is_empty() {
        return Ok(AiAnswer {
            answer: first.answer,
            explanation: first.explanation,
            prompt_tokens: first.prompt_tokens,
            completion_tokens: first.completion_tokens,
            total_tokens: first.total_tokens,
        });
    }

    // 第一次没拿到答案：多半是模型先写了一段分析、回复在 max_tokens 上限被截断，
    // 截断后既解析不出 JSON、也抠不出选项字母。
    // 换「只要答案」的提示重试一次 —— 这种输出只有几个 token，不会再撞上限。
    let (mut p, mut c, mut t) = (
        first.prompt_tokens,
        first.completion_tokens,
        first.total_tokens,
    );
    let mut reason = if first.truncated {
        format!(
            "AI 回复在 max_tokens（{}）上限处被截断，未能取到答案；可在「AI 供应商」里把该模型的 max_tokens 调大",
            model.max_tokens
        )
    } else {
        "AI 未能确定答案".to_string()
    };

    match complete(
        client,
        provider,
        model,
        qtype,
        &user,
        images,
        RETRY_SYSTEM_PROMPT,
        0.0,
    )
    .await
    {
        Ok(second) => {
            p += second.prompt_tokens;
            c += second.completion_tokens;
            t += second.total_tokens;
            if !second.answer.trim().is_empty() {
                return Ok(AiAnswer {
                    answer: second.answer,
                    explanation: second.explanation,
                    prompt_tokens: p,
                    completion_tokens: c,
                    total_tokens: t,
                });
            }
        }
        Err(e) => reason = format!("{}（重试也失败：{}）", reason, e),
    }

    // 两次都没答出来：把原因放进 explanation，让调用方原样提示给用户
    Ok(AiAnswer {
        answer: String::new(),
        explanation: reason,
        prompt_tokens: p,
        completion_tokens: c,
        total_tokens: t,
    })
}

/// 发一次请求并解析结果（不含重试逻辑）
#[allow(clippy::too_many_arguments)]
async fn complete(
    client: &reqwest::Client,
    provider: &Provider,
    model: &AiModel,
    qtype: &str,
    user_text: &str,
    images: &[String],
    system: &str,
    temperature: f64,
) -> Result<Completion, String> {
    // 图片题：题干里的图片 URL 已被 OCS 换成 [图片N] 占位符，图片本体随 images 一起传来。
    // 这里按 OpenAI 的多模态格式（content 为数组）提交，并要求按图片内容作答。
    let content = if images.is_empty() {
        json!(user_text)
    } else {
        let text = format!(
            "{}\n本题含 {} 张图片（按 [图片N] 顺序对应），请结合图片内容作答。",
            user_text,
            images.len()
        );
        let mut parts = vec![json!({ "type": "text", "text": text })];
        for img in images {
            parts.push(json!({ "type": "image_url", "image_url": { "url": img } }));
        }
        Value::Array(parts)
    };

    let body = json!({
        "model": model.model,
        "temperature": temperature,
        "max_tokens": model.max_tokens,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": content }
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
        let mut msg = format!("AI 返回 {}：{}", status.as_u16(), brief);
        // 图片题的常见失败：模型不支持图片输入，供应商往往只回一句
        // 「messages.content.type 参数非法」之类的提示，看不出是模型选错了
        if !images.is_empty() {
            let lower = text.to_lowercase();
            if lower.contains("content.type") || lower.contains("image") || lower.contains("vision") {
                msg.push_str("；当前模型可能不支持图片输入，请在「AI 供应商」里改用支持视觉的模型（如 glm-4.5v、qwen-vl-max）");
            }
        }
        return Err(msg);
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
            answer = loose_text_answer(&content);
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

    Ok(Completion {
        answer,
        explanation,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        truncated: value["choices"][0]["finish_reason"].as_str() == Some("length"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AiModel, Provider};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// 一个极简的 OpenAI 兼容桩：按顺序回指定的 (content, finish_reason, completion_tokens)。
    /// 用来确定性地验证「第一次没拿到答案 → 用严格提示重试」，不依赖真模型的随机行为。
    fn stub(replies: Vec<(&'static str, &'static str, i64)>) -> (u16, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("端口占用");
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = Arc::clone(&seen);
        let total = replies.len();
        std::thread::spawn(move || {
            for (content, finish, ct) in replies.into_iter().take(total) {
                let Ok((mut sock, _)) = listener.accept() else {
                    return;
                };
                let mut buf = vec![0u8; 128 * 1024];
                let n = sock.read(&mut buf).unwrap_or(0);
                seen2
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());

                let body = json!({
                    "choices": [{ "message": { "content": content }, "finish_reason": finish }],
                    "usage": { "prompt_tokens": 10, "completion_tokens": ct, "total_tokens": 10 + ct }
                })
                .to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            }
        });
        (port, seen)
    }

    fn provider(port: u16) -> Provider {
        Provider {
            id: 1,
            name: "stub".into(),
            base_url: format!("http://127.0.0.1:{}/v1", port),
            api_key: "test".into(),
            enabled: true,
            created_at: String::new(),
        }
    }

    fn model() -> AiModel {
        AiModel {
            id: 1,
            provider_id: 1,
            model: "stub-model".into(),
            display_name: "stub".into(),
            temperature: 0.3,
            max_tokens: 1024,
            enabled: true,
            is_default: true,
        }
    }

    /// 第一次是「跑偏的长回复」（纯中文、没有 JSON、也没有选项字母）→ 必须重试并拿到答案
    #[tokio::test]
    async fn retries_once_with_strict_prompt_when_no_answer() {
        let (port, seen) = stub(vec![
            (
                "让我先分析一下这道题。题目考查的是人生价值的本质……",
                "length",
                1024,
            ),
            ("{\"answer\":\"B\",\"explanation\":\"\"}", "stop", 20),
        ]);
        let client = build_client("");
        let res = ask(
            &client,
            &provider(port),
            &model(),
            "single",
            "测试题干",
            "A. 甲\nB. 乙",
            "",
            &[],
        )
        .await
        .expect("第一次调用本身应当成功");

        println!("第一次解析不出答案 → 重试后 answer = {:?}", res.answer);
        assert_eq!(res.answer, "B", "重试应当把答案取回来");
        assert_eq!(res.total_tokens, 1034 + 30, "两次调用的 token 应累加");

        let reqs = seen.lock().unwrap();
        assert_eq!(reqs.len(), 2, "应当发出两次请求");
        assert!(
            reqs[0].contains("你是一个严谨的答题助手"),
            "第一次用的是默认系统提示"
        );
        assert!(
            reqs[1].contains("只输出答案本身"),
            "第二次应当改用「只要答案」的严格提示"
        );
    }

    /// 第一次就有答案时不能多打一次请求（别把重试变成常态开销）
    #[tokio::test]
    async fn no_retry_when_first_reply_already_answers() {
        let (port, seen) = stub(vec![(
            "{\"answer\":\"A\",\"explanation\":\"ok\"}",
            "stop",
            25,
        )]);
        let client = build_client("");
        let res = ask(
            &client,
            &provider(port),
            &model(),
            "single",
            "测试题干",
            "A. 甲\nB. 乙",
            "",
            &[],
        )
        .await
        .expect("应当成功");
        println!("第一次就答出来了 → answer = {:?}", res.answer);
        assert_eq!(res.answer, "A");
        assert_eq!(seen.lock().unwrap().len(), 1, "不应发出第二次请求");
    }

    /// 填空题：一段被截断的长篇大论不能被当成答案交出去
    #[test]
    fn long_rambling_is_not_a_completion_answer() {
        assert_eq!(loose_text_answer("这是一段很长的分析，先说结论……"), "这是一段很长的分析，先说结论……");
        assert_eq!(loose_text_answer("神"), "神");
        assert_eq!(loose_text_answer("第一行\n第二行"), "");
        assert_eq!(loose_text_answer(&"字".repeat(60)), "");
    }
}
