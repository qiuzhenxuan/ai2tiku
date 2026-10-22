<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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

const browser = ref<BrowserInfo | null>(null);
const status = ref<ServerStatus | null>(null);
const release = ref<OcsRelease | null>(null);
const loadingRelease = ref(false);

const subscribeUrl = computed(
  () => `${status.value?.address || `http://127.0.0.1:${status.value?.port ?? 3100}`}/ocs`,
);

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
    const [b, s] = await Promise.all([api.browserInfo(), api.serverStatus()]);
    browser.value = b;
    status.value = s;
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

async function copySubscribe() {
  try {
    await writeText(subscribeUrl.value);
    toast.ok("订阅链接已复制，粘贴到 OCS 的「题库配置」里保存即可");
  } catch (e) {
    toast.fromError(e);
  }
}

/** 一键开始：打开脚本管理器安装页，顺手把订阅链接复制好 */
async function quickStart() {
  await copySubscribe();
  await open(SCRIPT_MANAGER_URL);
}

onMounted(load);
</script>

<template>
  <div class="page">
    <div class="card">
      <div class="card__head">
        <AppIcon name="rocket" :size="15" />
        <span class="card__title">一键部署 OCS</span>
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
          <span class="badge badge--ai">
            <span class="dot" />
            {{ release ? `OCS 脚本 ${release.version}` : loadingRelease ? "获取最新版本…" : "版本获取失败" }}
          </span>
        </div>
        <p class="faint small" style="margin: 0">
          部署脚本后，OCS 会在网课页面里自动读题，把题目发到本地服务（{{ subscribeUrl }}）取答案，再自动填回页面。
        </p>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="database" :size="15" />
        <span class="card__title">部署步骤</span>
        <span class="card__hint">共三步，大约一分钟</span>
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
              <div v-if="release" class="step__meta mono">
                {{ release.scriptUrl }}
              </div>
            </div>
          </div>

          <div class="step">
            <span class="step__num">3</span>
            <div class="step__main">
              <div class="step__title">把本地题库接进 OCS</div>
              <div class="step__desc">
                进入任意网课页面，打开 OCS 面板 →「题库配置」→ 粘贴下面的订阅链接并保存。
                之后脚本会自动使用本机的题库与 AI。
              </div>
              <div class="code">{{ subscribeUrl }}</div>
              <div class="step__actions">
                <button class="btn btn--primary" type="button" @click="copySubscribe">
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
        脚本装好后进入学习通等网课页面，能看到 OCS 悬浮面板就说明部署成功。
      </div>
    </div>
  </div>
</template>
