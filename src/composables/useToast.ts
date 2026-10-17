import { ref } from "vue";

export interface Toast {
  id: number;
  message: string;
  kind: "info" | "ok" | "err";
}

const items = ref<Toast[]>([]);
let seq = 0;

function push(message: string, kind: Toast["kind"] = "info", duration = 2800) {
  const id = ++seq;
  items.value.push({ id, message, kind });
  window.setTimeout(() => {
    items.value = items.value.filter((t) => t.id !== id);
  }, duration);
}

/** 全局轻提示 */
export function useToast() {
  return {
    items,
    info: (m: string) => push(m, "info"),
    ok: (m: string) => push(m, "ok"),
    error: (m: string) => push(m, "err", 4000),
    /** 统一处理来自后端的字符串错误 */
    fromError: (e: unknown) => push(typeof e === "string" ? e : String(e), "err", 4000),
  };
}
