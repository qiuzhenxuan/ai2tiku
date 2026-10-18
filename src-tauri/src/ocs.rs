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
            "contentType": "json",
            "type": "GM_xmlhttpRequest",
            "headers": { "Content-Type": "application/json" },
            "data": {
                "title": "${title}",
                "type": "${type}",
                "options": "${options}"
            },
            // OCS 的 handler 会拿到题库返回的数据，这里转成 [题目, 答案]；
            // 未命中时返回 [提示信息, undefined]，OCS 会把提示显示给用户。
            "handler": "return (res)=> res.code === 1 ? [res.question, res.answer] : [res.msg, undefined]"
        }
    ])
}
