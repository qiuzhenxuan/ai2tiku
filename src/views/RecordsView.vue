<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api } from "../api";
import type { SearchRecord } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import QuestionDetails from "../components/QuestionDetails.vue";
import StatsPanel from "../components/StatsPanel.vue";
import { useConfirm } from "../composables/useConfirm";
import { useLiveRefresh } from "../composables/useLiveRefresh";
import { useToast } from "../composables/useToast";
import { formatDate, sourceLabel } from "../utils/format";

const toast = useToast();
const route = useRoute();
const router = useRouter();
type Tab = "list" | "stats";
const tab = ref<Tab>(route.query.tab === "stats" ? "stats" : "list");
watch(() => route.query.tab, (v) => { tab.value = v === "stats" ? "stats" : "list"; });
function setTab(t: Tab) { router.replace({ query: t === "stats" ? { tab: "stats" } : {} }); }
const statusFilter = ref<"" | "success" | "failure">("");
const shown = computed(() => statusFilter.value ? items.value.filter((r) => r.status === statusFilter.value) : items.value);
const confirm = useConfirm();
const selected = ref<SearchRecord | null>(null);
const clearing = ref(false);
let requestId = 0;

const items = ref<SearchRecord[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 20;
const loading = ref(false);

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));

async function load(quiet = false) {
  const request = ++requestId;
  if (!quiet) loading.value = true;
  try {
    const res = await api.listRecords(page.value, pageSize);
    if (request !== requestId) return;
    if (page.value > 1 && res.items.length === 0) { page.value = Math.max(1, Math.ceil(res.total / pageSize)); await load(quiet); return; }
    items.value = res.items;
    total.value = res.total;
  } catch (e) {
    if (!quiet) toast.fromError(e);
    return false;
  } finally {
    if (request === requestId) loading.value = false;
  }
}

function go(delta: number) {
  const next = page.value + delta;
  if (next < 1 || next > totalPages.value) return;
  page.value = next;
  load();
}

async function clearAll() {
  if (clearing.value) return;
  clearing.value = true;
  try {
    const all = await api.listRecords(1, 1);
    if (!await confirm({ title: '清空答题记录？', message: `将清空全部 ${all.total} 条记录，相应的答题次数与 Token 统计也会清零；题库中的题目保留。`, confirmText: '清空全部记录', danger: true })) return;
    await api.clearRecords();
    toast.ok("记录已清空");
    page.value = 1;
    await load();
  } catch (e) {
    toast.fromError(e);
  } finally { clearing.value = false; }
}

const { liveConnected } = useLiveRefresh(load);
</script>

<template>
  <div class="page">
    <div class="tabs" role="tablist">
      <button class="tabs__btn" :class="{ 'is-active': tab === 'list' }" role="tab" :aria-selected="tab === 'list'" type="button" @click="setTab('list')">
        <AppIcon name="list" :size="14" />答题记录<span class="tabs__count">{{ total }}</span>
      </button>
      <button class="tabs__btn" :class="{ 'is-active': tab === 'stats' }" role="tab" :aria-selected="tab === 'stats'" type="button" @click="setTab('stats')">
        <AppIcon name="chart" :size="14" />统计看板
      </button>
    </div>
    <StatsPanel v-if="tab === 'stats'" />
    <div v-else class="card">
      <div class="card__head">
        <AppIcon name="list" :size="15" />
        <div class="segmented segmented--sm" role="group" aria-label="按状态筛选本页">
          <button v-for="o in [{ v: '', l: '全部' }, { v: 'success', l: '成功' }, { v: 'failure', l: '失败' }]" :key="o.v" class="segmented__btn" :class="{ 'is-active': statusFilter === o.v }" type="button" @click="statusFilter = o.v as typeof statusFilter">{{ o.l }}</button>
        </div>
        <span class="live-tag" :class="{ 'is-off': !liveConnected }">
          <span class="dot" :class="{ 'dot--pulse': liveConnected }" />
          {{ liveConnected ? "实时更新" : "连接中断，重试中" }}
        </span>
        <div class="row" style="margin-left: auto">
          <button class="btn btn--sm" type="button" @click="load()">
            <AppIcon name="refresh" :size="13" :class="{ spin: loading }" />
            刷新
          </button>
          <button class="btn btn--danger btn--sm" type="button" :disabled="clearing" @click="clearAll">清空记录</button>
        </div>
      </div>

      <div v-if="shown.length === 0" class="empty">
        <div class="empty__title">{{ loading ? "加载中…" : items.length ? "本页没有符合筛选的记录" : "还没有答题记录" }}</div>
        每一次 OCS 搜题或自测都会记录在这里
      </div>
      <template v-else>
        <div class="table-wrap">
          <table class="table table--records">
            <thead>
              <tr>
                <th style="width: 30%">题目</th>
                <th style="width: 18%">答案</th>
                <th>来源</th>
                <th>状态</th>
                <th>模型</th>
                <th class="num">Token</th>
                <th class="num">耗时</th>
                <th class="num">时间</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="r in shown" :key="r.id">
                <td><button class="cell-link" type="button" :aria-label="`查看答题详情：${r.title || '空题目'}`" @click="selected = r"><span class="cell-clip">{{ r.title || '（空题目）' }}</span></button><span v-if="r.error" class="record-error cell-clip">{{ r.error }}</span></td>
                <td><div class="mono cell-clip">{{ r.answer || "-" }}</div></td>
                <td>
                  <span class="badge" :class="r.source === 'cache' ? 'badge--ok' : 'badge--ai'">
                    {{ sourceLabel(r.source) }}
                  </span>
                </td>
                <td><span class="badge" :class="r.status === 'failure' ? 'badge--warn' : 'badge--ok'">{{ r.status === 'failure' ? '失败' : '成功' }}</span></td>
                <td class="mono faint small">{{ r.model || "-" }}</td>
                <td class="num">{{ r.totalTokens || "-" }}</td>
                <td class="num faint">{{ r.durationMs }}ms</td>
                <td class="num faint small nowrap">{{ formatDate(r.createdAt) }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="pager">
          <span>共 {{ total }} 条 · 第 {{ page }} / {{ totalPages }} 页</span>
          <button class="btn btn--sm" type="button" :disabled="page <= 1" @click="go(-1)">上一页</button>
          <button class="btn btn--sm" type="button" :disabled="page >= totalPages" @click="go(1)">下一页</button>
        </div>
      </template>
    </div>
    <QuestionDetails :record="selected" @close="selected = null" />
  </div>
</template>
