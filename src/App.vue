<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRoute } from "vue-router";
import AppSidebar from "./components/AppSidebar.vue";
import ThemeToggle from "./components/ThemeToggle.vue";
import ToastHost from "./components/ToastHost.vue";
import { api } from "./api";
import type { ServerStatus } from "./api/types";

const route = useRoute();
const title = computed(() => (route.meta.title as string) ?? "ai2tiku");
const desc = computed(() => (route.meta.desc as string) ?? "");

const status = ref<ServerStatus | null>(null);
let timer: number | undefined;

async function refreshStatus() {
  try {
    status.value = await api.serverStatus();
  } catch {
    status.value = null;
  }
}

onMounted(() => {
  refreshStatus();
  timer = window.setInterval(refreshStatus, 5000);
});

onUnmounted(() => {
  if (timer) window.clearInterval(timer);
});
</script>

<template>
  <div class="app-shell">
    <AppSidebar />

    <div class="main">
      <header class="topbar">
        <div>
          <div class="topbar__title">{{ title }}</div>
          <div v-if="desc" class="topbar__desc">{{ desc }}</div>
        </div>

        <div class="topbar__actions">
          <span class="badge" :class="status?.running ? 'badge--ok' : 'badge--off'">
            <span class="dot" :class="{ 'dot--pulse': status?.running }" />
            {{ status?.running ? `服务运行中 · ${status.port}` : "服务未启动" }}
          </span>
          <ThemeToggle />
        </div>
      </header>

      <main class="content">
        <RouterView v-slot="{ Component }">
          <component :is="Component" />
        </RouterView>
      </main>
    </div>

    <ToastHost />
  </div>
</template>
