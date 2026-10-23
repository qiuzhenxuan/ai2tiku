<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { api } from "../api";
import type { DiagnosticReport } from "../api/types";
import { useToast } from "../composables/useToast";
import { useConfirm } from "../composables/useConfirm";
import AppIcon from "./AppIcon.vue";
const route = useRoute(), toast = useToast(), confirm = useConfirm();
const busy = ref(false), report = ref<DiagnosticReport | null>(null);
const labels = { pass: '正常', warn: '注意', fail: '异常', skip: '未执行' };
async function run(includeModel = false) {
  if (busy.value) return;
  busy.value = true;
  try {
    if (includeModel && !await confirm({ title: '执行模型调用检查？', message: '将向当前选中的模型发送一道简单诊断题，可能产生 Token 费用。结果不写入题库或答题统计。超时后上游仍可能产生费用。', confirmText: '调用并检查' })) return;
    report.value = await api.runDiagnostics(includeModel);
  } catch (error) { toast.fromError(error); } finally { busy.value = false; }
}
onMounted(() => { if (route.query.diagnose === '1') void run(); });
</script>
<template>
  <section id="diagnostics" class="card">
    <div class="card__head"><AppIcon name="check" :size="15"/><span class="card__title">一键诊断</span><span class="card__hint">{{ report ? report.checkedAt : '定位服务与模型问题' }}</span></div>
    <div class="card__body stack">
      <p class="faint small">检查数据库、本地接口和模型配置。默认不访问模型服务，不产生模型 Token。</p>
      <div class="row"><button class="btn btn--primary" type="button" :disabled="busy" @click="run(false)">{{ busy ? '检查中…' : '一键诊断' }}</button><button class="btn" type="button" :disabled="busy" @click="run(true)">模型调用检查…</button></div>
      <div v-if="report" class="diagnostic-list" aria-live="polite"><div v-for="item in report.items" :key="item.name" class="diagnostic-item"><span class="badge" :class="item.status === 'pass' ? 'badge--ok' : item.status === 'skip' ? 'badge--off' : 'badge--warn'">{{ labels[item.status] }}</span><div><strong>{{ item.name }}</strong><p>{{ item.detail }}</p></div><span class="faint small nowrap">{{ item.durationMs }}ms</span></div></div>
    </div>
  </section>
</template>
