<script setup lang="ts">
import { computed } from "vue";
import { useTheme } from "../composables/useTheme";
import AppIcon from "./AppIcon.vue";

const { theme, setTheme } = useTheme();

const options = computed(() => [
  { value: "light" as const, label: "日间", icon: "sun" },
  { value: "dark" as const, label: "夜间", icon: "moon" },
]);
</script>

<template>
  <div class="segmented" role="group" aria-label="主题切换">
    <button
      v-for="opt in options"
      :key="opt.value"
      class="segmented__btn"
      :class="{ 'is-active': theme === opt.value }"
      type="button"
      :aria-pressed="theme === opt.value"
      @click="setTheme(opt.value)"
    >
      <AppIcon :name="opt.icon" :size="14" />
      {{ opt.label }}
    </button>
  </div>
</template>
