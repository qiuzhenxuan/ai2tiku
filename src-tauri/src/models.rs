use serde::{Deserialize, Serialize};

/// AI 供应商（任意 OpenAI 兼容服务：DeepSeek / 通义 / OpenAI / OneAPI / Ollama 等）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub id: Option<i64>,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: Option<bool>,
}

/// 供应商下的模型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModel {
    pub id: i64,
    pub provider_id: i64,
    pub model: String,
    pub display_name: String,
    pub temperature: f64,
    pub max_tokens: i64,
    pub enabled: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInput {
    pub id: Option<i64>,
    pub provider_id: i64,
    pub model: String,
    pub display_name: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

/// 本地题库中的一道题
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: i64,
    pub title: String,
    pub options: String,
    pub qtype: String,
    pub answer: String,
    pub source: String,
    pub provider: String,
    pub model: String,
    pub hits: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionInput {
    pub id: Option<i64>,
    pub title: String,
    pub options: Option<String>,
    pub qtype: Option<String>,
    pub answer: String,
}

/// 一次搜题的调用记录
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRecord {
    pub id: i64,
    pub title: String,
    pub answer: String,
    pub source: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: i64,
    pub created_at: String,
    #[serde(default)]
    pub options: String,
    #[serde(default)]
    pub qtype: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub image_count: i64,
}

/// 分页返回
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// 全局设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub port: u16,
    pub match_threshold: f64,
    pub auto_cache: bool,
    pub ai_fallback: bool,
    pub system_prompt: String,
    /// 网络代理。留空 = 强制直连（不走系统代理）；填写则所有 AI 请求走该代理，
    /// 例如 `http://127.0.0.1:7890`。访问 OpenAI 等被墙服务时才需要。
    pub proxy_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 3100,
            match_threshold: 0.85,
            auto_cache: true,
            ai_fallback: true,
            system_prompt: String::new(),
            proxy_url: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
    pub address: String,
}

/// 自动同步供应商模型的结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncModelsResult {
    pub provider: String,
    /// 供应商返回的模型总数
    pub total: usize,
    /// 本次新入库的数量
    pub added: usize,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub date: String,
    pub count: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub count: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub success_count: i64,
    pub failure_count: i64,
    pub success_rate: f64,
    pub question_total: i64,
    pub cache_hits: i64,
    pub ai_calls: i64,
    pub hit_rate: f64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub today_count: i64,
    pub today_tokens: i64,
    pub trend: Vec<TrendPoint>,
    pub model_usage: Vec<ModelUsage>,
}

/// 搜题请求（OCS 会传来 title / type / options）
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "type")]
    pub qtype: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub options: String,
    /// 图片题优化：图片的 base64 data URL 数组（对应题干/选项里的 [图片N]）
    #[serde(default)]
    pub images: Vec<String>,
    /// 图片题优化：题干中的图片 URL 被替换为 [图片N] 后的文本
    #[serde(default)]
    pub suggestion_title: String,
    /// 图片题优化：选项中的图片 URL 被替换为 [图片N] 后的文本
    #[serde(default)]
    pub suggestion_options: String,
}

impl SearchQuery {
    /// OCS 可能把题干放在 title 或 question 字段
    pub fn title(&self) -> &str {
        if !self.title.is_empty() {
            &self.title
        } else {
            &self.question
        }
    }

    /// 实际拿来提问的题干：图片题时 title 里是原始图片 URL（一大串噪声），
    /// 用 suggestion_title（[图片N] 占位）更干净，图片本身另走 images 字段。
    pub fn ask_title(&self) -> &str {
        if self.suggestion_title.is_empty() {
            self.title()
        } else {
            &self.suggestion_title
        }
    }

    /// 实际拿来提问的选项，规则同上
    pub fn ask_options(&self) -> &str {
        if self.suggestion_options.is_empty() {
            &self.options
        } else {
            &self.suggestion_options
        }
    }
}

/// AI 给出的答案
#[derive(Debug, Clone)]
pub struct AiAnswer {
    pub answer: String,
    pub explanation: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

/// 一次搜题的结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchOutcome {
    pub code: i64,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub source: String,
    pub similarity: f64,
    pub explanation: Option<String>,
    pub model: String,
    pub tokens: i64,
    pub duration_ms: i64,
    pub msg: Option<String>,
}

/// OCS 脚本的最新版本信息（运行时从 GitHub Releases 拉取）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcsRelease {
    pub version: String,
    pub published_at: String,
    /// ocs.user.js 的下载地址，脚本管理器会自动弹安装页
    pub script_url: String,
}

/// 浏览器环境检测结果（用于一键部署引导）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    pub edge_installed: bool,
    pub edge_default: bool,
    pub default_browser: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveModelInfo {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentStatus {
    pub server: ServerStatus,
    pub config: String,
    pub ai_model: Option<ActiveModelInfo>,
    pub ai_fallback: bool,
}
