<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "../api";
import type { SearchRecord, Stats } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import BarList from "../components/BarList.vue";
import LineChart from "../components/LineChart.vue";
import StatCard from "../components/StatCard.vue";
import QuestionDetails from "../components/QuestionDetails.vue";
import { useToast } from "../composables/useToast";
import { useLiveRefresh } from "../composables/useLiveRefresh";
import { useSetupProgress } from "../composables/useSetupProgress";
import { compactNumber, formatDate, formatPercent, sourceLabel } from "../utils/format";

const toast = useToast();
const stats = ref<Stats | null>(null);
const recent = ref<SearchRecord[]>([]);
const selected = ref<SearchRecord | null>(null);
const setup = useSetupProgress();

const trend = computed(() => (stats.value?.trend ?? []).map((p) => ({ label: p.date.slice(5), value: p.count })));
const progress = computed(() => Math.round((setup.doneCount.value / setup.steps.value.length) * 100));

async function load(quiet = false) {
  try {
    const [s, rec] = await Promise.all([api.getStats(), api.listRecords(1, 6)]);
    stats.value = s;
    recent.value = rec.items;
    void setup.load();
  } catch (e) {
    if (!quiet) toast.fromError(e);
    return false;
  }
}

const { liveConnected } = useLiveRefresh(load);
onMounted(() => void setup.load());
</script>

<template>
  <div class="page">
    <!-- 接入进度：全部完成后收起 -->
    <section v-if="setup.loaded.value && !setup.allDone.value" class="card onboard">
      <div class="onboard__head">
        <div>
          <div class="onboard__eyebrow">接入进度 · {{ setup.doneCount.value }} / {{ setup.steps.value.length }}</div>
          <h2 class="onboard__title">还差几步，就能让 OCS 自动答题</h2>
        </div>
        <RouterLink
          class="btn btn--primary"
          :to="{ path: '/setup', query: { step: setup.nextStep.value?.id } }"
        >
          继续：{{ setup.nextStep.value?.title }}
          <AppIcon name="play" :size="11" />
        </RouterLink>
      </div>
      <div class="progress" role="progressbar" :aria-valuenow="progress" aria-valuemin="0" aria-valuemax="100">
        <span class="progress__bar" :style="{ width: `${progress}%` }" />
      </div>
      <ol class="onboard__steps">
        <li v-for="(s, i) in setup.steps.value" :key="s.id">
          <RouterLink
            class="onboard__step"
            :class="{ 'is-done': s.done, 'is-next': s.id === setup.nextStep.value?.id }"
            :to="{ path: '/setup', query: { step: s.id } }"
          >
            <span class="onboard__num">
              <AppIcon v-if="s.done" name="check" :size="12" />
              <template v-else>{{ i + 1 }}</template>
            </span>
            <span class="onboard__text">
              <strong>{{ s.title }}</strong>
              <small>{{ s.hint }}</small>
            </span>
          </RouterLink>
        </li>
      </ol>
    </section>

    <div class="grid grid--4">
      <StatCard
        label="今日搜题"
        :value="stats?.todayCount ?? 0"
        :foot="`今日 Token ${compactNumber(stats?.todayTokens ?? 0)}`"
        icon="bolt"
      />
      <StatCard
        label="答题成功率"
        :value="formatPercent(stats?.successRate ?? 0)"
        :foot="`成功 ${stats?.successCount ?? 0} · 失败 ${stats?.failureCount ?? 0}`"
        icon="check"
      />
      <StatCard
        label="缓存命中率"
        :value="formatPercent(stats?.hitRate ?? 0)"
        :foot="`题库 ${stats?.questionTotal ?? 0} 道 · 命中 ${stats?.cacheHits ?? 0} 次`"
        accent
        icon="database"
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
          <AppIcon name="chart" :size="15" />
          <span class="card__title">近 14 天搜题量</span>
          <RouterLink class="btn btn--ghost btn--sm" style="margin-left: auto" :to="{ path: '/records', query: { tab: 'stats' } }">
            详细统计
          </RouterLink>
        </div>
        <div class="card__body">
          <LineChart :points="trend" :height="170" unit=" 次" />
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
        <span class="live-tag" :class="{ 'is-off': !liveConnected }">
          <span class="dot" :class="{ 'dot--pulse': liveConnected }" />
          {{ liveConnected ? "实时更新" : "连接中断，重试中" }}
        </span>
        <RouterLink to="/records" class="btn btn--ghost btn--sm" style="margin-left: auto">查看全部</RouterLink>
      </div>
      <div v-if="recent.length === 0" class="empty">
        <div class="empty__title">还没有搜题记录</div>
        完成接入向导后，在 OCS 里发起一次答题，结果会实时出现在这里
        <div><RouterLink class="btn btn--sm" to="/setup" style="margin-top: 12px">打开接入向导</RouterLink></div>
      </div>
      <div v-else class="table-wrap">
        <table class="table">
          <thead>
            <tr>
              <th style="width: 46%">题目</th>
              <th>来源</th>
              <th>模型</th>
              <th class="num">Token</th>
              <th class="num">时间</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in recent" :key="r.id">
              <td>
                <button class="cell-link" type="button" @click="selected = r"><span class="cell-clip">{{ r.title || "（空题目）" }}</span></button>
                <span v-if="r.status === 'failure'" class="badge badge--warn">失败</span>
              </td>
              <td>
                <span class="badge" :class="r.source === 'cache' ? 'badge--ok' : 'badge--ai'">{{ sourceLabel(r.source) }}</span>
              </td>
              <td class="mono faint small">{{ r.model || "-" }}</td>
              <td class="num">{{ r.totalTokens || "-" }}</td>
              <td class="num faint small nowrap">{{ formatDate(r.createdAt) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <QuestionDetails :record="selected" @close="selected = null" />
  </div>
</template>
