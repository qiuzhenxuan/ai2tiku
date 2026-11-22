<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { routes } from "./router";
import AppSidebar from "./components/AppSidebar.vue";
import TitleBar from "./components/TitleBar.vue";
import ThemeToggle from "./components/ThemeToggle.vue";
import ToastHost from "./components/ToastHost.vue";
import ConfirmHost from "./components/ConfirmHost.vue";
import AppIcon from "./components/AppIcon.vue";
import { useServer } from "./composables/useServer";

const route = useRoute();
const router = useRouter();
const contentEl = ref<HTMLElement | null>(null);

/* 切换页面时回到顶部（带锚点的除外） */
watch(() => route.path, () => { if (!route.hash) contentEl.value?.scrollTo({ top: 0 }); });

/* Ctrl + 1…6 快速切换页面 */
function onShortcut(e: KeyboardEvent) {
  if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
  const n = Number(e.key);
  if (!Number.isInteger(n) || n < 1 || n > routes.length) return;
  e.preventDefault();
  void router.push(routes[n - 1].path);
}
const title = computed(() => (route.meta.title as string) ?? "ai2tiku");
const desc = computed(() => (route.meta.desc as string) ?? "");

const { status, running, busy, toggle } = useServer();

/** 侧栏默认展开显示文字；窄窗口自动收起；用户选择会被记住 */
const RAIL_KEY = "ai2tiku:rail-expanded";
const narrow = ref(window.innerWidth < 960);
const preferred = ref(localStorage.getItem(RAIL_KEY) !== "0");
const sidebarExpanded = computed(() => preferred.value && !narrow.value);
function toggleRail() {
  if (narrow.value) {
    narrow.value = false;
    preferred.value = true;
  } else preferred.value = !preferred.value;
}
watch(preferred, (v) => localStorage.setItem(RAIL_KEY, v ? "1" : "0"));
const onResize = () => { narrow.value = window.innerWidth < 960; };

function preventBrowserMenu(event: Event) {
  event.preventDefault();
}

onMounted(() => {
  // Capture also covers modal content teleported outside the app-shell.
  document.addEventListener("contextmenu", preventBrowserMenu, true);
  window.addEventListener("resize", onResize);
  window.addEventListener("keydown", onShortcut);
});

onUnmounted(() => {
  document.removeEventListener("contextmenu", preventBrowserMenu, true);
  window.removeEventListener("resize", onResize);
  window.removeEventListener("keydown", onShortcut);
});
</script>

<template>
  <div class="app-shell" :class="{ 'is-rail-expanded': sidebarExpanded }">
    <TitleBar />
    <AppSidebar :expanded="sidebarExpanded" @toggle="toggleRail" />

    <div class="main">
      <header class="topbar">
        <div>
          <div class="topbar__title">{{ title }}</div>
          <div v-if="desc" class="topbar__desc">{{ desc }}</div>
        </div>
        <div class="topbar__actions">
          <button
            class="server-pill"
            :class="{ 'is-on': running }"
            type="button"
            :disabled="busy"
            :title="running ? '点击停止本地题库服务' : '点击启动本地题库服务'"
            @click="toggle"
          >
            <span class="dot" :class="{ 'dot--pulse': running }" />
            <span class="server-pill__text">{{ running ? `服务运行中 · ${status?.port}` : status ? "服务已停止" : "读取服务状态…" }}</span>
            <span class="server-pill__action">
              <AppIcon :name="busy ? 'refresh' : 'power'" :size="12" :class="{ spin: busy }" />
              {{ running ? "停止" : "启动" }}
            </span>
          </button>
          <ThemeToggle />
        </div>
      </header>

      <main ref="contentEl" class="content">
        <RouterView v-slot="{ Component, route: r }">
          <Transition name="page" mode="out-in">
            <component :is="Component" :key="r.path" />
          </Transition>
        </RouterView>
      </main>
    </div>

    <ToastHost />
    <ConfirmHost />
  </div>
</template>
