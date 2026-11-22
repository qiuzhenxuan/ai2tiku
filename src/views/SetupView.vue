<script lang="ts">
import type { OcsRelease as _R } from "../api/types";
/** 模块级缓存：同一次运行内切换页面不重复请求 GitHub */
let cachedRelease: _R | null = null;
</script>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api } from "../api";
import type { OcsRelease, ProviderPreset } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import SelfTest from "../components/SelfTest.vue";
import { useToast } from "../composables/useToast";
import { useOcsConnection } from "../composables/useOcsConnection";
import { useSetupProgress, type StepId } from "../composables/useSetupProgress";

const SCRIPT_MANAGER_URL =
  "https://microsoftedge.microsoft.com/addons/detail/scriptcat/liilgpjgabokdklappibcjfablkpcekh";
const SCRIPT_PERMISSION_URL = "https://docs.scriptcat.org/docs/use/open-dev/?browser=edge";
const OCS_RELEASES_URL = "https://github.com/ocsjs/ocsjs/releases/latest";
const OCS_DOC_URL = "https://docs.ocsjs.com/docs/work";

const toast = useToast();
const route = useRoute();
const router = useRouter();
const setup = useSetupProgress();
const conn = useOcsConnection();

/* ---------- 当前步骤（与地址栏 ?step= 同步） ---------- */
const ids = computed(() => setup.steps.value.map((s) => s.id));
const current = ref<StepId>("browser");
const index = computed(() => ids.value.indexOf(current.value));
const step = computed(() => setup.steps.value[index.value]);

function go(id: StepId) {
  current.value = id;
  router.replace({ query: { ...route.query, step: id } });
}
function move(delta: number) {
  const next = ids.value[index.value + delta];
  if (next) go(next);
}
watch(
  () => route.query.step,
  (v) => {
    if (typeof v === "string" && (ids.value as string[]).includes(v)) current.value = v as StepId;
  },
  { immediate: true },
);

async function open(url: string) {
  try {
    await api.openInEdge(url);
  } catch (e) {
    toast.fromError(e);
  }
}

/* ---------- 第 2 步：OCS 脚本版本 ----------
 * 只在进入这一步时才请求 GitHub，且本次运行成功一次后复用结果；
 * 失败不弹窗，在步骤内说明原因（国内直连 api.github.com 常失败，软件默认不走系统代理）。 */
const release = ref<OcsRelease | null>(cachedRelease);
const loadingRelease = ref(false);
const releaseError = ref("");
async function loadRelease(force = false) {
  if (loadingRelease.value || (release.value && !force)) return;
  loadingRelease.value = true;
  releaseError.value = "";
  try {
    release.value = cachedRelease = await api.latestOcsRelease();
  } catch (e) {
    releaseError.value = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  } finally {
    loadingRelease.value = false;
  }
}
watch(current, (id) => { if (id === "script") void loadRelease(); }, { immediate: true });
/** 安装：拿到版本号就直接装脚本，否则打开 Releases 页面手动安装 */
function installOcs() {
  void open(release.value?.scriptUrl ?? OCS_RELEASES_URL);
}

/* ---------- 第 3 步：就地添加模型 ---------- */
const presets = ref<ProviderPreset[]>([]);
const quick = reactive({ presetId: "zhipu", apiKey: "", saving: false });
const quickPreset = computed(() => presets.value.find((p) => p.id === quick.presetId));
const aiModel = computed(() => setup.deployment.value?.aiModel ?? null);
const fallbackOff = computed(() => !!aiModel.value && !setup.deployment.value?.aiFallback);

async function quickSave() {
  const p = quickPreset.value;
  if (!p || !p.baseUrl) return;
  if (!quick.apiKey.trim() && p.id !== "ollama") {
    toast.info("请先粘贴 API Key");
    return;
  }
  quick.saving = true;
  try {
    const id = await api.saveProvider({ name: p.name, baseUrl: p.baseUrl, apiKey: quick.apiKey.trim(), enabled: true });
    const r = await api.syncProviderModels(id).catch((e) => {
      toast.error(`供应商已保存，但自动获取模型失败：${typeof e === "string" ? e : String(e)}`);
      return null;
    });
    if (r) toast.ok(`${r.provider} 已就绪，自动添加了 ${r.added} 个模型`);
    quick.apiKey = "";
    await Promise.all([setup.load(), conn.refresh()]);
  } catch (e) {
    toast.fromError(e);
  } finally {
    quick.saving = false;
  }
}

/* ---------- 第 4 步：复制配置 ---------- */
async function copy() {
  if (await conn.copyConfig()) setup.setMark("copied", true);
}
async function startServer() {
  await conn.startServer();
  await setup.load();
}

onMounted(async () => {
  await setup.load();
  if (typeof route.query.step !== "string" && setup.nextStep.value) current.value = setup.nextStep.value.id;
  api.listProviderPresets().then((p) => (presets.value = p.filter((x) => x.baseUrl))).catch(() => {});
});
</script>

<template>
  <div class="wizard">
    <!-- 左：步骤导航 -->
    <nav class="wizard__nav card" aria-label="接入步骤">
      <div class="wizard__progress">
        <span>完成 {{ setup.doneCount.value }} / {{ setup.steps.value.length }}</span>
        <div class="progress"><span class="progress__bar" :style="{ width: `${(setup.doneCount.value / setup.steps.value.length) * 100}%` }" /></div>
      </div>
      <button
        v-for="(s, i) in setup.steps.value"
        :key="s.id"
        class="wizard__item"
        :class="{ 'is-current': s.id === current, 'is-done': s.done }"
        type="button"
        :aria-current="s.id === current ? 'step' : undefined"
        @click="go(s.id)"
      >
        <span class="wizard__num">
          <AppIcon v-if="s.done" name="check" :size="12" />
          <template v-else>{{ i + 1 }}</template>
        </span>
        <span class="wizard__label">
          <strong>{{ s.title }}</strong>
          <small>{{ s.done ? "已完成" : s.hint }}</small>
        </span>
      </button>
    </nav>

    <!-- 右：步骤内容 -->
    <Transition name="page" mode="out-in">
      <section :key="current" class="wizard__panel card">
        <header class="wizard__head">
          <span class="wizard__step">第 {{ index + 1 }} 步</span>
          <h2>{{ step?.title }}</h2>
          <span v-if="step?.done" class="badge badge--ok"><AppIcon name="check" :size="11" /> 已完成</span>
        </header>

        <div class="wizard__body">
          <!-- 1 浏览器与脚本猫 -->
          <template v-if="current === 'browser'">
            <p class="lead">OCS 是一个用户脚本，需要先在 Edge 里装好脚本管理器「脚本猫」，并允许它运行用户脚本。</p>
            <div class="checks">
              <div class="check" :class="{ 'is-ok': setup.browser.value?.edgeInstalled }">
                <AppIcon :name="setup.browser.value?.edgeInstalled ? 'check' : 'info'" :size="14" />
                {{ setup.browser.value ? (setup.browser.value.edgeInstalled ? "已检测到 Edge" : "未检测到 Edge，请先安装 Microsoft Edge") : "正在检测浏览器…" }}
                <span v-if="setup.browser.value" class="faint small">默认浏览器：{{ setup.browser.value.defaultBrowser }}</span>
              </div>
            </div>
            <ol class="howto">
              <li>在 Edge 加载项商店打开脚本猫，点击「获取」安装。
                <div class="howto__actions">
                  <button class="btn btn--primary" type="button" @click="open(SCRIPT_MANAGER_URL)"><AppIcon name="external" :size="13" />打开 Edge 加载项</button>
                </div>
              </li>
              <li>打开扩展管理 → 脚本猫「详细信息」→ 开启 <strong>允许用户脚本</strong>。旧版 Edge 没有该选项时，开启扩展页左下角的「开发人员模式」。
                <div class="howto__actions">
                  <button class="btn" type="button" @click="open('edge://extensions/')">打开扩展管理</button>
                  <button class="btn btn--ghost" type="button" @click="open(SCRIPT_PERMISSION_URL)">查看图解</button>
                </div>
              </li>
              <li>关闭再开启一次脚本猫扩展（或重启浏览器），让权限生效。</li>
            </ol>
            <label class="confirm-check">
              <input type="checkbox" :checked="!!setup.marks.scriptcat" @change="setup.setMark('scriptcat', ($event.target as HTMLInputElement).checked)" />
              <span>我已安装脚本猫并开启了用户脚本权限</span>
            </label>
          </template>

          <!-- 2 OCS 脚本 -->
          <template v-else-if="current === 'script'">
            <p class="lead">从 OCS 官方 GitHub Releases 安装最新脚本。点击后脚本猫会弹出安装页，点「安装」即可。</p>
            <div class="checks">
              <div class="check" :class="{ 'is-ok': !!release }">
                <AppIcon :name="release ? 'check' : 'info'" :size="14" />
                <span v-if="release">最新版本 {{ release.version }}</span>
                <span v-else-if="loadingRelease">正在从 GitHub 获取最新版本…</span>
                <span v-else>暂时拿不到版本号，不影响安装：点下方按钮会打开 Releases 页面手动安装</span>
                <button class="btn btn--ghost btn--sm" type="button" :disabled="loadingRelease" @click="loadRelease(true)">
                  <AppIcon name="refresh" :size="12" :class="{ spin: loadingRelease }" />重新获取
                </button>
              </div>
              <details v-if="releaseError && !release" class="disclosure">
                <summary>为什么获取失败？</summary>
                <p class="faint small">
                  软件访问 GitHub 时默认直连、不经过系统代理，国内网络直连 <code class="mono">api.github.com</code> 常常超时；
                  GitHub 对未登录请求也有每小时 60 次的限制。开着 Clash 等代理软件的话，可以到
                  <RouterLink to="/settings#service">设置 → 网络代理</RouterLink> 填入代理地址（Clash Verge 默认
                  <code class="mono">http://127.0.0.1:7897</code>）后再点「重新获取」。
                </p>
                <p class="faint small mono">错误信息：{{ releaseError }}</p>
              </details>
            </div>
            <div class="howto__actions">
              <button class="btn btn--primary" type="button" :disabled="loadingRelease" @click="installOcs">
                <AppIcon name="rocket" :size="13" />{{ release ? `安装 OCS 脚本 ${release.version}` : "前往 Releases 安装" }}
              </button>
              <button v-if="release" class="btn" type="button" @click="open(OCS_RELEASES_URL)"><AppIcon name="external" :size="13" />打开 Releases</button>
            </div>
            <div class="callout">
              <AppIcon name="info" :size="15" />
              <div>只下载了文件、没有弹出安装页？通常是脚本猫未启用或用户脚本权限没开，回到上一步检查。</div>
            </div>
            <label class="confirm-check">
              <input type="checkbox" :checked="!!setup.marks.ocs" @change="setup.setMark('ocs', ($event.target as HTMLInputElement).checked)" />
              <span>我已安装 OCS 脚本</span>
            </label>
          </template>

          <!-- 3 AI 模型 -->
          <template v-else-if="current === 'model'">
            <template v-if="aiModel">
              <div class="model-ready">
                <span class="model-ready__icon"><AppIcon name="sparkles" :size="18" /></span>
                <div>
                  <div class="faint small">当前作答模型</div>
                  <strong>{{ aiModel.provider }} · {{ aiModel.model }}</strong>
                </div>
                <RouterLink class="btn" to="/models" style="margin-left: auto">管理模型</RouterLink>
              </div>
              <div v-if="fallbackOff" class="callout callout--warn">
                <AppIcon name="info" :size="15" />
                <div>「AI 兜底作答」已关闭，本地未命中的题目不会交给模型。<RouterLink to="/settings#strategy">去设置开启</RouterLink></div>
              </div>
            </template>
            <template v-else>
              <p class="lead">选一个厂商，粘贴 API Key，保存后会自动获取可用模型。没有 Key？推荐先用智谱 GLM。</p>
              <div class="quick-form">
                <div class="field">
                  <label class="field__label" for="q-preset">厂商</label>
                  <select id="q-preset" v-model="quick.presetId" class="select">
                    <option v-for="p in presets" :key="p.id" :value="p.id">{{ p.name }}</option>
                  </select>
                </div>
                <div class="field">
                  <label class="field__label" for="q-key">API Key</label>
                  <input id="q-key" v-model="quick.apiKey" class="input" type="password" autocomplete="off" placeholder="粘贴你的 API Key" @keyup.enter="quickSave" />
                  <span v-if="quickPreset?.homepage" class="field__help">
                    还没有 Key？<a href="#" @click.prevent="open(quickPreset.homepage)">去 {{ quickPreset.name }} 申请</a>
                  </span>
                </div>
                <button class="btn btn--primary" type="button" :disabled="quick.saving" @click="quickSave">
                  <AppIcon :name="quick.saving ? 'refresh' : 'check'" :size="13" :class="{ spin: quick.saving }" />
                  {{ quick.saving ? "保存并获取模型…" : "保存并获取模型" }}
                </button>
              </div>
              <p class="faint small">需要自定义 base_url、温度或多个供应商？到 <RouterLink to="/models">AI 模型</RouterLink> 页面配置。</p>
            </template>
          </template>

          <!-- 4 接入 OCS -->
          <template v-else-if="current === 'connect'">
            <p class="lead">复制下面的题库配置，打开网课页面的 OCS 面板 →「题库配置」，粘贴并保存。</p>
            <div class="checks">
              <div class="check" :class="{ 'is-ok': conn.server.value?.running }">
                <AppIcon :name="conn.server.value?.running ? 'check' : 'info'" :size="14" />
                {{ conn.server.value?.running ? `本地服务运行中：${conn.address.value}/api/search` : "本地服务未启动，复制前需要先启动" }}
                <button v-if="!conn.server.value?.running" class="btn btn--sm" type="button" :disabled="conn.busy.value" @click="startServer">
                  <AppIcon name="power" :size="12" />启动服务
                </button>
              </div>
            </div>
            <div class="howto__actions">
              <button class="btn btn--primary btn--lg" type="button" :disabled="!conn.canCopy.value || conn.busy.value" @click="copy">
                <AppIcon name="copy" :size="14" />复制题库配置
              </button>
              <button class="btn btn--ghost" type="button" @click="open(OCS_DOC_URL)"><AppIcon name="external" :size="13" />OCS 自动答题说明</button>
            </div>
            <details class="disclosure">
              <summary>查看配置内容</summary>
              <div class="code">{{ conn.config.value || "本地服务启动后显示" }}</div>
            </details>
            <div class="callout callout--info">
              <AppIcon name="info" :size="15" />
              <div>
                <div class="callout__title">出现跨域失败？</div>
                到脚本猫把 OCS 的 <code class="mono">@connect</code> 模式设为宽松模式，或安装官方「全域名通用版本」。
                粘贴的是完整 JSON，不是订阅链接；更改端口后需要重新复制。
              </div>
            </div>
          </template>

          <!-- 5 验证 -->
          <template v-else>
            <p class="lead">先在本机自测「题库 + AI」链路；通过后到网课页面发起一次真实搜题，结果会出现在「记录与统计」。</p>
            <SelfTest @done="(ok) => ok && setup.load()" />
            <div v-if="setup.steps.value[4].done" class="callout callout--info">
              <AppIcon name="check" :size="15" />
              <div>
                <div class="callout__title">已经有答题记录了</div>
                接入完成。之后的每一次搜题都可以在 <RouterLink to="/records">记录与统计</RouterLink> 里查看。
              </div>
            </div>
          </template>
        </div>

        <footer class="wizard__foot">
          <button class="btn btn--ghost" type="button" :disabled="index === 0" @click="move(-1)">上一步</button>
          <button v-if="index < ids.length - 1" class="btn" type="button" @click="move(1)">
            下一步：{{ setup.steps.value[index + 1]?.title }}
          </button>
          <RouterLink v-else class="btn" to="/">返回首页</RouterLink>
        </footer>
      </section>
    </Transition>
  </div>
</template>
