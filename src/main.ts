import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import { useAccent } from "./composables/useAccent";
import "./styles/main.css";

// 启动就套用上次选的主题色，避免先闪一下默认蓝
useAccent();

createApp(App).use(router).mount("#app");
