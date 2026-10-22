import { invoke } from "@tauri-apps/api/core";
import type {
  AiModel,
  BrowserInfo,
  ModelInput,
  OcsRelease,
  Page,
  Provider,
  ProviderInput,
  ProviderPreset,
  Question,
  QuestionInput,
  SearchOutcome,
  SearchRecord,
  ServerStatus,
  Settings,
  Stats,
  SyncModelsResult,
} from "./types";

/** Tauri 命令封装，前端只通过这里访问后端 */
export const api = {
  // 设置与服务
  getSettings: () => invoke<Settings>("get_settings"),
  /** 内置默认提示词原文，设置页用来展示 */
  defaultSystemPrompt: () => invoke<string>("default_system_prompt"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  serverStatus: () => invoke<ServerStatus>("server_status"),
  startServer: () => invoke<ServerStatus>("start_server"),
  stopServer: () => invoke<ServerStatus>("stop_server"),

  // 界面偏好（主题色等），存本地数据库，重启不丢
  getUiPref: (key: string) => invoke<string | null>("get_ui_pref", { key }),
  setUiPref: (key: string, value: string) => invoke<void>("set_ui_pref", { key, value }),

  // 一键部署 OCS
  latestOcsRelease: () => invoke<OcsRelease>("latest_ocs_release"),
  browserInfo: () => invoke<BrowserInfo>("browser_info"),

  // 供应商
  listProviders: () => invoke<Provider[]>("list_providers"),
  listProviderPresets: () => invoke<ProviderPreset[]>("list_provider_presets"),
  saveProvider: (input: ProviderInput) => invoke<number>("save_provider", { input }),
  deleteProvider: (id: number) => invoke<void>("delete_provider", { id }),
  testProvider: (id: number) => invoke<string>("test_provider", { id }),
  /** 只试探，不入库 */
  fetchModels: (baseUrl: string, apiKey: string) =>
    invoke<string[]>("fetch_models", { baseUrl, apiKey }),
  /** 拉取该供应商的模型并自动写入题库 */
  syncProviderModels: (id: number) => invoke<SyncModelsResult>("sync_provider_models", { id }),

  // 模型
  listModels: () => invoke<AiModel[]>("list_models"),
  saveModel: (input: ModelInput) => invoke<number>("save_model", { input }),
  deleteModel: (id: number) => invoke<void>("delete_model", { id }),

  // 题库
  listQuestions: (keyword: string, source: string, page: number, pageSize: number) =>
    invoke<Page<Question>>("list_questions", { keyword, source, page, pageSize }),
  saveQuestion: (input: QuestionInput) => invoke<number>("save_question", { input }),
  deleteQuestion: (id: number) => invoke<void>("delete_question", { id }),
  clearQuestions: () => invoke<void>("clear_questions"),

  // 记录与统计
  listRecords: (page: number, pageSize: number) =>
    invoke<Page<SearchRecord>>("list_records", { page, pageSize }),
  clearRecords: () => invoke<void>("clear_records"),
  getStats: () => invoke<Stats>("get_stats"),

  // OCS
  buildOcsConfig: () => invoke<string>("build_ocs_config"),
  testSearch: (title: string, qtype: string, options: string) =>
    invoke<SearchOutcome>("test_search", { title, qtype, options }),
};
