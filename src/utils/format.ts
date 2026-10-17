/** 展示层格式化工具 */

export function formatNumber(n: number | null | undefined): string {
  if (n === null || n === undefined) return "0";
  return n.toLocaleString("zh-CN");
}

/** 大数字缩写：1234 → 1.2k，1250000 → 1.25M */
export function compactNumber(n: number | null | undefined): string {
  const v = n ?? 0;
  if (v < 1000) return String(v);
  if (v < 1_000_000) return `${(v / 1000).toFixed(v < 10_000 ? 2 : 1)}k`;
  return `${(v / 1_000_000).toFixed(2)}M`;
}

export function formatPercent(ratio: number | null | undefined, digits = 1): string {
  const v = ratio ?? 0;
  return `${(v * 100).toFixed(digits)}%`;
}

export function formatDate(value: string): string {
  if (!value) return "-";
  return value.length > 16 ? value.slice(0, 16) : value;
}

/** 题型标识转中文 */
export function typeLabel(qtype: string): string {
  const map: Record<string, string> = {
    single: "单选",
    multiple: "多选",
    judgement: "判断",
    completion: "填空",
  };
  return map[qtype] ?? "未知";
}

export function sourceLabel(source: string): string {
  const map: Record<string, string> = {
    cache: "本地题库",
    ai: "AI 作答",
    manual: "手动录入",
    none: "未命中",
  };
  return map[source] ?? source;
}
