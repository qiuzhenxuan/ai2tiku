<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

defineProps<{
  title: string;
  items: { label: string; value: number; hint?: string }[];
}>();
</script>

<template>
  <div class="card">
    <div class="card__head">
      <AppIcon name="chart" :size="15" />
      <span class="card__title">{{ title }}</span>
    </div>
    <div class="card__body stack" style="gap: 12px">
      <div v-if="items.length === 0" class="empty">暂无数据</div>
      <div v-for="item in items" :key="item.label" class="bar-item">
        <div class="bar-item__top">
          <span class="bar-item__label">{{ item.label }}</span>
          <span class="bar-item__value">{{ item.hint ?? item.value }}</span>
        </div>
        <div class="bar-item__track">
          <div
            class="bar-item__fill"
            :style="{ width: `${Math.max(3, (item.value / Math.max(...items.map((i) => i.value), 1)) * 100)}%` }"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bar-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bar-item__top {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 12.5px;
}

.bar-item__label {
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bar-item__value {
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.bar-item__track {
  height: 6px;
  border-radius: var(--r-full);
  background: var(--surface-active);
  overflow: hidden;
}

.bar-item__fill {
  height: 100%;
  border-radius: var(--r-full);
  background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #7ad0ff));
  transition: width 400ms var(--ease);
}
</style>
