import { ref } from "vue";
import { api } from "../api";

export type Theme = "light" | "dark";

const STORAGE_KEY = "ai2tiku:theme";
const PREF_KEY = "theme";

function isTheme(v: unknown): v is Theme {
  return v === "light" || v === "dark";
}

/** 本地缓存只用来让首屏不闪，真正的记忆在数据库里 */
function cached(): Theme | null {
  const saved = localStorage.getItem(STORAGE_KEY);
  return isTheme(saved) ? saved : null;
}

/** 没有存过任何选择时跟随系统 */
function detect(): Theme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

const theme = ref<Theme>(cached() ?? detect());

function apply(value: Theme) {
  document.documentElement.dataset.theme = value;
}

apply(theme.value);

/**
 * 启动时从数据库同步一次：
 * - 数据库有值 → 以数据库为准
 * - 数据库没值 → 把当前选择（本地缓存 / 系统偏好）写进数据库
 */
export async function hydrateTheme() {
  try {
    const stored = await api.getUiPref(PREF_KEY);
    if (isTheme(stored)) {
      theme.value = stored;
      apply(stored);
      localStorage.setItem(STORAGE_KEY, stored);
    } else {
      await api.setUiPref(PREF_KEY, theme.value);
    }
  } catch {
    /* 浏览器预览等非 Tauri 环境忽略 */
  }
}

/** 日夜主题切换，选择同时写入数据库与本地缓存 */
export function useTheme() {
  function setTheme(value: Theme) {
    theme.value = value;
    apply(value);
    localStorage.setItem(STORAGE_KEY, value);
    api.setUiPref(PREF_KEY, value).catch(() => {});
  }

  function toggle() {
    setTheme(theme.value === "dark" ? "light" : "dark");
  }

  return { theme, setTheme, toggle };
}
