<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "../api";
import type { SearchRecord } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import { useToast } from "../composables/useToast";
import { formatDate, sourceLabel } from "../utils/format";

const toast = useToast();

const items = ref<SearchRecord[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 20;
const loading = ref(false);

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));

async function load() {
  loading.value = true;
  try {
    const res = await api.listRecords(page.value, pageSize);
    items.value = res.items;
    total.value = res.total;
  } catch (e) {
    toast.fromError(e);
  } finally {
    loading.value = false;
  }
}

function go(delta: number) {
  const next = page.value + delta;
  if (next < 1 || next > totalPages.value) return;
  page.value = next;
  load();
}

async function clearAll() {
  if (!window.confirm(`清空全部 ${total.value} 条记录？统计数字也会一并清零。`)) return;
  try {
    await api.clearRecords();
    toast.ok("记录已清空");
    page.value = 1;
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

onMounted(load);
</script>

<template>
  <div class="page">
    <div class="card">
      <div class="card__head">
        <AppIcon name="list" :size="15" />
        <span class="card__title">答题记录</span>
        <span class="card__hint">共 {{ total }} 条</span>
        <div class="row" style="margin-left: auto">
          <button class="btn btn--sm" type="button" @click="load">
            <AppIcon name="refresh" :size="13" :class="{ spin: loading }" />
            刷新
          </button>
          <button class="btn btn--danger btn--sm" type="button" @click="clearAll">清空记录</button>
        </div>
      </div>

      <div v-if="items.length === 0" class="empty">
        <div class="empty__title">{{ loading ? "加载中…" : "还没有答题记录" }}</div>
        每一次 OCS 搜题或自测都会记录在这里
      </div>
      <template v-else>
        <div class="table-wrap">
          <table class="table">
            <thead>
              <tr>
                <th style="width: 30%">题目</th>
                <th style="width: 18%">答案</th>
                <th>来源</th>
                <th>模型</th>
                <th class="num">Token</th>
                <th class="num">耗时</th>
                <th class="num">时间</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="r in items" :key="r.id">
                <td><div class="cell-clip">{{ r.title }}</div></td>
                <td><div class="mono cell-clip">{{ r.answer || "-" }}</div></td>
                <td>
                  <span class="badge" :class="r.source === 'cache' ? 'badge--ok' : 'badge--ai'">
                    {{ sourceLabel(r.source) }}
                  </span>
                </td>
                <td class="mono faint small">{{ r.model || "-" }}</td>
                <td class="num">{{ r.totalTokens || "-" }}</td>
                <td class="num faint">{{ r.durationMs }}ms</td>
                <td class="num faint small nowrap">{{ formatDate(r.createdAt) }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="pager">
          <span>第 {{ page }} / {{ totalPages }} 页</span>
          <button class="btn btn--sm" type="button" :disabled="page <= 1" @click="go(-1)">上一页</button>
          <button class="btn btn--sm" type="button" :disabled="page >= totalPages" @click="go(1)">下一页</button>
        </div>
      </template>
    </div>
  </div>
</template>
