<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

defineProps<{
  label: string;
  value: string | number;
  foot?: string;
  accent?: boolean;
  icon?: string;
}>();
</script>

<template>
  <div class="card stat">
    <div class="stat__label">
      <AppIcon v-if="icon" :name="icon" :size="13" />
      {{ label }}
    </div>
    <div class="stat__value" :class="{ 'stat__accent': accent }">{{ value }}</div>
    <div v-if="foot" class="stat__foot">{{ foot }}</div>
  </div>
</template>
