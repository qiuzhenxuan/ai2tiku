<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref } from "vue";
import { onBeforeRouteLeave, useRoute } from "vue-router";
import { api } from "../api";
import type { Settings } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import DiagnosticsCard from "../components/DiagnosticsCard.vue";
import ThemeToggle from "../components/ThemeToggle.vue";
import { ACCENT_PRESETS, useAccent } from "../composables/useAccent";
import { useConfirm } from "../composables/useConfirm";
import { useServer } from "../composables/useServer";
import { useTheme } from "../composables/useTheme";
import { useToast } from "../composables/useToast";

const toast = useToast();
const route = useRoute();
const confirm = useConfirm();
const { accent, setAccent } = useAccent();
const { theme } = useTheme();
const server = useServer();

const SECTIONS = [
  { id: "service", title: "服务与网络", icon: "plug" },
  { id: "strategy", title: "作答策略", icon: "database" },
  { id: "prompt", title: "系统提示词", icon: "sparkles" },
  { id: "appearance", title: "外观", icon: "sun" },
  { id: "diagnostics", title: "诊断", icon: "check" },
];

function swatchColor(p: (typeof ACCENT_PRESETS)[number]) {
  return theme.value === "dark" ? p.dark : p.light;
}
const activeAccentLabel = computed(() => ACCENT_PRESETS.find((p) => p.id === accent.value)?.label ?? "");

/* ---------- 表单与「未保存」检测 ---------- */
const form = reactive<Settings>({
  port: 3100,
  matchThreshold: 0.85,
  autoCache: true,
  aiFallback: true,
  systemPrompt: "",
  proxyUrl: "",
});
const saved = ref<Settings | null>(null);
const dirty = computed(() => !!saved.value && JSON.stringify(form) !== JSON.stringify(saved.value));
const portChanged = computed(() => !!saved.value && form.port !== saved.value.port);
const saving = ref(false);

const builtinPrompt = ref("");
const usingBuiltin = computed(() => form.systemPrompt.trim() === "");
const promptText = computed({
  get: () => (usingBuiltin.value ? builtinPrompt.value : form.systemPrompt),
  set: (v: string) => {
    form.systemPrompt = v;
  },
});

async function load() {
  try {
    const [s, builtin] = await Promise.all([api.getSettings(), api.defaultSystemPrompt()]);
    Object.assign(form, s);
    saved.value = { ...s };
    builtinPrompt.value = builtin;
  } catch (e) {
    toast.fromError(e);
  }
}

async function save() {
  saving.value = true;
  try {
    const next = await api.saveSettings({ ...form });
    Object.assign(form, next);
    saved.value = { ...next };
    await server.refresh();
    toast.ok(portChanged.value ? "设置已保存，服务已按新端口重启" : "设置已保存");
  } catch (e) {
    toast.fromError(e);
  } finally {
    saving.value = false;
  }
}

function discard() {
  if (saved.value) Object.assign(form, saved.value);
}

onBeforeRouteLeave(async () => {
  if (!dirty.value) return true;
  return await confirm({ title: "放弃未保存的设置？", message: "离开后，本页尚未保存的修改将丢失。", confirmText: "放弃修改", danger: true });
});

/* ---------- 侧边目录 ---------- */
const activeSection = ref("service");
function scrollTo(id: string) {
  activeSection.value = id;
  document.getElementById(`sec-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
}

/* Ctrl + S 保存 */
function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    if (dirty.value && !saving.value) void save();
  }
}
onUnmounted(() => window.removeEventListener("keydown", onKey));

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await load();
  const hash = route.hash.replace("#", "") || (route.query.diagnose === "1" ? "diagnostics" : "");
  if (hash) {
    await nextTick();
    scrollTo(hash);
  }
});
</script>

<template>
  <div class="settings">
    <nav class="settings__nav" aria-label="设置分组">
      <button
        v-for="s in SECTIONS"
        :key="s.id"
        type="button"
        class="settings__link"
        :class="{ 'is-active': activeSection === s.id }"
        @click="scrollTo(s.id)"
      >
        <AppIcon :name="s.icon" :size="14" />{{ s.title }}
      </button>
    </nav>

    <div class="page settings__body">
      <!-- 服务与网络 -->
      <section id="sec-service" class="card">
        <div class="card__head">
          <AppIcon name="plug" :size="15" />
          <span class="card__title">服务与网络</span>
          <span class="badge" :class="server.running.value ? 'badge--ok' : 'badge--off'" style="margin-left: auto">
            <span class="dot" :class="{ 'dot--pulse': server.running.value }" />
            {{ server.running.value ? `运行中 · ${server.status.value?.port}` : "已停止" }}
          </span>
        </div>
        <div class="card__body">
          <div class="setting-row">
            <div class="setting-row__text">
              <strong>本地题库服务</strong>
              <span>只监听 127.0.0.1，外网无法访问。{{ server.status.value?.address }}</span>
            </div>
            <button class="btn" type="button" :disabled="server.busy.value" @click="server.toggle">
              <AppIcon name="power" :size="13" />{{ server.running.value ? "停止服务" : "启动服务" }}
            </button>
          </div>
          <div class="setting-row">
            <label class="setting-row__text" for="s-port">
              <strong>监听端口</strong>
              <span>保存后自动重启服务；改端口后需要在 OCS 中重新粘贴配置。</span>
            </label>
            <input id="s-port" v-model.number="form.port" class="input setting-row__control--narrow" type="number" min="1024" max="65535" />
          </div>
          <div class="setting-row setting-row--stack">
            <label class="setting-row__text" for="s-proxy">
              <strong>网络代理</strong>
              <span>留空表示<b>强制直连</b>，不读取系统代理。国内模型建议留空；只有访问 OpenAI 等服务时才需要填写。</span>
            </label>
            <input id="s-proxy" v-model="form.proxyUrl" class="input" placeholder="留空 = 直连，例如 http://127.0.0.1:7890" />
            <details class="disclosure">
              <summary>为什么默认直连？</summary>
              <p class="faint small">
                reqwest 默认读取 Windows 系统代理。若本机开着 Clash 等代理软件但它没有正常工作，所有 AI 请求都会失败在
                <code class="mono">tunnel error → os error 10061</code>。
              </p>
            </details>
          </div>
        </div>
      </section>

      <!-- 作答策略 -->
      <section id="sec-strategy" class="card">
        <div class="card__head">
          <AppIcon name="database" :size="15" />
          <span class="card__title">作答策略</span>
        </div>
        <div class="card__body">
          <div class="setting-row">
            <div class="setting-row__text">
              <strong>AI 兜底作答</strong>
              <span>本地题库未命中时交给 AI 回答；关闭后只返回本地题库的答案。</span>
            </div>
            <label class="switch"><input v-model="form.aiFallback" type="checkbox" /><span class="switch__track" /></label>
          </div>
          <div class="setting-row">
            <div class="setting-row__text">
              <strong>自动缓存 AI 答案</strong>
              <span>通过格式校验的纯文本题答案写入本地题库，下次直接命中、不再消耗 Token。</span>
            </div>
            <label class="switch"><input v-model="form.autoCache" type="checkbox" /><span class="switch__track" /></label>
          </div>
          <div class="setting-row">
            <div class="setting-row__text">
              <strong>精确匹配</strong>
              <span>只有题干、题型和完整选项都一致时才复用答案，避免「正确 / 不正确」这类相似题误答。此项固定开启。</span>
            </div>
            <span class="badge badge--ok">始终开启</span>
          </div>
        </div>
      </section>

      <!-- 系统提示词 -->
      <section id="sec-prompt" class="card">
        <div class="card__head">
          <AppIcon name="sparkles" :size="15" />
          <span class="card__title">系统提示词</span>
          <span class="badge" :class="usingBuiltin ? 'badge--ok' : 'badge--ai'">{{ usingBuiltin ? "使用内置" : "已自定义" }}</span>
          <button class="btn btn--ghost btn--sm" style="margin-left: auto" type="button" :disabled="usingBuiltin" @click="form.systemPrompt = ''">
            <AppIcon name="refresh" :size="13" />恢复内置
          </button>
        </div>
        <div class="card__body stack">
          <textarea v-model="promptText" class="textarea" rows="12" spellcheck="false" placeholder="加载中…" />
          <p class="faint small">
            直接编辑即成为自定义提示词。请保留 JSON 输出约定（<code class="mono">answer</code> /
            <code class="mono">explanation</code>），否则 OCS 可能无法解析答案；多选题字母会自动用
            <code class="mono">#</code> 连接。
          </p>
        </div>
      </section>

      <!-- 外观（即时生效，不需要保存） -->
      <section id="sec-appearance" class="card">
        <div class="card__head">
          <AppIcon name="sun" :size="15" />
          <span class="card__title">外观</span>
          <span class="card__hint">即时生效</span>
        </div>
        <div class="card__body">
          <div class="setting-row">
            <div class="setting-row__text"><strong>日夜主题</strong><span>默认跟随系统，选择后会被记住。</span></div>
            <ThemeToggle />
          </div>
          <div class="setting-row setting-row--stack">
            <div class="setting-row__text"><strong>强调色</strong><span>默认水墨黑白，也可以选一个颜色。当前：{{ activeAccentLabel }}</span></div>
            <div class="swatches">
              <button
                v-for="p in ACCENT_PRESETS"
                :key="p.id"
                type="button"
                class="swatch"
                :class="{ 'is-active': accent === p.id }"
                :style="{ '--swatch': swatchColor(p) }"
                :title="p.label"
                :aria-label="p.label"
                @click="setAccent(p.id)"
              />
            </div>
          </div>
        </div>
      </section>

      <div id="sec-diagnostics"><DiagnosticsCard /></div>
    </div>

    <!-- 未保存提示条 -->
    <Transition name="savebar">
      <div v-if="dirty" class="savebar" role="status">
        <AppIcon name="info" :size="15" />
        <span>有尚未保存的修改{{ portChanged ? "，保存后服务将按新端口重启" : "" }}</span>
        <button class="btn btn--ghost btn--sm" type="button" :disabled="saving" @click="discard">撤销</button>
        <button class="btn btn--primary btn--sm" type="button" :disabled="saving" @click="save">
          <AppIcon :name="saving ? 'refresh' : 'check'" :size="13" :class="{ spin: saving }" />保存设置<kbd class="kbd-on-primary">Ctrl S</kbd>
        </button>
      </div>
    </Transition>
  </div>
</template>
