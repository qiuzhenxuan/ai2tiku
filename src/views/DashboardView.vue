<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "../api";
import type { SearchRecord, ServerStatus, Stats } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import BarList from "../components/BarList.vue";
import StatCard from "../components/StatCard.vue";
import { useToast } from "../composables/useToast";
import { compactNumber, formatDate, formatPercent, sourceLabel } from "../utils/format";

const toast = useToast();
const stats = ref<Stats | null>(null);
const status = ref<ServerStatus | null>(null);
const recent = ref<SearchRecord[]>([]);
const busy = ref(false);

async function load() {
  try {
    const [s, st, rec] = await Promise.all([
      api.getStats(),
      api.serverStatus(),
      api.listRecords(1, 6),
    ]);
    stats.value = s;
    status.value = st;
    recent.value = rec.items;
  } catch (e) {
    toast.fromError(e);
  }
}

async function toggleServer() {
  busy.value = true;
  try {
    const next = status.value?.running ? await api.stopServer() : await api.startServer();
    status.value = next;
    toast.ok(next.running ? "服务已启动" : "服务已停止");
  } catch (e) {
    toast.fromError(e);
  } finally {
    busy.value = false;
  }
}

onMounted(load);
</script>

<template>
  <div class="page">
    <div class="grid grid--4">
      <StatCard
        label="今日搜题"
        :value="stats?.todayCount ?? 0"
        :foot="`Token ${compactNumber(stats?.todayTokens ?? 0)}`"
        icon="bolt"
      />
      <StatCard
        label="题库总量"
        :value="stats?.questionTotal ?? 0"
        foot="道已缓存题目"
        icon="database"
      />
      <StatCard
        label="缓存命中率"
        :value="formatPercent(stats?.hitRate ?? 0)"
        :foot="`命中 ${stats?.cacheHits ?? 0} · AI ${stats?.aiCalls ?? 0}`"
        accent
        icon="check"
      />
      <StatCard
        label="Token 总消耗"
        :value="compactNumber(stats?.totalTokens ?? 0)"
        :foot="`输入 ${compactNumber(stats?.promptTokens ?? 0)} · 输出 ${compactNumber(stats?.completionTokens ?? 0)}`"
        icon="sparkles"
      />
    </div>

    <div class="split">
      <div class="card">
        <div class="card__head">
          <AppIcon name="plug" :size="15" />
          <span class="card__title">本地题库服务</span>
          <button
            class="btn btn--sm"
            style="margin-left: auto"
            type="button"
            :disabled="busy"
            @click="toggleServer"
          >
            <AppIcon :name="status?.running ? 'close' : 'power'" :size="13" />
            {{ status?.running ? "停止服务" : "启动服务" }}
          </button>
        </div>
        <div class="card__body stack">
          <div class="row row--between">
            <span class="faint small">接口地址</span>
            <span class="mono">{{ status?.address ?? "-" }}/api/search</span>
          </div>
          <div class="row row--between">
            <span class="faint small">监听范围</span>
            <span class="mono">127.0.0.1（仅本机可访问）</span>
          </div>
          <div class="row row--between">
            <span class="faint small">当前状态</span>
            <span class="badge" :class="status?.running ? 'badge--ok' : 'badge--off'">
              <span class="dot" :class="{ 'dot--pulse': status?.running }" />
              {{ status?.running ? "运行中" : "已停止" }}
            </span>
          </div>
          <RouterLink to="/ocs" class="btn btn--primary" style="justify-content: center">
            <AppIcon name="copy" :size="14" />
            生成 OCS 题库配置
          </RouterLink>
        </div>
      </div>

      <BarList
        title="模型调用分布"
        :items="(stats?.modelUsage ?? []).map((m) => ({ label: m.model, value: m.count, hint: `${m.count} 次 · ${compactNumber(m.tokens)} token` }))"
      />
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="list" :size="15" />
        <span class="card__title">最近搜题</span>
        <RouterLink to="/records" class="btn btn--ghost btn--sm" style="margin-left: auto">
          查看全部
        </RouterLink>
      </div>
      <div v-if="recent.length === 0" class="empty">
        <div class="empty__title">还没有搜题记录</div>
        配好 AI 供应商，再到 OCS 里发起一次答题即可
      </div>
      <div v-else class="table-wrap">
        <table class="table">
          <thead>
            <tr>
              <th style="width: 44%">题目</th>
              <th>来源</th>
              <th>模型</th>
              <th class="num">Token</th>
              <th class="num">时间</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in recent" :key="r.id">
              <td><div class="cell-clip">{{ r.title }}</div></td>
              <td>
                <span class="badge" :class="r.source === 'cache' ? 'badge--ok' : 'badge--ai'">
                  {{ sourceLabel(r.source) }}
                </span>
              </td>
              <td class="mono">{{ r.model || "-" }}</td>
              <td class="num">{{ r.totalTokens || "-" }}</td>
              <td class="num faint small">{{ formatDate(r.createdAt) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
