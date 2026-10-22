<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import AppLogo from "./AppLogo.vue";

/** 只有跑在 Tauri 里才有窗口 API，浏览器预览时全部跳过 */
const win = (() => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
})();

const maximized = ref(false);
let unlisten: (() => void) | undefined;

async function syncMaximized() {
  if (!win) return;
  try {
    maximized.value = await win.isMaximized();
  } catch {
    /* ignore */
  }
}

async function minimize() {
  await win?.minimize().catch(() => {});
}

async function toggleMaximize() {
  if (!win) return;
  await win.toggleMaximize().catch(() => {});
  await syncMaximized();
}

async function close() {
  await win?.close().catch(() => {});
}

onMounted(async () => {
  await syncMaximized();
  if (!win) return;
  unlisten = await win.onResized(syncMaximized).catch(() => undefined);
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <header class="titlebar">
    <div class="titlebar__drag" data-tauri-drag-region>
      <span class="titlebar__mark" data-tauri-drag-region><AppLogo /></span>
      <span class="titlebar__title" data-tauri-drag-region>ai2tiku · AI 题库</span>
    </div>

    <div class="titlebar__actions">
      <button class="titlebar__btn" type="button" title="最小化" @click="minimize">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 5h8" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button
        class="titlebar__btn"
        type="button"
        :title="maximized ? '向下还原' : '最大化'"
        @click="toggleMaximize"
      >
        <svg v-if="!maximized" width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <rect x="1" y="1" width="8" height="8" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
        <svg v-else width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <rect x="1" y="3" width="6" height="6" fill="none" stroke="currentColor" stroke-width="1" />
          <path d="M3 3V1h6v6H7" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button
        class="titlebar__btn titlebar__btn--close"
        type="button"
        title="关闭"
        @click="close"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
    </div>
  </header>
</template>
