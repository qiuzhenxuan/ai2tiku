use serde_json::{json, Value};

/// OCS「题库配置」的唯一来源。
/// 同时供两个地方使用：
///   1. 前端「OCS 题库配置」页展示 / 复制
///   2. 本地服务 `GET /ocs` 订阅链接（OCS 官方支持填订阅链接，
///      打开后返回这段 JSON 即可，比整段粘贴更省事）
pub fn config_value(port: u16) -> Value {
    let base = crate::server::address(port);
    json!([
        {
            "name": "ai2tiku (本地 AI 题库)",
            "homepage": base,
            "url": format!("{}/api/search", base),
            "method": "post",
            // contentType 是 5.0 之前的旧字段，responseType 取代它；两个都写上以兼容 4.x。
            // 另外 OCS 的 isAnswererWrapperSupportImageOptimize 只认 POST，
            // 所以图片题优化只对 POST 题库生效。
            "contentType": "json",
            "responseType": "json",
            "type": "GM_xmlhttpRequest",
            "headers": { "Content-Type": "application/json" },
            "data": {
                "title": "${title}",
                "type": "${type}",
                "options": "${options}",
                // 以下三个字段是 OCS 5.0 图片题优化的声明：
                // 在 data 里引用 ${images} / ${suggestion_title} / ${suggestion_options}
                // 即被视为「支持图片题优化」，否则 OCS 会提示题库不支持图片题。
                //
                // images 是数组（data:image/png;base64,... 列表），
                // 用字符串占位符会被拼成一个字符串（base64 里还带逗号，无法还原），
                // 所以改用字段解析器返回数组——这也是 OCS 官方保留数组结构的方式。
                "images": { "handler": "return (env) => env.images" },
                // suggestion_title：题干里图片 URL 已被替换成 [图片N] 的文本
                "suggestion_title": "${suggestion_title}",
                "suggestion_options": "${suggestion_options}"
            },
            // OCS 的 handler 会拿到题库返回的数据，这里转成 [题目, 答案]；
            // 未命中时返回 [提示信息, undefined]，OCS 会把提示显示给用户。
            "handler": "return (res)=> res.code === 1 ? [res.question, res.answer] : [res.msg, undefined]"
        }
    ])
}
