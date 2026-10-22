/** 与 Rust 端 models.rs 一一对应的类型定义 */

export interface Provider {
  id: number;
  name: string;
  baseUrl: string;
  apiKey: string;
  enabled: boolean;
  createdAt: string;
}

export interface ProviderInput {
  id?: number | null;
  name: string;
  baseUrl: string;
  apiKey: string;
  enabled?: boolean;
}

export interface AiModel {
  id: number;
  providerId: number;
  model: string;
  displayName: string;
  temperature: number;
  maxTokens: number;
  enabled: boolean;
  isDefault: boolean;
}

export interface ModelInput {
  id?: number | null;
  providerId: number;
  model: string;
  displayName?: string;
  temperature?: number;
  maxTokens?: number;
  enabled?: boolean;
  isDefault?: boolean;
}

export interface Question {
  id: number;
  title: string;
  options: string;
  qtype: string;
  answer: string;
  source: string;
  provider: string;
  model: string;
  hits: number;
  createdAt: string;
  updatedAt: string;
}

export interface QuestionInput {
  id?: number | null;
  title: string;
  options?: string;
  qtype?: string;
  answer: string;
}

export interface SearchRecord {
  id: number;
  title: string;
  answer: string;
  source: string;
  provider: string;
  model: string;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  durationMs: number;
  createdAt: string;
}

export interface Page<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export interface Settings {
  port: number;
  matchThreshold: number;
  autoCache: boolean;
  aiFallback: boolean;
  systemPrompt: string;
  /** 网络代理；留空 = 强制直连（不走系统代理） */
  proxyUrl: string;
}

export interface ServerStatus {
  running: boolean;
  port: number;
  address: string;
}

/** OCS 脚本最新版本（后端从 GitHub Releases 拉取） */
export interface OcsRelease {
  version: string;
  publishedAt: string;
  /** ocs.user.js 的下载地址 */
  scriptUrl: string;
}

/** 本机浏览器环境检测结果 */
export interface BrowserInfo {
  edgeInstalled: boolean;
  edgeDefault: boolean;
  defaultBrowser: string;
}

export interface DeploymentStatus {
  server: ServerStatus;
  config: string;
  aiModel: { provider: string; model: string } | null;
  aiFallback: boolean;
}

/** 内置的常用供应商预设：选一个、填 Key 即可 */
export interface ProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  homepage: string;
  note: string;
  models: string[];
}

/** 自动同步供应商模型的结果 */
export interface SyncModelsResult {
  provider: string;
  total: number;
  added: number;
  models: string[];
}

export interface TrendPoint {
  date: string;
  count: number;
  tokens: number;
}

export interface ModelUsage {
  model: string;
  count: number;
  tokens: number;
}

export interface Stats {
  questionTotal: number;
  cacheHits: number;
  aiCalls: number;
  hitRate: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  todayCount: number;
  todayTokens: number;
  trend: TrendPoint[];
  modelUsage: ModelUsage[];
}

export interface SearchOutcome {
  code: number;
  question: string | null;
  answer: string | null;
  source: string;
  similarity: number;
  explanation: string | null;
  model: string;
  tokens: number;
  durationMs: number;
  msg: string | null;
}
