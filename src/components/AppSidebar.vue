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
        <button
          type="button"
          class="rail__item"
          :class="{ 'is-active': isActive }"
          :title="item.title"
          :aria-label="item.title"
          @click="navigate"
        >
          <AppIcon :name="item.icon" :size="18" />
        </button>
      </RouterLink>
    </nav>

    <div class="rail__foot" :title="`版本 ${version}`">v{{ version }}</div>
  </aside>
</template>
