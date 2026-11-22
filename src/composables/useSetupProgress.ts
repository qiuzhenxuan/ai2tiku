import { computed, reactive, ref } from "vue";
import { api } from "../api";
import type { BrowserInfo, DeploymentStatus } from "../api/types";

/**
 * 接入进度：能自动检测的步骤由后端状态决定，
 * 浏览器里完成、软件无法检测的步骤由用户手动标记（存本地）。
 */
export type StepId = "browser" | "script" | "model" | "connect" | "verify";

const STORAGE_KEY = "ai2tiku:setup-marks";

type Marks = Partial<Record<"scriptcat" | "ocs" | "copied", boolean>>;

function readMarks(): Marks {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") ?? {};
  } catch {
    return {};
  }
}

const marks = reactive<Marks>(readMarks());
const browser = ref<BrowserInfo | null>(null);
const deployment = ref<DeploymentStatus | null>(null);
const recordCount = ref(0);
const loaded = ref(false);
let pending: Promise<void> | undefined;

function setMark(key: keyof Marks, value: boolean) {
  marks[key] = value;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(marks));
  } catch {
    /* ignore */
  }
}

function load(): Promise<void> {
  if (pending) return pending;
  pending = (async () => {
    const [b, d, r] = await Promise.allSettled([
      browser.value ? Promise.resolve(browser.value) : api.browserInfo(),
      api.deploymentStatus(),
      api.listRecords(1, 1),
    ]);
    if (b.status === "fulfilled") browser.value = b.value;
    if (d.status === "fulfilled") deployment.value = d.value;
    if (r.status === "fulfilled") recordCount.value = r.value.total;
    loaded.value = true;
    pending = undefined;
  })();
  return pending;
}

export interface SetupStep {
  id: StepId;
  title: string;
  hint: string;
  done: boolean;
}

export function useSetupProgress() {
  const modelReady = computed(() => !!deployment.value?.aiModel && !!deployment.value?.aiFallback);
  const serverRunning = computed(() => !!deployment.value?.server.running);

  const steps = computed<SetupStep[]>(() => [
    {
      id: "browser",
      title: "浏览器与脚本猫",
      hint: "在 Edge 安装脚本猫并开启用户脚本",
      done: !!browser.value?.edgeInstalled && !!marks.scriptcat,
    },
    {
      id: "script",
      title: "安装 OCS 脚本",
      hint: "从官方 Releases 安装最新版",
      done: !!marks.ocs,
    },
    {
      id: "model",
      title: "配置 AI 模型",
      hint: "选一个厂商，填入 API Key",
      done: modelReady.value,
    },
    {
      id: "connect",
      title: "接入 OCS",
      hint: "复制题库配置粘贴到 OCS",
      done: serverRunning.value && !!marks.copied,
    },
    {
      id: "verify",
      title: "验证答题",
      hint: "自测或在网课页发起一次搜题",
      done: recordCount.value > 0,
    },
  ]);

  const doneCount = computed(() => steps.value.filter((s) => s.done).length);
  const allDone = computed(() => doneCount.value === steps.value.length);
  const nextStep = computed(() => steps.value.find((s) => !s.done) ?? null);

  return {
    steps,
    doneCount,
    allDone,
    nextStep,
    marks,
    setMark,
    browser,
    deployment,
    modelReady,
    loaded,
    load,
  };
}
