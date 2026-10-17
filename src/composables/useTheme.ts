import { ref } from "vue";

export type Theme = "light" | "dark";

const STORAGE_KEY = "ai2tiku:theme";

function detect(): Theme {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === "light" || saved === "dark") return saved;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

const theme = ref<Theme>(detect());

function apply(value: Theme) {
  document.documentElement.dataset.theme = value;
}

apply(theme.value);

/** 日夜主题切换，选择结果会记住 */
export function useTheme() {
  function setTheme(value: Theme) {
    theme.value = value;
    localStorage.setItem(STORAGE_KEY, value);
    apply(value);
  }

  function toggle() {
    setTheme(theme.value === "dark" ? "light" : "dark");
  }

  return { theme, setTheme, toggle };
}
