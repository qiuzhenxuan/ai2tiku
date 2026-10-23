import { onMounted, onUnmounted, ref } from "vue";
import { api } from "../api";

/** Cheap revision checks: reload only changed data; suspend hidden windows. */
export function useLiveRefresh(load: (quiet?: boolean) => Promise<unknown>) {
  const connected = ref(true);
  let revision: number | undefined;
  let running = false, disposed = false, queued = false;
  let attempted = false;
  let timer: number | undefined;
  async function refresh(force = false) {
    if (disposed || document.hidden) return;
    if (running) { queued ||= force; return; }
    running = true;
    try {
      const next = await api.dataRevision();
      if (disposed) return;
      if (force || revision === undefined || revision !== next) {
        const quiet = attempted && !force;
        attempted = true;
        if (await load(quiet) === false) throw new Error('refresh failed');
        if (!disposed) revision = next;
      }
      connected.value = true;
    } catch { connected.value = false; }
    finally { running = false; if (queued && !disposed) { queued = false; void refresh(true); } }
  }
  const onFocus = () => { void refresh(true); };
  onMounted(() => {
    void refresh(true); timer = window.setInterval(() => { void refresh(); }, 1000);
    window.addEventListener("focus", onFocus); document.addEventListener("visibilitychange", onFocus);
  });
  onUnmounted(() => {
    disposed = true; if (timer) clearInterval(timer);
    window.removeEventListener("focus", onFocus); document.removeEventListener("visibilitychange", onFocus);
  });
  return { liveConnected: connected };
}
