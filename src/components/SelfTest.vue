<script setup lang="ts">
import { ref } from "vue";
import { api } from "../api";
import type { SearchOutcome } from "../api/types";
import AppIcon from "./AppIcon.vue";
import { useToast } from "../composables/useToast";
import { sourceLabel } from "../utils/format";

/** 不经过 OCS，直接在本机验证「题库 + AI」链路 */
const emit = defineEmits<{ done: [ok: boolean] }>();
const toast = useToast();
const testTitle = ref("1 + 1 = ?");
const testType = ref("single");
const testOptions = ref("A. 1\nB. 2\nC. 3\nD. 4");
const testing = ref(false);
const result = ref<SearchOutcome | null>(null);

async function runTest() {
  if (!testTitle.value.trim()) {
    toast.info("请先填写测试题目");
    return;
  }
  testing.value = true;
  result.value = null;
  try {
    result.value = await api.testSearch(testTitle.value, testType.value, testOptions.value);
    emit("done", result.value.code === 1);
  } catch (e) {
    toast.fromError(e);
  } finally {
    testing.value = false;
  }
}
</script>

<template>
  <div class="stack">
    <div class="grid grid--3">
      <div class="field">
        <label class="field__label" for="t-type">题型</label>
        <select id="t-type" v-model="testType" class="select">
          <option value="single">单选题</option>
          <option value="multiple">多选题</option>
          <option value="judgement">判断题</option>
          <option value="completion">填空题</option>
        </select>
      </div>
      <div class="field" style="grid-column: span 2">
        <label class="field__label" for="t-title">题干</label>
        <input id="t-title" v-model="testTitle" class="input" placeholder="粘贴一道题目试试" />
      </div>
    </div>
    <div class="field">
      <label class="field__label" for="t-options">选项（每行一个，可留空）</label>
      <textarea id="t-options" v-model="testOptions" class="textarea" rows="4" />
    </div>
    <div class="row">
      <button class="btn btn--primary" type="button" :disabled="testing" @click="runTest">
        <AppIcon :name="testing ? 'refresh' : 'play'" :size="14" :class="{ spin: testing }" />
        {{ testing ? "搜索中…" : "开始测试" }}
      </button>
      <span class="faint small">未命中时会消耗 AI Token，可在「设置」里关闭 AI 兜底。</span>
    </div>

    <div v-if="result" class="result" :class="result.code === 1 ? 'result--ok' : 'result--bad'">
      <div class="row row--between">
        <span class="badge" :class="result.code === 1 ? 'badge--ok' : 'badge--warn'">
          {{ result.code === 1 ? "命中" : "未命中" }}
        </span>
        <span class="faint small">
          {{ sourceLabel(result.source) }} · {{ result.durationMs }}ms
          <template v-if="result.tokens > 0"> · {{ result.tokens }} token</template>
        </span>
      </div>
      <template v-if="result.code === 1">
        <div v-if="result.msg" class="callout callout--warn">{{ result.msg }}</div>
        <div class="result__row">
          <span class="faint small">题目</span>
          <span>{{ result.question ?? "-" }}</span>
        </div>
        <div class="result__row">
          <span class="faint small">答案</span>
          <span class="mono result__answer">{{ result.answer }}</span>
        </div>
        <div v-if="result.explanation" class="result__row">
          <span class="faint small">说明</span>
          <span class="muted">{{ result.explanation }}</span>
        </div>
        <div v-if="result.similarity > 0" class="result__row">
          <span class="faint small">相似度</span>
          <span>{{ result.similarity }}</span>
        </div>
      </template>
      <div v-else class="result__row">
        <span class="faint small">提示</span>
        <span>{{ result.msg ?? "未找到答案" }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.result {
  margin-top: 4px;
  padding: 14px 16px;
  border-radius: var(--r-sm);
  border: 1px solid var(--border);
  background: var(--surface-hover);
  display: flex;
  flex-direction: column;
  gap: 9px;
  animation: result-rise 220ms var(--ease);
}

@keyframes result-rise {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: none; }
}

.result--ok {
  border-color: color-mix(in srgb, var(--success) 30%, transparent);
}

.result--bad {
  border-color: color-mix(in srgb, var(--warning) 32%, transparent);
}

.result__row {
  display: grid;
  grid-template-columns: 62px 1fr;
  gap: 10px;
  font-size: 13px;
  align-items: start;
}

.result__answer {
  font-weight: 600;
  color: var(--accent);
  word-break: break-all;
}
</style>
