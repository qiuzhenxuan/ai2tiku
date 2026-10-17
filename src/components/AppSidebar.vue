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
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <div class="brand__mark">AI</div>
      <div>
        <div class="brand__name">ai2tiku</div>
        <div class="brand__sub">OCS 智能题库</div>
      </div>
    </div>

    <nav class="nav">
      <RouterLink
        v-for="item in navItems"
        :key="item.name"
        :to="item.path"
        custom
        v-slot="{ navigate, isActive }"
      >
        <button
          type="button"
          class="nav__item"
          :class="{ 'is-active': isActive }"
          @click="navigate"
        >
          <AppIcon :name="item.icon" :size="16" class="nav__icon" />
          {{ item.title }}
        </button>
      </RouterLink>
    </nav>

    <div class="sidebar__foot">
      本地运行 · 仅监听 127.0.0.1
    </div>
  </aside>
</template>
