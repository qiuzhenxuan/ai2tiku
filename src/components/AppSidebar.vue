<script setup lang="ts">
import { routes } from "../router";
import AppIcon from "./AppIcon.vue";

const ICON_MAP: Record<string, string> = {
  dashboard: "dashboard",
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
</script>

<template>
  <aside class="rail">
    <div class="rail__brand" title="ai2tiku · OCS 智能题库">AI</div>

    <nav class="rail__nav">
      <RouterLink
        v-for="item in navItems"
        :key="item.name"
        :to="item.path"
        custom
        v-slot="{ navigate, isActive }"
      >
        <div class="rail__wrap">
          <button
            type="button"
            class="rail__item"
            :class="{
              'is-active': isActive,
              'rail__item--spin': item.name === 'settings',
            }"
            :aria-label="item.title"
            @click="navigate"
          >
            <AppIcon :name="item.icon" :size="18" />
          </button>

          <div class="rail__flyout" role="tooltip">
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
