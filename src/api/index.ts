import { invoke } from "@tauri-apps/api/core";
import type {
  AiModel,
  ModelInput,
  Page,
  Provider,
  ProviderInput,
  Question,
  QuestionInput,
  SearchOutcome,
  SearchRecord,
  ServerStatus,
  Settings,
  Stats,
} from "./types";

/** Tauri 命令封装，前端只通过这里访问后端 */
export const api = {
  // 设置与服务
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  serverStatus: () => invoke<ServerStatus>("server_status"),
  startServer: () => invoke<ServerStatus>("start_server"),
  stopServer: () => invoke<ServerStatus>("stop_server"),

  // 供应商
  listProviders: () => invoke<Provider[]>("list_providers"),
  saveProvider: (input: ProviderInput) => invoke<number>("save_provider", { input }),
  deleteProvider: (id: number) => invoke<void>("delete_provider", { id }),
  testProvider: (id: number) => invoke<string>("test_provider", { id }),

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
