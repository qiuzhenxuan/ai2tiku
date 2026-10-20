import { ref } from "vue";

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

function detect(): AccentId {
  const saved = localStorage.getItem(STORAGE_KEY);
  return ACCENT_PRESETS.some((p) => p.id === saved) ? (saved as AccentId) : "blue";
}

const accent = ref<AccentId>(detect());

function apply(id: AccentId) {
  document.documentElement.dataset.accent = id;
}

apply(accent.value);

/** 主题色自选，颜色由 CSS 的 [data-accent] 变量接管，选择结果会记住 */
export function useAccent() {
  function setAccent(id: AccentId) {
    accent.value = id;
    localStorage.setItem(STORAGE_KEY, id);
    apply(id);
  }

  return { accent, setAccent };
}
