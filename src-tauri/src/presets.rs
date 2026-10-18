use serde::Serialize;

/// 内置的常用 AI 供应商预设。
/// 用户只需选一个预设、填 API Key 即可，base_url 与常用模型都已备好。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    /// 稳定标识，前端用来做选择
    pub id: &'static str,
    pub name: &'static str,
    /// OpenAI 兼容的 base_url（程序会自动拼 /chat/completions 与 /models）
    pub base_url: &'static str,
    /// 去哪儿申请 Key
    pub homepage: &'static str,
    /// 给用户看的补充说明
    pub note: &'static str,
    /// 自动拉取模型失败时的兜底模型名
    pub models: &'static [&'static str],
}

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "zhipu",
        name: "智谱 AI (GLM)",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        homepage: "https://open.bigmodel.cn",
        note: "国内直连，GLM 系列性价比高",
        models: &["glm-4-flash", "glm-4-air", "glm-4-plus"],
    },
    ProviderPreset {
        id: "deepseek",
        name: "深度求索 DeepSeek",
        base_url: "https://api.deepseek.com/v1",
        homepage: "https://platform.deepseek.com",
        note: "便宜、中文作答能力强",
        models: &["deepseek-chat", "deepseek-reasoner"],
    },
    ProviderPreset {
        id: "dashscope",
        name: "阿里云百炼 (通义千问)",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        homepage: "https://bailian.console.aliyun.com",
        note: "通义千问系列，兼容模式接口",
        models: &["qwen-turbo", "qwen-plus", "qwen-max"],
    },
    ProviderPreset {
        id: "moonshot",
        name: "月之暗面 Kimi",
        base_url: "https://api.moonshot.cn/v1",
        homepage: "https://platform.moonshot.cn",
        note: "长上下文，长题干友好",
        models: &["moonshot-v1-8k", "moonshot-v1-32k"],
    },
    ProviderPreset {
        id: "siliconflow",
        name: "硅基流动 SiliconFlow",
        base_url: "https://api.siliconflow.cn/v1",
        homepage: "https://cloud.siliconflow.cn",
        note: "聚合多家开源模型，模型名形如 org/model",
        models: &[],
    },
    ProviderPreset {
        id: "hunyuan",
        name: "腾讯混元",
        base_url: "https://api.hunyuan.cloud.tencent.com/v1",
        homepage: "https://cloud.tencent.com/product/hunyuan",
        note: "腾讯云混元大模型",
        models: &["hunyuan-turbo", "hunyuan-lite"],
    },
    ProviderPreset {
        id: "volcengine",
        name: "火山方舟 (豆包)",
        base_url: "https://ark.cn-beijing.volces.com/api/v3",
        homepage: "https://console.volcengine.com/ark",
        note: "模型名需填接入点 ID（形如 ep-xxxx），请到方舟控制台创建",
        models: &[],
    },
    ProviderPreset {
        id: "spark",
        name: "讯飞星火",
        base_url: "https://spark-api-open.xf-yun.com/v1",
        homepage: "https://console.xfyun.cn",
        note: "星火大模型 HTTP 接口",
        models: &["lite", "generalv3.5", "4.0Ultra"],
    },
    ProviderPreset {
        id: "qianfan",
        name: "百度千帆 (文心一言)",
        base_url: "https://qianfan.baidubce.com/v2",
        homepage: "https://console.bce.baidu.com/qianfan",
        note: "文心一言系列",
        models: &["ernie-speed-128k", "ernie-4.0-turbo-8k"],
    },
    ProviderPreset {
        id: "minimax",
        name: "MiniMax",
        base_url: "https://api.minimax.chat/v1",
        homepage: "https://platform.minimaxi.com",
        note: "MiniMax 文本模型",
        models: &["abab6.5s-chat"],
    },
    ProviderPreset {
        id: "openai",
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        homepage: "https://platform.openai.com",
        note: "国内网络通常无法直连，需要自备代理",
        models: &["gpt-4o-mini", "gpt-4o"],
    },
    ProviderPreset {
        id: "ollama",
        name: "Ollama（本地模型）",
        base_url: "http://127.0.0.1:11434/v1",
        homepage: "https://ollama.com",
        note: "本地部署，无需 Key；需先 ollama serve",
        models: &["qwen2.5:7b", "llama3.1:8b"],
    },
    ProviderPreset {
        id: "custom",
        name: "自定义 / 其他兼容服务",
        base_url: "",
        homepage: "",
        note: "任何 OpenAI 兼容服务，base_url 需写到 /v1 这一层",
        models: &[],
    },
];

pub fn list() -> Vec<ProviderPreset> {
    PRESETS.to_vec()
}
