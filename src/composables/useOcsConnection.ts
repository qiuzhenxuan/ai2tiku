import { computed, onMounted, onUnmounted, ref } from "vue";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api } from "../api";
import type { DeploymentStatus } from "../api/types";
import { useToast } from "./useToast";

/** 部署页和配置页共用真实后端状态；复制前重新核对，避免使用过期端口。 */
export function useOcsConnection() {
  const toast = useToast();
  const info = ref<DeploymentStatus | null>(null);
  const error = ref("");
  const loading = ref(true);
  const busy = ref(false);
  let pending: Promise<boolean> | undefined;
  let timer: number | undefined;
  let disposed = false;

  function refresh(): Promise<boolean> {
    if (pending) return pending;
    pending = (async () => {
      try {
        const next = await api.deploymentStatus();
        if (!Number.isInteger(next.server.port) || next.server.port < 1 || next.server.port > 65535) {
          throw new Error("服务端口无效，请到设置页检查监听端口");
        }
        if (disposed) return false;
        info.value = next;
        error.value = "";
        return true;
      } catch (e) {
        if (!disposed) {
          info.value = null;
          error.value = e instanceof Error ? e.message : String(e);
        }
        return false;
      } finally {
        loading.value = false;
        pending = undefined;
      }
    })();
    return pending;
  }

  const server = computed(() => info.value?.server ?? null);
  const config = computed(() => info.value?.config ?? "");
  const canCopy = computed(() => !!server.value?.running && !!config.value);
  const address = computed(() => server.value ? `http://127.0.0.1:${server.value.port}` : "");
  const aiModel = computed(() => info.value?.aiModel ?? null);
  const aiEnabled = computed(() => !!aiModel.value && !!info.value?.aiFallback);

  async function copyConfig() {
    if (busy.value) return;
    busy.value = true;
    try {
      if (!await refresh()) throw new Error(error.value || "读取题库配置失败");
      if (!canCopy.value) throw new Error("本地服务尚未启动，请先启动服务再复制配置");
      await writeText(config.value);
      toast.ok("题库 JSON 配置已复制，请粘贴到 OCS 的题库配置中");
    } catch (e) {
      toast.fromError(e);
    } finally {
      busy.value = false;
    }
  }

  async function startServer() {
    if (busy.value) return;
    busy.value = true;
    try {
      await api.startServer();
      if (!await refresh() || !server.value?.running) throw new Error(error.value || "服务未能启动");
      toast.ok("本地题库服务已启动");
    } catch (e) {
      await refresh();
      toast.fromError(e);
    } finally {
      busy.value = false;
    }
  }

  const onFocus = () => { void refresh(); };
  onMounted(() => {
    void refresh();
    timer = window.setInterval(onFocus, 3000);
    window.addEventListener("focus", onFocus);
  });
  onUnmounted(() => {
    disposed = true;
    window.clearInterval(timer);
    window.removeEventListener("focus", onFocus);
  });
  return { info, error, loading, busy, server, address, config, aiModel, aiEnabled, canCopy, refresh, copyConfig, startServer };
}
