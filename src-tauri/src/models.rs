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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 3100,
            match_threshold: 0.85,
            auto_cache: true,
            ai_fallback: true,
            system_prompt: String::new(),
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
