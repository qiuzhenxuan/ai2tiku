<script setup lang="ts">
import { ref } from "vue";
import { api } from "../api";
import type { Stats } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import BarList from "../components/BarList.vue";
import LineChart from "../components/LineChart.vue";
import StatCard from "../components/StatCard.vue";
import { useToast } from "../composables/useToast";
import { useLiveRefresh } from "../composables/useLiveRefresh";
import { compactNumber, formatNumber, formatPercent } from "../utils/format";

const toast = useToast();
const stats = ref<Stats | null>(null);
const loading = ref(false);
let requestId = 0;

const countPoints = ref<{ label: string; value: number }[]>([]);
const tokenPoints = ref<{ label: string; value: number }[]>([]);

async function load(quiet = false) {
  const request = ++requestId;
  if (!quiet) loading.value = true;
  try {
    const s = await api.getStats();
    if (request !== requestId) return;
    stats.value = s;
    countPoints.value = s.trend.map((p) => ({ label: p.date.slice(5), value: p.count }));
    tokenPoints.value = s.trend.map((p) => ({ label: p.date.slice(5), value: p.tokens }));
  } catch (e) {
    if (!quiet) toast.fromError(e);
    return false;
  } finally {
    if (request === requestId) loading.value = false;
  }
}

useLiveRefresh(load);
</script>

<template>
  <div class="page">
    <div class="grid grid--3"><StatCard label="成功请求" :value="formatNumber(stats?.successCount ?? 0)" icon="check"/><StatCard label="失败请求" :value="formatNumber(stats?.failureCount ?? 0)" icon="info"/><StatCard label="答题成功率" :value="formatPercent(stats?.successRate ?? 0)" :foot="'缓存命中率按全部已记录请求计算'" icon="bolt"/></div>
    <div class="grid grid--4">
      <StatCard label="题库题目" :value="formatNumber(stats?.questionTotal ?? 0)" icon="database" />
      <StatCard label="AI 请求次数" :value="formatNumber(stats?.aiCalls ?? 0)" icon="sparkles" />
      <StatCard label="缓存命中次数" :value="formatNumber(stats?.cacheHits ?? 0)" accent icon="check" />
      <StatCard label="缓存命中率" :value="formatPercent(stats?.hitRate ?? 0)" icon="bolt" />
    </div>

    <div class="grid grid--3">
      <StatCard label="输入 Token" :value="compactNumber(stats?.promptTokens ?? 0)" icon="info" />
      <StatCard label="输出 Token" :value="compactNumber(stats?.completionTokens ?? 0)" icon="sparkles" />
      <StatCard
        label="合计消耗"
        :value="compactNumber(stats?.totalTokens ?? 0)"
        :foot="formatNumber(stats?.totalTokens ?? 0) + ' tokens'"
        accent
        icon="bolt"
      />
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="chart" :size="15" />
        <span class="card__title">近 14 天搜题量</span>
        <button class="btn btn--ghost btn--sm" style="margin-left: auto" type="button" @click="load()">
          <AppIcon name="refresh" :size="13" :class="{ spin: loading }" />
          刷新
        </button>
      </div>
      <div class="card__body">
        <LineChart :points="countPoints" :height="188" unit=" 次" />
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="bolt" :size="15" />
        <span class="card__title">近 14 天 Token 消耗</span>
      </div>
      <div class="card__body">
        <LineChart :points="tokenPoints" :height="188" unit=" token" />
      </div>
    </div>

    <BarList
      title="按模型统计"
      :items="
        (stats?.modelUsage ?? []).map((m) => ({
          label: m.model,
          value: m.count,
          hint: `${m.count} 次 · ${compactNumber(m.tokens)} token`,
        }))
      "
    />
  </div>
</template>
