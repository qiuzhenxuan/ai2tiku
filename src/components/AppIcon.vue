<script setup lang="ts">
/** 统一的线性图标集，颜色跟随 currentColor，自动适配日夜主题 */
const ICONS: Record<string, string> = {
  dashboard:
    '<rect x="3" y="3" width="7.5" height="9.5" rx="1.8"/><rect x="14" y="3" width="7" height="5.5" rx="1.8"/><rect x="14" y="12" width="7" height="9" rx="1.8"/><rect x="3" y="16.5" width="7.5" height="4.5" rx="1.8"/>',
  plug: '<path d="M9 2v6"/><path d="M15 2v6"/><path d="M6 8h12v3a6 6 0 0 1-12 0V8Z"/><path d="M12 17v5"/>',
  sparkles:
    '<path d="M12 3l1.7 4.6L18 9.3l-4.3 1.7L12 15.6l-1.7-4.6L6 9.3l4.3-1.7L12 3Z"/><path d="M18.5 15.4l.8 2.3 2.2.8-2.2.8-.8 2.3-.8-2.3-2.2-.8 2.2-.8.8-2.3Z"/>',
  database:
    '<ellipse cx="12" cy="5.5" rx="7.5" ry="3"/><path d="M4.5 5.5v13c0 1.7 3.4 3 7.5 3s7.5-1.3 7.5-3v-13"/><path d="M4.5 12c0 1.7 3.4 3 7.5 3s7.5-1.3 7.5-3"/>',
  list: '<path d="M9 6h12"/><path d="M9 12h12"/><path d="M9 18h12"/><circle cx="4" cy="6" r="1.2"/><circle cx="4" cy="12" r="1.2"/><circle cx="4" cy="18" r="1.2"/>',
  chart:
    '<path d="M3 21h18"/><rect x="4.5" y="12" width="3.6" height="6" rx="1.2"/><rect x="10.2" y="7" width="3.6" height="11" rx="1.2"/><rect x="15.9" y="14" width="3.6" height="4" rx="1.2"/>',
  settings:
    '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2v.3a2 2 0 1 1-4 0v-.2a1.7 1.7 0 0 0-2.9-1.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3 15a2 2 0 1 1 0-4h.2A1.7 1.7 0 0 0 4.5 8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 10 3.9V3.7a2 2 0 1 1 4 0v.2a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0 1.2 2.9h.2a2 2 0 1 1 0 4h-.2a1.7 1.7 0 0 0-1.5 1Z"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="M4.9 4.9l1.4 1.4"/><path d="M17.7 17.7l1.4 1.4"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="M4.9 19.1l1.4-1.4"/><path d="M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M20.5 14.5A8.5 8.5 0 0 1 9.5 3.5a8.5 8.5 0 1 0 11 11Z"/>',
  plus: '<path d="M12 5v14"/><path d="M5 12h14"/>',
  trash:
    '<path d="M4 7h16"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M6 7l1 13h10l1-13"/><path d="M9 7V4h6v3"/>',
  edit: '<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/>',
  refresh: '<path d="M21 12a9 9 0 1 1-2.6-6.4"/><path d="M21 3v6h-6"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2.2"/><path d="M5 15H4.2A2.2 2.2 0 0 1 2 12.8V4.2A2.2 2.2 0 0 1 4.2 2h8.6A2.2 2.2 0 0 1 15 4.2V5"/>',
  play: '<path d="M7 4.5l12 7.5-12 7.5V4.5Z"/>',
  check: '<path d="M20 6L9 17l-5-5"/>',
  bolt: '<path d="M13 2L4 14h6l-1 8 9-12h-6l1-8Z"/>',
  close: '<path d="M18 6L6 18"/><path d="M6 6l12 12"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="M20.5 20.5L16.7 16.7"/>',
  power: '<path d="M12 3v9"/><path d="M6.3 6.3a8 8 0 1 0 11.4 0"/>',
  globe:
    '<circle cx="12" cy="12" r="9"/><path d="M3.5 9h17"/><path d="M3.5 15h17"/><path d="M12 3a14 14 0 0 1 0 18"/><path d="M12 3a14 14 0 0 0 0 18"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><path d="M12 8h.01"/>',
};

defineProps<{ name: string; size?: number }>();
</script>

<template>
  <svg
    :width="size ?? 16"
    :height="size ?? 16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.9"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    v-html="ICONS[name] ?? ''"
  />
</template>
