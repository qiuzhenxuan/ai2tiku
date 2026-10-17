import { createRouter, createWebHashHistory } from "vue-router";

const routes = [
  {
    path: "/",
    name: "dashboard",
    component: () => import("../views/DashboardView.vue"),
    meta: { title: "概览", desc: "本地 AI 题库的运行状态一览" },
  },
  {
    path: "/ocs",
    name: "ocs",
    component: () => import("../views/OcsConfigView.vue"),
    meta: { title: "OCS 题库配置", desc: "生成题库配置并接入 OCS 网课助手" },
  },
  {
    path: "/providers",
    name: "providers",
    component: () => import("../views/ProvidersView.vue"),
    meta: { title: "AI 供应商与模型", desc: "配置 OpenAI 兼容的服务与模型" },
  },
  {
    path: "/questions",
    name: "questions",
    component: () => import("../views/QuestionsView.vue"),
    meta: { title: "题库管理", desc: "管理本地缓存的题目与答案" },
  },
  {
    path: "/records",
    name: "records",
    component: () => import("../views/RecordsView.vue"),
    meta: { title: "答题记录", desc: "每一次搜题的详细调用记录" },
  },
  {
    path: "/stats",
    name: "stats",
    component: () => import("../views/StatsView.vue"),
    meta: { title: "统计看板", desc: "命中率与 Token 消耗趋势" },
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("../views/SettingsView.vue"),
    meta: { title: "设置", desc: "端口、匹配阈值与答题行为" },
  },
];

export default createRouter({
  history: createWebHashHistory(),
  routes,
});

export { routes };
