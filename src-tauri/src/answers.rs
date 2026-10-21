use serde_json::Value;

/// 只接受完整 JSON（可带完整代码围栏），不从分析段落中截取答案。
pub(crate) fn extract_json(content: &str) -> Option<Value> {
    let text = content.trim();
    let text = if text.starts_with("```") {
        text.strip_prefix("```json")
            .or_else(|| text.strip_prefix("```JSON"))
            .or_else(|| text.strip_prefix("```"))?
            .strip_suffix("```")?
            .trim()
    } else {
        text
    };
    serde_json::from_str(text).ok()
}

/// 按题型校验整个 answer 字段。空数组、空位、对象等均不能静默转换。
pub(crate) fn validate(value: &Value, qtype: &str, options: &str) -> Option<String> {
    let parts: Vec<&str> = match value {
        Value::String(s) => vec![s.trim()],
        Value::Array(items) if matches!(qtype, "multiple" | "completion") => items
            .iter()
            .map(|v| v.as_str().map(str::trim))
            .collect::<Option<Vec<_>>>()?,
        _ => return None,
    };
    if parts.is_empty() || parts.iter().any(|s| s.is_empty()) {
        return None;
    }
    match qtype {
        "single" | "multiple" => {
            let count = options.lines().filter(|s| !s.trim().is_empty()).count();
            if count == 0 || count > 26 {
                return None;
            }
            let joined = parts.join("#");
            // 支持 A、AC、A#C、A,C；绝不从句子里抽取英文字母。
            let letters = if joined.bytes().all(|c| c.is_ascii_alphabetic()) {
                joined.chars().map(|c| c.to_string()).collect::<Vec<_>>()
            } else {
                joined
                    .split(['#', ',', '，', '、'])
                    .map(|s| s.trim().to_string())
                    .collect()
            };
            if letters.is_empty() || (qtype == "single" && letters.len() != 1) {
                return None;
            }
            let mut indices = Vec::new();
            for letter in letters {
                if letter.len() != 1 || !letter.as_bytes()[0].is_ascii_alphabetic() {
                    return None;
                }
                let c = letter.as_bytes()[0].to_ascii_uppercase();
                if (c - b'A') as usize >= count || indices.contains(&c) {
                    return None;
                }
                indices.push(c);
            }
            indices.sort_unstable();
            Some(
                indices
                    .into_iter()
                    .map(|c| (c as char).to_string())
                    .collect::<Vec<_>>()
                    .join("#"),
            )
        }
        "judgement" => match parts.as_slice() {
            ["对" | "正确" | "true" | "True" | "TRUE"] => Some("对".into()),
            ["错" | "错误" | "false" | "False" | "FALSE"] => Some("错".into()),
            _ => None,
        },
        // 自由文本不能仅凭“短”就认定为答案，调用方必须已确认 JSON 结构。
        "completion" => {
            if parts
                .iter()
                .any(|s| s.split('#').any(|p| p.trim().is_empty()))
            {
                return None;
            }
            if parts.iter().any(|s| {
                [
                    "无法确定",
                    "不确定",
                    "无法判断",
                    "不知道",
                    "信息不足",
                    "无法作答",
                ]
                .iter()
                .any(|marker| s.contains(marker))
            }) {
                return None;
            }
            Some(parts.join("#"))
        }
        _ => None,
    }
}

pub(crate) fn parse(content: &str, qtype: &str, options: &str, finish: &str) -> String {
    // 即使截断回复里恰好包含一个可解析的答案，也不视为完整作答。
    if finish != "stop" {
        return String::new();
    }
    let answer = if let Some(value) = extract_json(content) {
        value
            .get("answer")
            .and_then(|v| validate(v, qtype, options))
    } else if matches!(qtype, "single" | "multiple" | "judgement") {
        validate(&Value::String(content.trim().into()), qtype, options)
    } else {
        None
    };
    answer.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    const OPTIONS: &str = "A. 甲\nB. 乙\nC. 丙\nD. 丁\nE. 戊";

    #[test]
    fn rejects_uncertainty_invalid_types_and_out_of_range() {
        for content in [
            "无法判断，A和B都有可能",
            "答案可能是A",
            "A or B",
            "Z",
            "A#A",
            "A#",
            "{\"answer\":null}",
            "{\"answer\":{\"x\":\"A\"}}",
            "{\"answer\":[]}",
            "{\"answer\":[\"A\",\"\"]}",
            "先猜测一下{\"answer\":\"A\"}",
        ] {
            assert_eq!(parse(content, "multiple", OPTIONS, "stop"), "", "{content}");
        }
        assert_eq!(parse("AB", "single", OPTIONS, "stop"), "");
        assert_eq!(parse("A", "single", "", "stop"), "");
        assert_eq!(parse("A", "unknown", OPTIONS, "stop"), "");
        assert_eq!(parse("{\"answer\":true}", "single", OPTIONS, "stop"), "");
    }

    #[test]
    fn accepts_only_complete_answers_for_each_type() {
        assert_eq!(parse("a", "single", OPTIONS, "stop"), "A");
        assert_eq!(parse("ABCDE", "multiple", OPTIONS, "stop"), "A#B#C#D#E");
        assert_eq!(
            parse("{\"answer\":[\"C\",\"A\"]}", "multiple", OPTIONS, "stop"),
            "A#C"
        );
        assert_eq!(
            parse(
                "```json\n{\"answer\":\"B\"}\n```",
                "single",
                OPTIONS,
                "stop"
            ),
            "B"
        );
        assert_eq!(parse("错", "judgement", "对\n错", "stop"), "错");
        assert_eq!(
            parse("{\"answer\":[\"张三\",\"李四\"]}", "completion", "", "stop"),
            "张三#李四"
        );
        assert_eq!(
            parse(
                "{\"answer\":[\"张三\",\"\",\"李四\"]}",
                "completion",
                "",
                "stop"
            ),
            ""
        );
        assert_eq!(parse("无法确定", "completion", "", "stop"), "");
        assert_eq!(
            parse("{\"answer\":\"无法确定\"}", "completion", "", "stop"),
            ""
        );
        assert_eq!(parse("{\"answer\":\"\"}", "completion", "", "stop"), "");
        for finish in ["length", "content_filter", "", "tool_calls"] {
            assert_eq!(parse("{\"answer\":\"A\"}", "single", OPTIONS, finish), "");
        }
    }
}
