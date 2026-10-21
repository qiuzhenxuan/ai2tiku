import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import { hydrateAccent, useAccent } from "./composables/useAccent";
import { hydrateTheme, useTheme } from "./composables/useTheme";
import "./styles/main.css";

// 启动就套用上次选的主题色与日夜主题（先用本地缓存避免闪），再从数据库校准
useAccent();
hydrateAccent();
useTheme();
hydrateTheme();

createApp(App).use(router).mount("#app");
