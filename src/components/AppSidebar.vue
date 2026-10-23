<script setup lang="ts">
import { ref } from "vue";
import { routes } from "../router";
import AppIcon from "./AppIcon.vue";
import AppLogo from "./AppLogo.vue";
import RocketMark from "./RocketMark.vue";

defineProps<{ expanded: boolean }>();
const emit = defineEmits<{ toggle: [] }>();

function toggle() {
  hovered.value = "";
  focused.value = "";
  dismissed.value = true;
  emit("toggle");
}

const ICON_MAP: Record<string, string> = {
  dashboard: "dashboard",
  deploy: "rocket",
  ocs: "plug",
  providers: "sparkles",
  questions: "database",
  records: "list",
  stats: "chart",
  settings: "settings",
};

const navItems = routes.map((r) => ({
  path: r.path as string,
  name: r.name as string,
  title: (r.meta?.title as string) ?? "",
  desc: (r.meta?.desc as string) ?? "",
  icon: ICON_MAP[r.name as string] ?? "info",
}));

const version = __APP_VERSION__;
const hovered = ref("");
const focused = ref("");
const inputMode = ref<'pointer' | 'keyboard'>('pointer');
const dismissed = ref(false);
const isOpen = (name: string) => !dismissed.value && (inputMode.value === 'keyboard' ? focused.value : hovered.value) === name;

function enter(name: string, keyboard = false) {
  inputMode.value = keyboard ? 'keyboard' : 'pointer';
  if (keyboard) focused.value = name;
  else hovered.value = name;
  dismissed.value = false;
}

function focusItem(event: FocusEvent, name: string) {
  focused.value = name;
  if ((event.currentTarget as HTMLElement).matches(':focus-visible')) enter(name, true);
}

function pointerMove(name: string) {
  if (inputMode.value === 'keyboard') enter(name);
}

function moveFocus(event: KeyboardEvent) {
  const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
  if (!keys.includes(event.key)) return;
  const nav = event.currentTarget as HTMLElement;
  const links = Array.from(nav.querySelectorAll<HTMLAnchorElement>(".rail__item"));
  const index = links.indexOf(document.activeElement as HTMLAnchorElement);
  if (index < 0) return;
  inputMode.value = 'keyboard';
  event.preventDefault();
  const next = event.key === "Home" ? 0 : event.key === "End" ? links.length - 1
    : (index + (event.key === "ArrowDown" ? 1 : -1) + links.length) % links.length;
  links[next]?.focus();
}
</script>

<template>
  <aside class="rail" :class="{ 'rail--expanded': expanded }" aria-label="应用菜单">
    <div class="rail__brand" title="ai2tiku · OCS 智能题库" aria-label="ai2tiku · 无限墨环">
      <span class="rail__brand-mark"><AppLogo /></span>
      <span class="rail__brand-copy" :aria-hidden="!expanded"><strong>ai2tiku</strong><small>AI 题库</small></span>
    </div>
    <button class="rail__toggle" type="button" :aria-expanded="expanded" aria-controls="sidebar-navigation"
      :aria-label="expanded ? '收起菜单' : '展开菜单'" :title="expanded ? '收起菜单' : '展开菜单'" @click="toggle">
      <span class="rail__glyph">
        <svg class="rail__toggle-symbol" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path class="rail__toggle-divider" d="M9 4v16" />
          <path class="rail__toggle-arrow" :d="expanded ? 'm16 9-3 3 3 3' : 'm13 9 3 3-3 3'" />
        </svg>
      </span>
      <span class="rail__label" :aria-hidden="!expanded">收起菜单</span>
    </button>

    <nav id="sidebar-navigation" class="rail__nav" aria-label="主导航" @keydown="moveFocus" @keydown.esc="dismissed = true">
      <RouterLink
        v-for="item in navItems"
        :key="item.name"
        :to="item.path"
        custom
        v-slot="{ href, navigate, isExactActive }"
      >
        <div
          class="rail__wrap"
          :class="{ 'is-open': isOpen(item.name) }"
          @mouseenter="enter(item.name)"
          @mousemove="pointerMove(item.name)"
          @mouseleave="hovered = ''"
        >
          <a
            :href="href"
            class="rail__item"
            :class="[{ 'is-active': isExactActive }, `rail__item--${item.icon}`]"
            :aria-label="item.title"
            :aria-current="isExactActive ? 'page' : undefined"
            :aria-describedby="isOpen(item.name) ? `nav-tip-${item.name}` : undefined"
            @focus="focusItem($event, item.name)"
            @blur="focused = ''"
            @click="dismissed = true; navigate($event)"
          >
            <span class="rail__glyph" aria-hidden="true">
              <RocketMark v-if="item.name === 'deploy'" />
              <AppIcon v-else :name="item.icon" :size="20" />
            </span>
            <span class="rail__label" :aria-hidden="!expanded">{{ item.title }}</span>
          </a>

          <div :id="`nav-tip-${item.name}`" class="rail__flyout" role="tooltip" :aria-hidden="!isOpen(item.name)">
            <div class="rail__flyout-inner">
              <div class="rail__flyout-title">{{ item.title }}</div>
              <div class="rail__flyout-desc">{{ item.desc }}</div>
            </div>
          </div>
        </div>
      </RouterLink>
    </nav>

    <div class="rail__foot" :title="`版本 ${version}`">v{{ version }}</div>
  </aside>
</template>
