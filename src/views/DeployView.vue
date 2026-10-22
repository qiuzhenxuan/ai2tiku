<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api } from "../api";
import type { BrowserInfo, OcsRelease } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import { useToast } from "../composables/useToast";
import { useOcsConnection } from "../composables/useOcsConnection";

/** 脚本猫（ScriptCat）Edge 加载项 */
const SCRIPT_MANAGER_URL =
  "https://microsoftedge.microsoft.com/addons/detail/scriptcat/liilgpjgabokdklappibcjfablkpcekh";
const OCS_RELEASES_URL = "https://github.com/ocsjs/ocsjs/releases/latest";
const OCS_DOC_URL = "https://docs.ocsjs.com/docs/work";
const SCRIPT_PERMISSION_URL = "https://docs.scriptcat.org/docs/use/open-dev/?browser=edge";

const toast = useToast();
const router = useRouter();

const browser = ref<BrowserInfo | null>(null);
const browserLoading = ref(true);
const browserError = ref("");
const { server: status, address, config, error, loading, busy, aiModel, aiEnabled, canCopy, refresh, copyConfig, startServer } = useOcsConnection();
const release = ref<OcsRelease | null>(null);
const loadingRelease = ref(false);
/** 默认推荐智谱 GLM，注册走邀请链接；正常情况下以预设里的 homepage 为准 */
const glmHomepage = ref(
  "https://www.bigmodel.cn/invite?icode=sgWVI0vRXdaQMNjdvoiGd0jPr3uHog9F4g5tjuOUqno%3D",
);
const glmBaseUrl = ref("https://open.bigmodel.cn/api/paas/v4");

const aiLabel = computed(() => aiEnabled.value ? `${aiModel.value?.provider} · ${aiModel.value?.model}`
  : aiModel.value ? "已配置模型，AI 兜底已关闭" : loading.value ? "读取中…" : "没有启用的供应商和模型");

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
  const [detected, presets] = await Promise.allSettled([api.browserInfo(), api.listProviderPresets()]);
  if (detected.status === "fulfilled") browser.value = detected.value;
  else browserError.value = String(detected.reason);
  browserLoading.value = false;
  if (presets.status === "fulfilled") {
    const glm = presets.value.find((p) => p.id === "zhipu");
    if (glm) {
      glmHomepage.value = glm.homepage;
      glmBaseUrl.value = glm.baseUrl;
    }
  }
}

async function open(url: string) {
  try {
    await api.openInEdge(url);
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

/** 所有安装步骤使用同一个 Edge 浏览器。配置在第 5 步复制。 */
async function quickStart() {
  await open(SCRIPT_MANAGER_URL);
}

/** 配置供应商：跳到供应商页并自动弹出「智谱 GLM」预设 */
function configureProvider() {
  router.push(aiModel.value ? { path: "/providers" } : { path: "/providers", query: { add: "zhipu" } });
}

onMounted(() => { void load(); void loadRelease(); });
</script>

<template>
  <div class="page">
    <div class="card">
      <div class="card__head">
        <AppIcon name="rocket" :size="15" />
        <span class="card__title">从零部署 OCS</span>
        <span class="card__hint">安装脚本、开启权限、配置模型后，在 OCS 中验证接入</span>
        <button class="btn btn--primary btn--sm" style="margin-left: auto" type="button" @click="quickStart">
          <AppIcon name="play" :size="12" />
          一键开始
        </button>
      </div>
      <div class="card__body stack">
        <div class="row row--wrap">
          <span class="badge" :class="browser?.edgeInstalled ? 'badge--ok' : 'badge--off'">
            <span class="dot" />
            {{ browserLoading ? "检测中…" : browser === null ? "浏览器检测失败" : browser.edgeInstalled ? "Edge 已安装" : "未检测到 Edge" }}
          </span>
          <span class="badge" :class="browser?.edgeDefault ? 'badge--ok' : 'badge--off'">
            <span class="dot" />
            默认浏览器：{{ browser?.defaultBrowser ?? (browserLoading ? "检测中…" : "未知") }}（部署使用 Edge）
          </span>
          <span class="badge" :class="status?.running ? 'badge--ok' : 'badge--off'">
            <span class="dot" :class="{ 'dot--pulse': status?.running }" />
            本地题库服务：{{ loading ? "读取中…" : error ? "读取失败" : status?.running ? `运行中 · ${status.port}` : "未启动" }}
          </span>
          <span class="badge" :class="aiEnabled ? 'badge--ok' : 'badge--off'">
            <span class="dot" />
            AI：{{ aiLabel }}
          </span>
          <span class="badge badge--ai">
            <span class="dot" />
            {{ release ? `OCS 脚本 ${release.version}` : loadingRelease ? "获取最新版本…" : "版本获取失败" }}
          </span>
        </div>
        <p class="faint small" style="margin: 0">
          所有安装链接会在 Edge 中打开。OCS 的本地题库连接需要单独验证，页面检测到本地服务和模型并不代表浏览器已经接入。
        </p>
        <div v-if="error" class="callout callout--warn">{{ error }}</div>
        <div v-if="browserError" class="callout callout--warn">浏览器检测失败：{{ browserError }}。可以重试打开 Edge，或手动在 Edge 中完成安装。</div>
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
              <div class="step__desc">
                <strong>还需要开启用户脚本权限：</strong>打开 Edge 扩展管理 → 脚本猫「详细信息」→ 开启「允许用户脚本」。
                旧版 Edge 或没有该选项时，开启扩展管理页的「开发人员模式」。
                然后关闭再开启脚本猫扩展，或重启浏览器，并刷新网课页面。
              </div>
              <div class="step__actions">
                <button class="btn" type="button" @click="open(SCRIPT_MANAGER_URL)">
                  <AppIcon name="external" :size="13" />
                  打开 Edge 加载项
                </button>
                <button class="btn" type="button" @click="open('edge://extensions/')">打开 Edge 扩展管理</button>
                <button class="btn btn--ghost" type="button" @click="open(SCRIPT_PERMISSION_URL)">用户脚本权限设置图解</button>
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
                若只下载了文件或没有弹出安装页，请检查脚本猫是否已启用、用户脚本权限是否已打开，以及下载地址是否被网络拦截。
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
                打开智谱开放平台，按平台提示完成注册和认证，在「API Keys」里创建一个 Key；可用额度和费用以平台账户显示为准。
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
            <span class="step__num" :class="{ 'step__num--done': aiEnabled }">
              {{ aiEnabled ? "✓" : "4" }}
            </span>
            <div class="step__main">
              <div class="step__title">
                把 Key 填进 ai2tiku
                <span class="badge" :class="aiEnabled ? 'badge--ok' : 'badge--off'">
                  {{ aiLabel }}
                </span>
              </div>
              <div class="step__desc">
                点击下面的按钮会打开「AI 供应商与模型」，并自动选中「智谱 AI (GLM)」预设；
                把刚才复制的 Key 粘进去保存，软件会自动拉取可用模型。
                请启用供应商和需要使用的对话模型；模型已配置后，仍需通过本地自测确认 Key 和调用权限可用。
              </div>
              <div class="step__actions">
                <button class="btn btn--primary" type="button" @click="configureProvider">
                  <AppIcon name="sparkles" :size="13" />
                  {{ aiModel ? "查看 / 修改供应商" : "去配置 GLM 供应商" }}
                </button>
              </div>
            </div>
          </div>

          <div class="step">
            <span class="step__num">5</span>
            <div class="step__main">
              <div class="step__title">把本地题库接进 OCS <span class="badge badge--off">待在 OCS 中验证</span></div>
              <div class="step__desc">
                进入受支持的网课页面，打开 OCS 面板 →「题库配置」→ 粘贴下面的完整 JSON 内容并保存。
                最新版 OCS 不接受订阅地址。保存后发起一次实际搜题，并在「答题记录」中核对对应题目及返回结果；出现悬浮面板只说明脚本已加载。
              </div>
              <div class="step__meta">{{ address ? `本地接口：${address}/api/search` : "本地接口信息尚未就绪" }}。更改端口后请重新复制 JSON 并在 OCS 中保存。</div>
              <div v-if="config" class="code" style="max-height: 240px; overflow: auto">{{ config }}</div>
              <div class="step__actions">
                <button class="btn btn--primary" type="button" :disabled="!canCopy || busy" @click="copyConfig">
                  <AppIcon name="copy" :size="13" />
                  复制题库 JSON 配置
                </button>
                <button v-if="!status?.running" class="btn" type="button" :disabled="busy || loading" @click="startServer">启动本地服务</button>
                <button class="btn" type="button" @click="refresh">刷新状态</button>
                <button class="btn" type="button" @click="router.push('/ocs')">本地链路自测</button>
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
        本软件无法替你读取或修改 Edge 的用户脚本权限。若 OCS 没有运行，请按第 1 步检查「允许用户脚本」或「开发人员模式」，
        并确认脚本猫及 OCS 脚本均已启用。
      </div>
    </div>
  </div>
</template>
