<script setup lang="ts">
import type { Question, SearchRecord } from "../api/types";
import { formatDate, sourceLabel, typeLabel } from "../utils/format";
import AppModal from "./AppModal.vue";
defineProps<{ question?: Question | null; record?: SearchRecord | null }>();
defineEmits<{ close: [] }>();
</script>
<template>
  <AppModal v-if="question || record" :title="record ? '答题详情' : '题目详情'" drawer @close="$emit('close')">
    <div class="detail-meta">
      <span class="badge badge--off">{{ typeLabel((question || record)?.qtype || 'unknown') }}</span>
      <span class="badge" :class="record?.status === 'failure' ? 'badge--warn' : 'badge--ok'">{{ record ? record.status === 'failure' ? '失败' : '成功' : sourceLabel(question?.source || '') }}</span>
      <span v-if="record" class="faint small">{{ sourceLabel(record.source) }} · {{ record.durationMs }}ms</span>
    </div>
    <section class="detail-section"><h3>完整题干</h3><div class="detail-text">{{ (question || record)?.title || '（空题目）' }}</div></section>
    <section class="detail-section"><h3>全部选项</h3><div class="detail-text">{{ (question || record)?.options || (record ? '无选项；旧版记录可能未保存选项' : '无选项') }}</div></section>
    <section class="detail-section"><h3>答案</h3><div class="detail-text mono">{{ (question || record)?.answer || '未得到有效答案' }}</div></section>
    <section v-if="record?.error" class="detail-section"><h3>{{ record.status === 'failure' ? '失败原因' : '附加提示' }}</h3><div class="callout callout--warn detail-text">{{ record.error }}</div></section>
    <section v-if="record?.explanation" class="detail-section"><h3>解析</h3><div class="detail-text">{{ record.explanation }}</div></section>
    <section class="detail-section"><h3>来源与用量</h3><dl class="detail-facts">
      <dt>供应商</dt><dd>{{ (question || record)?.provider || '—' }}</dd>
      <dt>模型</dt><dd>{{ (question || record)?.model || '—' }}</dd>
      <template v-if="record"><dt>Token</dt><dd>{{ record.totalTokens }}（输入 {{ record.promptTokens }} / 输出 {{ record.completionTokens }}）<p v-if="record.status === 'failure' && !record.totalTokens" class="faint small">上游未返回用量，不能据此判断是否扣费。</p></dd><dt>图片</dt><dd>{{ record.imageCount || 0 }} 张；记录不保存图片原始内容</dd></template>
      <template v-if="question"><dt>命中次数</dt><dd>{{ question.hits }}</dd></template>
      <dt>时间</dt><dd>{{ formatDate((question || record)?.createdAt || '') }}</dd>
    </dl></section>
    <template #foot><button class="btn" type="button" @click="$emit('close')">关闭详情</button></template>
  </AppModal>
</template>
