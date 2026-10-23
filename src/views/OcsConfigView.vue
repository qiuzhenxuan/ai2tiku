<script setup lang="ts">
import { ref } from "vue";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api } from "../api";
import type { SearchOutcome } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import { useToast } from "../composables/useToast";
import { sourceLabel } from "../utils/format";
import { useOcsConnection } from "../composables/useOcsConnection";

const toast = useToast();
const { config, server: status, address, canCopy, busy, loading, error, refresh, copyConfig, startServer } = useOcsConnection();

const testTitle = ref("1 + 1 = ?");
const testType = ref("single");
const testOptions = ref("A. 1\nB. 2\nC. 3\nD. 4");
const testing = ref(false);
const result = ref<SearchOutcome | null>(null);

async function copyAddress() {
  try {
    if (!await refresh() || !canCopy.value) throw new Error("本地服务未就绪，请先启动服务");
    await writeText(`${address.value}/api/search`);
    toast.ok("接口地址已复制到剪贴板");
  } catch (e) {
    toast.fromError(e);
  }
}

async function runTest() {
  if (!testTitle.value.trim()) {
    toast.info("请先填写测试题目");
    return;
  }
  testing.value = true;
  result.value = null;
  try {
    result.value = await api.testSearch(testTitle.value, testType.value, testOptions.value);
  } catch (e) {
    toast.fromError(e);
  } finally {
    testing.value = false;
  }
}

async function openPermissions() {
  try { await api.openInEdge("https://docs.scriptcat.org/docs/use/open-dev/?browser=edge"); }
  catch (e) { toast.fromError(e); }
}
</script>

<template>
  <div class="page">
    <div class="callout callout--info">
      <AppIcon name="info" :size="16" />
      <div>
        <div class="callout__title">接入前必读</div>
        OCS 脚本请求本机地址时，需要在脚本头部 <code class="mono">@connect</code> 中放行域名。
        请到脚本管理器把 <code class="mono">@connect 模式</code> 设为宽松模式，或安装官方「全域名通用版本」，
        否则会出现跨域失败。本服务只监听 <code class="mono">127.0.0.1</code>，不会对外网暴露。
      </div>
    </div>
    <div v-if="error" class="callout callout--warn">{{ error }} <button class="btn btn--sm" type="button" @click="refresh">重试</button></div>
    <div class="callout callout--info">
      <AppIcon name="info" :size="16" />
      <div>脚本猫还需要开启「允许用户脚本」；没有该选项时开启 Edge 扩展页的「开发人员模式」，再重新启用扩展或重启浏览器。
        <button class="btn btn--sm" type="button" @click="openPermissions">查看权限设置图解</button>
      </div>
    </div>

    <div class="grid grid--2">
      <div class="card">
        <div class="card__head">
          <AppIcon name="plug" :size="15" />
          <span class="card__title">接口地址</span>
        </div>
        <div class="card__body stack">
          <div class="code">{{ address ? `${address}/api/search` : "正在读取接口地址…" }}</div>
          <div class="row row--wrap">
            <button
              class="btn"
              type="button"
              :disabled="!canCopy || busy"
              @click="copyAddress"
            >
              <AppIcon name="copy" :size="14" />
              复制地址
            </button>
            <button v-if="!status?.running" class="btn" type="button" :disabled="busy || loading" @click="startServer">启动本地服务</button>
            <span class="badge" :class="status?.running ? 'badge--ok' : 'badge--off'">
              <span class="dot" :class="{ 'dot--pulse': status?.running }" />
              {{ status?.running ? "运行中" : "已停止" }}
            </span>
          </div>
          <p class="faint small">
            支持 <code class="mono">GET</code> 与 <code class="mono">POST</code>，参数为
            <code class="mono">title</code> / <code class="mono">type</code> /
            <code class="mono">options</code>。
          </p>
        </div>
      </div>

      <div class="card">
        <div class="card__head">
          <AppIcon name="settings" :size="15" />
          <span class="card__title">接入步骤</span>
        </div>
        <div class="card__body stack" style="gap: 10px">
          <div class="step"><span class="step__idx">1</span>打开 OCS 脚本设置，找到「题库配置」。</div>
          <div class="step"><span class="step__idx">2</span>复制下面的完整 JSON 内容，粘贴到题库配置并保存。不要粘贴订阅链接。</div>
          <div class="step"><span class="step__idx">3</span>在 OCS 中发起实际搜题，核对「答题记录」中的对应题目和结果。</div>
          <div class="step"><span class="step__idx">4</span>本题库会先用本地缓存命中，未命中再交给 AI 作答。</div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="copy" :size="15" />
        <span class="card__title">题库配置 JSON（粘贴给 OCS）</span>
        <button class="btn btn--primary btn--sm" style="margin-left: auto" type="button" :disabled="!canCopy || busy" @click="copyConfig">
          <AppIcon name="copy" :size="13" />
          复制配置
        </button>
      </div>
      <div class="card__body">
        <div class="code">{{ config }}</div>
        <p class="faint small">更改监听端口后，需要重新复制 JSON 并在 OCS 中保存。模型切换不需要重新复制配置。</p>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="play" :size="15" />
        <span class="card__title">链路自测</span>
        <span class="card__hint">不经过 OCS，直接在本机验证「题库 + AI」是否可用</span>
      </div>
      <div class="card__body stack">
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
            <AppIcon name="play" :size="14" :class="{ spin: testing }" />
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
    </div>
  </div>
</template>

<style scoped>
.step {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  font-size: 13px;
  color: var(--text-muted);
  line-height: 1.6;
}

.step__idx {
  display: grid;
  place-items: center;
  width: 19px;
  height: 19px;
  flex: none;
  margin-top: 2px;
  border-radius: 50%;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11px;
  font-weight: 700;
}

.result {
  margin-top: 4px;
  padding: 14px 16px;
  border-radius: var(--r-sm);
  border: 1px solid var(--border);
  background: var(--surface-hover);
  display: flex;
  flex-direction: column;
  gap: 9px;
  animation: rise 220ms var(--ease);
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
