import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";

export interface NavMeta {
  title: string;
  desc: string;
  icon: string;
  group: "start" | "data" | "system";
}

/** 侧栏导航：按「开始使用 / 数据 / 系统」分组 */
const routes: (RouteRecordRaw & { name: string; meta: NavMeta })[] = [
  {
    path: "/",
    name: "home",
    component: () => import("../views/HomeView.vue"),
    meta: { title: "首页", desc: "运行状态与接入进度", icon: "dashboard", group: "start" },
  },
  {
    path: "/setup",
    name: "setup",
    component: () => import("../views/SetupView.vue"),
    meta: { title: "接入向导", desc: "五步把本地题库接进 OCS", icon: "rocket", group: "start" },
  },
  {
    path: "/models",
    name: "models",
    component: () => import("../views/ModelsView.vue"),
    meta: { title: "AI 模型", desc: "供应商、API Key 与默认模型", icon: "sparkles", group: "start" },
  },
  {
    path: "/questions",
    name: "questions",
    component: () => import("../views/QuestionsView.vue"),
    meta: { title: "题库", desc: "本地缓存的题目与答案", icon: "database", group: "data" },
  },
  {
    path: "/records",
    name: "records",
    component: () => import("../views/RecordsView.vue"),
    meta: { title: "记录与统计", desc: "每次搜题的结果与 Token 消耗", icon: "list", group: "data" },
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("../views/SettingsView.vue"),
    meta: { title: "设置", desc: "服务、作答策略、外观与诊断", icon: "settings", group: "system" },
  },
];

/** 旧版地址保留跳转，避免外部链接或习惯失效 */
const redirects: RouteRecordRaw[] = [
  { path: "/deploy", redirect: "/setup" },
  { path: "/ocs", redirect: { path: "/setup", query: { step: "connect" } } },
  { path: "/providers", redirect: (to) => ({ path: "/models", query: to.query }) },
  { path: "/stats", redirect: { path: "/records", query: { tab: "stats" } } },
  { path: "/:pathMatch(.*)*", redirect: "/" },
];

export default createRouter({
  history: createWebHashHistory(),
  routes: [...routes, ...redirects],
});

export const GROUP_LABELS: Record<NavMeta["group"], string> = {
  start: "开始使用",
  data: "数据",
  system: "系统",
};

export { routes };
