<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import type { BrowserInfo, OcsRelease, ServerStatus } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import { useToast } from "../composables/useToast";

/** 脚本猫（ScriptCat）Edge 加载项 */
const SCRIPT_MANAGER_URL =
  "https://microsoftedge.microsoft.com/addons/detail/scriptcat/liilgpjgabokdklappibcjfablkpcekh";
const OCS_RELEASES_URL = "https://github.com/ocsjs/ocsjs/releases/latest";
const OCS_DOC_URL = "https://docs.ocsjs.com/docs/work";

const toast = useToast();
const router = useRouter();

const browser = ref<BrowserInfo | null>(null);
const status = ref<ServerStatus | null>(null);
const release = ref<OcsRelease | null>(null);
const loadingRelease = ref(false);
const providerCount = ref(0);
const modelCount = ref(0);
/** 默认推荐智谱 GLM，注册走邀请链接；正常情况下以预设里的 homepage 为准 */
const glmHomepage = ref(
  "https://www.bigmodel.cn/invite?icode=sgWVI0vRXdaQMNjdvoiGd0jPr3uHog9F4g5tjuOUqno%3D",
);
const glmBaseUrl = ref("https://open.bigmodel.cn/api/paas/v4");

const subscribeUrl = computed(
  () => `${status.value?.address || `http://127.0.0.1:${status.value?.port ?? 3100}`}/ocs`,
);

/** 能自动判断的步骤就自动打勾，剩下的是用户在浏览器里的手动操作 */
const aiReady = computed(() => providerCount.value > 0 && modelCount.value > 0);

async function loadRelease() {
  loadingRelease.value = true;
  try {
    release.value = await api.latestOcsRelease();
  } catch (e) {
    release.value = null;
    toast.fromError(e);
  } finally {
    loadingRelease.value = false;
  }
}

async function load() {
  try {
    const [b, s, providers, models, presets] = await Promise.all([
      api.browserInfo(),
      api.serverStatus(),
      api.listProviders(),
      api.listModels(),
      api.listProviderPresets(),
    ]);
    browser.value = b;
    status.value = s;
    providerCount.value = providers.length;
    modelCount.value = models.length;
    const glm = presets.find((p) => p.id === "zhipu");
    if (glm) {
      glmHomepage.value = glm.homepage;
      glmBaseUrl.value = glm.baseUrl;
    }
  } catch (e) {
    toast.fromError(e);
  }
  loadRelease();
}

async function open(url: string) {
  try {
    await openUrl(url);
  } catch (e) {
    toast.fromError(e);
  }
}

async function copy(text: string, label: string) {
  try {
    await writeText(text);
    toast.ok(`${label}已复制`);
  } catch (e) {
    toast.fromError(e);
  }
}

/** 一键开始：打开脚本猫安装页，并先把订阅链接复制好 */
async function quickStart() {
  await copy(subscribeUrl.value, "订阅链接");
  await open(SCRIPT_MANAGER_URL);
}

/** 配置供应商：跳到供应商页并自动弹出「智谱 GLM」预设 */
function configureProvider() {
  router.push({ path: "/providers", query: { add: "zhipu" } });
}

onMounted(load);
</script>

<template>
  <div class="page">
    <div class="card">
      <div class="card__head">
        <AppIcon name="rocket" :size="15" />
        <span class="card__title">从零部署 OCS</span>
        <span class="card__hint">按顺序走完 5 步即可自动答题</span>
        <button class="btn btn--primary btn--sm" style="margin-left: auto" type="button" @click="quickStart">
          <AppIcon name="play" :size="12" />
          一键开始
        </button>
      </div>
      <div class="card__body stack">
        <div class="row row--wrap">
          <span class="badge" :class="browser?.edgeInstalled ? 'badge--ok' : 'badge--off'">
            <span class="dot" />
            {{ browser === null ? "检测中…" : browser.edgeInstalled ? "Edge 已安装" : "未检测到 Edge" }}
          </span>
          <span class="badge" :class="browser?.edgeDefault ? 'badge--ok' : 'badge--off'">
            <span class="dot" />
            默认浏览器：{{ browser?.defaultBrowser ?? "检测中…" }}
          </span>
          <span class="badge" :class="status?.running ? 'badge--ok' : 'badge--off'">
            <span class="dot" :class="{ 'dot--pulse': status?.running }" />
            本地题库服务：{{ status?.running ? `运行中 · ${status.port}` : "未启动" }}
          </span>
          <span class="badge" :class="aiReady ? 'badge--ok' : 'badge--off'">
            <span class="dot" />
            AI：{{ aiReady ? `${providerCount} 个供应商 · ${modelCount} 个模型` : "未配置" }}
          </span>
          <span class="badge badge--ai">
            <span class="dot" />
            {{ release ? `OCS 脚本 ${release.version}` : loadingRelease ? "获取最新版本…" : "版本获取失败" }}
          </span>
        </div>
        <p class="faint small" style="margin: 0">
          部署完成后，OCS 会在网课页面里读题并发给本地服务（{{ subscribeUrl }}）取答案，再自动填回页面。
        </p>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="database" :size="15" />
        <span class="card__title">部署步骤</span>
        <span class="card__hint">第一步到第三步在浏览器里完成</span>
      </div>
      <div class="card__body">
        <div class="steps">
          <div class="step">
            <span class="step__num">1</span>
            <div class="step__main">
              <div class="step__title">安装脚本管理器（脚本猫）</div>
              <div class="step__desc">
                在 Edge 加载项商店点「获取」安装，装完在浏览器右上角的扩展菜单里确认脚本猫已启用。
              </div>
              <div class="step__actions">
                <button class="btn" type="button" @click="open(SCRIPT_MANAGER_URL)">
                  <AppIcon name="external" :size="13" />
                  打开 Edge 加载项
                </button>
              </div>
            </div>
          </div>

          <div class="step">
            <span class="step__num">2</span>
            <div class="step__main">
              <div class="step__title">
                安装 OCS 脚本（GitHub 最新版）
                <span v-if="release" class="badge badge--ai">{{ release.version }}</span>
              </div>
              <div class="step__desc">
                从 OCS 官方 GitHub Releases 安装最新脚本；浏览器会弹出安装页，点「安装」即可。
                如果没有任何反应，说明第 1 步的脚本管理器还没装好。
              </div>
              <div class="step__actions">
                <button
                  class="btn btn--primary"
                  type="button"
                  :disabled="loadingRelease || !release"
                  @click="release && open(release.scriptUrl)"
                >
                  <AppIcon name="rocket" :size="13" />
                  安装 OCS 脚本
                </button>
                <button class="btn" type="button" @click="open(OCS_RELEASES_URL)">
                  <AppIcon name="external" :size="13" />
                  查看 Releases
                </button>
                <button class="btn btn--ghost" type="button" :disabled="loadingRelease" @click="loadRelease">
                  <AppIcon name="refresh" :size="13" :class="{ spin: loadingRelease }" />
                  重新获取版本
                </button>
              </div>
              <div v-if="release" class="step__meta mono">{{ release.scriptUrl }}</div>
            </div>
          </div>

          <div class="step">
            <span class="step__num">3</span>
            <div class="step__main">
              <div class="step__title">
                申请 AI 的 API Key
                <span class="badge badge--ok">推荐智谱 GLM</span>
              </div>
              <div class="step__desc">
                打开智谱开放平台注册并完成实名认证，在「API Keys」里创建一个 Key（新用户有免费额度）。
                复制生成的 Key，下一步直接粘贴到本软件里即可，无需手动填地址。
              </div>
              <div class="step__actions">
                <button class="btn btn--primary" type="button" @click="open(glmHomepage)">
                  <AppIcon name="external" :size="13" />
                  去智谱开放平台领 Key
                </button>
                <button class="btn" type="button" @click="copy(glmBaseUrl, '接口地址')">
                  <AppIcon name="copy" :size="13" />
                  复制接口地址
                </button>
              </div>
              <div class="step__meta mono">{{ glmBaseUrl }}</div>
            </div>
          </div>

          <div class="step">
            <span class="step__num" :class="{ 'step__num--done': aiReady }">
              {{ aiReady ? "✓" : "4" }}
            </span>
            <div class="step__main">
              <div class="step__title">
                把 Key 填进 ai2tiku
                <span class="badge" :class="aiReady ? 'badge--ok' : 'badge--off'">
                  {{ aiReady ? `已配置 ${modelCount} 个模型` : "待配置" }}
                </span>
              </div>
              <div class="step__desc">
                点击下面的按钮会打开「AI 供应商与模型」，并自动选中「智谱 AI (GLM)」预设；
                把刚才复制的 Key 粘进去保存，软件会自动拉取可用模型。
              </div>
              <div class="step__actions">
                <button class="btn btn--primary" type="button" @click="configureProvider">
                  <AppIcon name="sparkles" :size="13" />
                  {{ aiReady ? "查看 / 修改供应商" : "去配置 GLM 供应商" }}
                </button>
              </div>
            </div>
          </div>

          <div class="step">
            <span class="step__num" :class="{ 'step__num--done': status?.running && aiReady }">
              {{ status?.running && aiReady ? "✓" : "5" }}
            </span>
            <div class="step__main">
              <div class="step__title">把本地题库接进 OCS</div>
              <div class="step__desc">
                进入任意网课页面，打开 OCS 面板 →「题库配置」→ 粘贴下面的订阅链接并保存。
                之后脚本会自动使用本机的题库与 AI；页面上能看到 OCS 悬浮面板就说明部署成功。
              </div>
              <div class="code">{{ subscribeUrl }}</div>
              <div class="step__actions">
                <button class="btn btn--primary" type="button" @click="copy(subscribeUrl, '订阅链接')">
                  <AppIcon name="copy" :size="13" />
                  复制订阅链接
                </button>
                <button class="btn" type="button" @click="open(OCS_DOC_URL)">
                  <AppIcon name="external" :size="13" />
                  自动答题设置说明
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="callout callout--warn">
      <AppIcon name="info" :size="16" />
      <div>
        Edge 从 2025 下半年起要求开启「开发者模式」才能运行用户脚本：
        打开 <code class="mono">edge://extensions</code>，在左侧打开「开发人员模式」，然后重启浏览器。
      </div>
    </div>
  </div>
</template>
