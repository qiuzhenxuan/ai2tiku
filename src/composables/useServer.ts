import { computed, onMounted, onUnmounted, ref } from "vue";
import { api } from "../api";
import type { ServerStatus } from "../api/types";
import { useToast } from "./useToast";

/**
 * 全局唯一的本地服务状态。
 * 顶栏、首页、接入向导、设置页共用同一份状态和同一个轮询，
 * 不再各自请求导致显示不一致。
 */
const status = ref<ServerStatus | null>(null);
const busy = ref(false);
let subscribers = 0;
let timer: number | undefined;
let pending: Promise<void> | undefined;

function refresh(): Promise<void> {
  if (document.hidden) return Promise.resolve();
  if (pending) return pending;
  pending = (async () => {
    try {
      status.value = await api.serverStatus();
    } catch {
      status.value = null;
    } finally {
      pending = undefined;
    }
  })();
  return pending;
}

const onFocus = () => void refresh();

export function useServer() {
  const toast = useToast();
  const running = computed(() => !!status.value?.running);

  async function setRunning(next: boolean) {
    if (busy.value) return;
    busy.value = true;
    try {
      status.value = next ? await api.startServer() : await api.stopServer();
      toast.ok(status.value.running ? "本地题库服务已启动" : "本地题库服务已停止");
    } catch (e) {
      await refresh();
      toast.fromError(e);
    } finally {
      busy.value = false;
    }
  }

  onMounted(() => {
    if (subscribers++ === 0) {
      void refresh();
      timer = window.setInterval(onFocus, 5000);
      window.addEventListener("focus", onFocus);
    }
  });
  onUnmounted(() => {
    if (--subscribers === 0) {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    }
  });

  return {
    status,
    running,
    busy,
    refresh,
    start: () => setRunning(true),
    stop: () => setRunning(false),
    toggle: () => setRunning(!running.value),
  };
}
