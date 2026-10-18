<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { api } from "../api";
import type { ServerStatus } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import { useToast } from "../composables/useToast";

const toast = useToast();

const form = reactive({
  port: 3100,
  matchThreshold: 0.85,
  autoCache: true,
  aiFallback: true,
  systemPrompt: "",
  proxyUrl: "",
});

const status = ref<ServerStatus | null>(null);
const saving = ref(false);
const busy = ref(false);

async function load() {
  try {
    const [s, st] = await Promise.all([api.getSettings(), api.serverStatus()]);
    Object.assign(form, s);
    status.value = st;
  } catch (e) {
    toast.fromError(e);
  }
}

async function save() {
  saving.value = true;
  try {
    const saved = await api.saveSettings({ ...form });
    Object.assign(form, saved);
    status.value = await api.serverStatus();
    toast.ok("设置已保存");
  } catch (e) {
    toast.fromError(e);
  } finally {
    saving.value = false;
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
    <div class="card">
      <div class="card__head">
        <AppIcon name="plug" :size="15" />
        <span class="card__title">服务端</span>
        <span class="badge" :class="status?.running ? 'badge--ok' : 'badge--off'" style="margin-left: auto">
          <span class="dot" :class="{ 'dot--pulse': status?.running }" />
          {{ status?.running ? `运行中 · ${status?.port}` : "已停止" }}
        </span>
      </div>
      <div class="card__body stack">
        <div class="field" style="max-width: 220px">
          <label class="field__label" for="s-port">监听端口</label>
          <input id="s-port" v-model.number="form.port" class="input" type="number" min="1024" max="65535" />
          <span class="field__help">修改后保存会自动重启服务</span>
        </div>
        <div class="row">
          <button class="btn btn--primary" type="button" :disabled="saving" @click="save">
            <AppIcon name="check" :size="14" />
            保存设置
          </button>
          <button class="btn" type="button" :disabled="busy" @click="toggleServer">
            <AppIcon name="power" :size="14" />
            {{ status?.running ? "停止服务" : "启动服务" }}
          </button>
          <span class="mono faint small">{{ status?.address }}</span>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="globe" :size="15" />
        <span class="card__title">网络代理</span>
        <span class="card__hint">留空 = 强制直连</span>
      </div>
      <div class="card__body stack">
        <div class="field" style="max-width: 420px">
          <label class="field__label" for="s-proxy">代理地址</label>
          <input id="s-proxy" v-model="form.proxyUrl" class="input" placeholder="http://127.0.0.1:7890" />
          <span class="field__help">
            留空则**强制直连**，不使用系统代理。国内模型（智谱 / DeepSeek / 通义等）可直接访问，
            建议留空；只有访问 OpenAI 这类被墙服务时才需要填写你的代理地址。
          </span>
        </div>
        <div class="callout callout--warn">
          <AppIcon name="info" :size="16" />
          <div>
            留空的必要性：reqwest 默认会读取 Windows 系统代理。若本机开着代理软件（Clash 等）
            而它没有正常服务，所有 AI 请求都会失败在
            <code class="mono">tunnel error → os error 10061</code>。
          </div>
        </div>
        <div class="row">
          <button class="btn btn--primary" type="button" :disabled="saving" @click="save">
            <AppIcon name="check" :size="14" />
            保存设置
          </button>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="database" :size="15" />
        <span class="card__title">题库匹配与作答</span>
      </div>
      <div class="card__body stack">
        <div class="field" style="max-width: 420px">
          <label class="field__label" for="s-threshold">
            模糊匹配阈值 <span class="mono">{{ form.matchThreshold.toFixed(2) }}</span>
          </label>
          <input
            id="s-threshold"
            v-model.number="form.matchThreshold"
            type="range"
            min="0.5"
            max="1"
            step="0.01"
            style="width: 100%; accent-color: var(--accent)"
          />
          <span class="field__help">
            越低越容易命中缓存，但误答风险也更高。建议 0.80 ~ 0.92
          </span>
        </div>

        <label class="switch">
          <input v-model="form.autoCache" type="checkbox" />
          <span class="switch__track" />
          <span>AI 答完的题目自动写入本地题库（下次直接命中，省 Token）</span>
        </label>

        <label class="switch">
          <input v-model="form.aiFallback" type="checkbox" />
          <span class="switch__track" />
          <span>本地未命中时调用 AI 兜底作答</span>
        </label>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="sparkles" :size="15" />
        <span class="card__title">自定义系统提示词</span>
        <span class="card__hint">留空使用内置提示词</span>
      </div>
      <div class="card__body stack">
        <textarea
          v-model="form.systemPrompt"
          class="textarea"
          rows="7"
          placeholder="留空则使用内置的答题提示词（要求模型返回 answer / explanation 的 JSON）"
        />
        <div class="callout">
          <AppIcon name="info" :size="16" />
          <div>
            内置提示词会要求模型对单选题返回选项字母、多选题返回字母数组、填空题返回文本。
            自定义时请保持同样的输出约定，否则答案格式可能无法被 OCS 正确解析。
          </div>
        </div>
        <div class="row">
          <button class="btn btn--primary" type="button" :disabled="saving" @click="save">
            <AppIcon name="check" :size="14" />
            保存设置
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
