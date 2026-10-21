import { ref } from "vue";
import { api } from "../api";

export type AccentId =
  | "blue"
  | "purple"
  | "green"
  | "teal"
  | "orange"
  | "pink"
  | "red"
  | "graphite";

export interface AccentPreset {
  id: AccentId;
  label: string;
  /** 日间主题下的强调色 */
  light: string;
  /** 夜间主题下的强调色（通常更亮一档，避免在深色背景上发闷） */
  dark: string;
}

/** 参考 ChatGPT 手机端的强调色自选：一排色点，点一下立即生效 */
export const ACCENT_PRESETS: AccentPreset[] = [
  { id: "blue", label: "靛蓝", light: "#2f6fd0", dark: "#7d92ff" },
  { id: "purple", label: "罗兰紫", light: "#7c4dff", dark: "#a98bff" },
  { id: "green", label: "森林绿", light: "#0f7b45", dark: "#4cd27e" },
  { id: "teal", label: "青碧", light: "#0b7f8a", dark: "#3fd0d8" },
  { id: "orange", label: "琥珀橙", light: "#c96a10", dark: "#f0a24c" },
  { id: "pink", label: "玫红", light: "#c2185b", dark: "#ff7aa8" },
  { id: "red", label: "朱红", light: "#c42b1c", dark: "#ff7a6a" },
  { id: "graphite", label: "石墨", light: "#4a4a4a", dark: "#b0b6c0" },
];

const STORAGE_KEY = "ai2tiku:accent";
const PREF_KEY = "accent";

function isAccentId(v: unknown): v is AccentId {
  return ACCENT_PRESETS.some((p) => p.id === v);
}

/** 本地缓存只用来让首屏不闪，真正的记忆在数据库里 */
function cached(): AccentId | null {
  const saved = localStorage.getItem(STORAGE_KEY);
  return isAccentId(saved) ? saved : null;
}

const accent = ref<AccentId>(cached() ?? "blue");

function apply(id: AccentId) {
  document.documentElement.dataset.accent = id;
}

apply(accent.value);

/**
 * 启动时从数据库同步一次：
 * - 数据库有值 → 以数据库为准（换机器/清缓存也能恢复）
 * - 数据库没值但有本地缓存 → 把缓存写进数据库（老用户平滑迁移）
 */
export async function hydrateAccent() {
  try {
    const stored = await api.getUiPref(PREF_KEY);
    if (isAccentId(stored)) {
      accent.value = stored;
      apply(stored);
      localStorage.setItem(STORAGE_KEY, stored);
    } else {
      await api.setUiPref(PREF_KEY, accent.value);
    }
  } catch {
    /* 浏览器预览等非 Tauri 环境忽略 */
  }
}

/** 主题色自选，选择同时写入数据库与本地缓存，重启后依然生效 */
export function useAccent() {
  function setAccent(id: AccentId) {
    accent.value = id;
    apply(id);
    localStorage.setItem(STORAGE_KEY, id);
    api.setUiPref(PREF_KEY, id).catch(() => {});
  }

  return { accent, setAccent };
}
