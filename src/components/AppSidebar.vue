<script setup lang="ts">
import { ref } from "vue";
import { routes } from "../router";
import AppIcon from "./AppIcon.vue";
import AppLogo from "./AppLogo.vue";

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
const dismissed = ref(false);
const isOpen = (name: string) => !dismissed.value && (hovered.value || focused.value) === name;

function enter(name: string, keyboard = false) {
  if (keyboard) focused.value = name;
  else hovered.value = name;
  dismissed.value = false;
}

function moveFocus(event: KeyboardEvent) {
  const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
  if (!keys.includes(event.key)) return;
  const nav = event.currentTarget as HTMLElement;
  const links = Array.from(nav.querySelectorAll<HTMLAnchorElement>(".rail__item"));
  const index = links.indexOf(document.activeElement as HTMLAnchorElement);
  if (index < 0) return;
  event.preventDefault();
  const next = event.key === "Home" ? 0 : event.key === "End" ? links.length - 1
    : (index + (event.key === "ArrowDown" ? 1 : -1) + links.length) % links.length;
  links[next]?.focus();
}
</script>

<template>
  <aside class="rail">
    <div class="rail__brand" title="ai2tiku · OCS 智能题库" aria-label="ai2tiku · 无限墨环"><AppLogo /></div>

    <nav class="rail__nav" aria-label="主导航" @keydown="moveFocus" @keydown.esc="dismissed = true">
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
          @mouseleave="hovered = ''"
        >
          <a
            :href="href"
            class="rail__item"
            :class="[{ 'is-active': isExactActive }, `rail__item--${item.icon}`]"
            :aria-label="item.title"
            :aria-current="isExactActive ? 'page' : undefined"
            :aria-describedby="isOpen(item.name) ? `nav-tip-${item.name}` : undefined"
            @focus="enter(item.name, true)"
            @blur="focused = ''"
            @click="navigate"
          >
            <!-- 火箭与喷口共用坐标系，升空时粒子始终从尾部发出。 -->
            <span v-if="item.name === 'deploy'" class="rail__rocket" aria-hidden="true">
              <AppIcon :name="item.icon" :size="18" />
              <span class="rail__thrust">
                <i
                  v-for="(dx, i) in [-3, -1.5, -0.5, 0.5, 1.5, 3]"
                  :key="i"
                  :style="{ '--dx': `${dx}px`, '--d': `${-i * 137}ms` }"
                />
              </span>
            </span>
            <AppIcon v-else :name="item.icon" :size="18" />
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
