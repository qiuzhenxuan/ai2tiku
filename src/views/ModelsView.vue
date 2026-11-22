<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useConfirm } from "../composables/useConfirm";
import { useRoute } from "vue-router";
import { api } from "../api";
import type {
  AiModel,
  ModelInput,
  Provider,
  ProviderInput,
  ProviderPreset,
} from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import AppModal from "../components/AppModal.vue";
import { useToast } from "../composables/useToast";

const toast = useToast();
const route = useRoute();

const providers = ref<Provider[]>([]);
const models = ref<AiModel[]>([]);
const presets = ref<ProviderPreset[]>([]);
const testing = ref<number | null>(null);
const syncing = ref<number | null>(null);

const presetId = ref("custom");
const preset = computed(() => presets.value.find((p) => p.id === presetId.value));

const providerModal = ref(false);
const providerForm = reactive<ProviderInput>({
  id: null,
  name: "",
  baseUrl: "",
  apiKey: "",
  enabled: true,
});

const modelModal = ref(false);
const modelForm = reactive<ModelInput>({
  id: null,
  providerId: 0,
  model: "",
  displayName: "",
  temperature: 0.3,
  maxTokens: 2048,
  enabled: true,
  isDefault: false,
});


const modelsOf = (providerId: number) => models.value.filter((m) => m.providerId === providerId);
const settingDefault = ref<number | null>(null);

async function setDefault(m: AiModel) {
  settingDefault.value = m.id;
  try {
    await api.saveModel({ ...m, isDefault: true });
    toast.ok(`已将「${m.displayName || m.model}」设为默认模型`);
    await load();
  } catch (e) {
    toast.fromError(e);
  } finally {
    settingDefault.value = null;
  }
}

async function load() {
  try {
    const [p, m, ps] = await Promise.all([
      api.listProviders(),
      api.listModels(),
      api.listProviderPresets(),
    ]);
    providers.value = p;
    models.value = m;
    presets.value = ps;
  } catch (e) {
    toast.fromError(e);
  }
}

/** 选中预设后自动填好名称与 base_url，用户只需再填 Key */
function applyPreset(id: string) {
  presetId.value = id;
  const p = presets.value.find((x) => x.id === id);
  if (p && p.baseUrl) {
    providerForm.name = p.name;
    providerForm.baseUrl = p.baseUrl;
  }
}

function openProvider(item?: Provider) {
  presetId.value = "custom";
  Object.assign(providerForm, {
    id: item?.id ?? null,
    name: item?.name ?? "",
    baseUrl: item?.baseUrl ?? "",
    apiKey: item?.apiKey ?? "",
    enabled: item?.enabled ?? true,
  });
  providerModal.value = true;
}

/** 拉取供应商的模型列表并自动入库 */
async function syncModels(id: number, quiet = false) {
  syncing.value = id;
  try {
    const r = await api.syncProviderModels(id);
    if (r.added > 0) {
      toast.ok(`${r.provider}：已自动添加 ${r.added} 个模型（供应商共 ${r.total} 个）`);
    } else if (!quiet) {
      toast.info(`${r.provider}：模型已是最新，共 ${r.total} 个`);
    }
    await load();
  } catch (e) {
    // 拉取失败不影响供应商本身可用，提示用户可手动添加
    const msg = typeof e === "string" ? e : String(e);
    toast.error(`自动获取模型失败：${msg}`);
  } finally {
    syncing.value = null;
  }
}

async function submitProvider() {
  try {
    const id = await api.saveProvider({ ...providerForm });
    providerModal.value = false;
    toast.ok("供应商已保存");
    await load();
    // 保存后自动拉取可用模型并填入
    await syncModels(id);
  } catch (e) {
    toast.fromError(e);
  }
}

async function removeProvider(item: Provider) {
  if (!await useConfirm()({ title: '删除供应商？', message: `将删除供应商「${item.name}」及其下所有模型。`, confirmText: '删除供应商', danger: true })) return;
  try {
    await api.deleteProvider(item.id);
    toast.ok("已删除");
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

function openModel(item?: AiModel, providerId?: number) {
  if (providers.value.length === 0) {
    toast.info("请先添加一个 AI 供应商");
    return;
  }
  Object.assign(modelForm, {
    id: item?.id ?? null,
    providerId: item?.providerId ?? providerId ?? providers.value[0].id,
    model: item?.model ?? "",
    displayName: item?.displayName ?? "",
    temperature: item?.temperature ?? 0.3,
    maxTokens: item?.maxTokens ?? 2048,
    enabled: item?.enabled ?? true,
    isDefault: item?.isDefault ?? false,
  });
  modelModal.value = true;
}

async function submitModel() {
  try {
    await api.saveModel({ ...modelForm });
    modelModal.value = false;
    toast.ok("模型已保存");
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

async function removeModel(item: AiModel) {
  if (!await useConfirm()({ title: '删除模型？', message: `将删除模型「${item.displayName || item.model}」。`, confirmText: '删除模型', danger: true })) return;
  try {
    await api.deleteModel(item.id);
    toast.ok("已删除");
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

async function test(item: Provider) {
  testing.value = item.id;
  try {
    toast.ok(`${item.name}：${await api.testProvider(item.id)}`);
  } catch (e) {
    toast.fromError(e);
  } finally {
    testing.value = null;
  }
}

onMounted(async () => {
  await load();
  // 从「一键部署」跳过来时带 ?add=zhipu，直接弹出新增框并选好预设
  const add = route.query.add;
  if (typeof add === "string" && presets.value.some((p) => p.id === add)) {
    openProvider();
    applyPreset(add);
  }
});
</script>

<template>
  <div class="page">
    <div class="page-tools">
      <span class="faint small">选一个预置厂商只需填 Key；保存后自动获取可用模型。默认模型用于 AI 作答。</span>
      <button class="btn btn--primary" type="button" @click="openProvider()">
        <AppIcon name="plus" :size="13" />
        新增供应商
      </button>
    </div>

    <div v-if="providers.length === 0" class="card empty empty--hero">
      <span class="empty__icon"><AppIcon name="sparkles" :size="22" /></span>
      <div class="empty__title">还没有配置 AI 供应商</div>
      选一个预置厂商、粘贴 API Key，base_url 与模型都会自动就位
      <div><button class="btn btn--primary" type="button" style="margin-top: 14px" @click="openProvider()">添加第一个供应商</button></div>
    </div>

    <section v-for="p in providers" :key="p.id" class="card provider" :class="{ 'is-disabled': !p.enabled }">
      <header class="provider__head">
        <span class="provider__avatar">{{ p.name.slice(0, 1) }}</span>
        <div class="provider__meta">
          <div class="row" style="gap: 8px">
            <strong class="provider__name">{{ p.name }}</strong>
            <span class="badge" :class="p.enabled ? 'badge--ok' : 'badge--off'"><span class="dot" />{{ p.enabled ? "启用" : "停用" }}</span>
          </div>
          <div class="provider__sub mono">
            {{ p.baseUrl }} · <span :class="{ 'text-warn': !p.apiKey }">{{ p.apiKey ? "Key ••••" + p.apiKey.slice(-4) : "未填写 Key" }}</span>
          </div>
        </div>
        <div class="row provider__actions">
          <button class="btn btn--ghost btn--sm" type="button" :disabled="testing === p.id" @click="test(p)">
            <AppIcon :name="testing === p.id ? 'refresh' : 'bolt'" :size="13" :class="{ spin: testing === p.id }" />
            测试连通
          </button>
          <button class="btn btn--ghost btn--sm" type="button" :disabled="syncing === p.id" title="重新拉取该供应商的模型列表" @click="syncModels(p.id)">
            <AppIcon name="refresh" :size="13" :class="{ spin: syncing === p.id }" />
            拉取模型
          </button>
          <button class="btn btn--ghost btn--sm btn--icon" type="button" :aria-label="`编辑供应商：${p.name}`" title="编辑供应商" @click="openProvider(p)">
            <AppIcon name="edit" :size="13" />
          </button>
          <button class="btn btn--ghost btn--sm btn--icon" type="button" :aria-label="`删除供应商：${p.name}`" title="删除供应商" @click="removeProvider(p)">
            <AppIcon name="trash" :size="13" />
          </button>
        </div>
      </header>

      <ul v-if="modelsOf(p.id).length" class="model-list">
        <li v-for="m in modelsOf(p.id)" :key="m.id" class="model-row" :class="{ 'is-default': m.isDefault, 'is-disabled': !m.enabled }">
          <button
            class="model-row__radio"
            type="button"
            :aria-pressed="m.isDefault"
            :title="m.isDefault ? '当前默认模型' : '设为默认模型'"
            :disabled="m.isDefault || settingDefault === m.id"
            @click="setDefault(m)"
          >
            <span />
          </button>
          <div class="model-row__main">
            <strong>{{ m.displayName || m.model }}</strong>
            <span v-if="m.isDefault" class="badge badge--ai">默认</span>
            <span v-if="!m.enabled" class="badge badge--off">停用</span>
            <span v-if="m.displayName && m.displayName !== m.model" class="mono faint small">{{ m.model }}</span>
          </div>
          <span class="model-row__params faint small">温度 {{ m.temperature }} · 最大 {{ m.maxTokens }}</span>
          <div class="row model-row__actions">
            <button class="btn btn--ghost btn--sm btn--icon" type="button" :aria-label="`编辑模型：${m.displayName || m.model}`" title="编辑模型" @click="openModel(m)">
              <AppIcon name="edit" :size="13" />
            </button>
            <button class="btn btn--ghost btn--sm btn--icon" type="button" :aria-label="`删除模型：${m.displayName || m.model}`" title="删除模型" @click="removeModel(m)">
              <AppIcon name="trash" :size="13" />
            </button>
          </div>
        </li>
      </ul>
      <div v-else class="provider__empty faint small">
        还没有模型。点「拉取模型」自动获取，或手动添加。
      </div>
      <footer class="provider__foot">
        <button class="btn btn--ghost btn--sm" type="button" @click="openModel(undefined, p.id)">
          <AppIcon name="plus" :size="12" />
          手动添加模型
        </button>
      </footer>
    </section>

    <AppModal :open="providerModal" title="AI 供应商" @close="providerModal = false">
      <div class="stack">
        <div class="field">
          <label class="field__label" for="p-preset">预置供应商</label>
          <select id="p-preset" class="select" :value="presetId" @change="applyPreset(($event.target as HTMLSelectElement).value)">
            <option v-for="ps in presets" :key="ps.id" :value="ps.id">{{ ps.name }}</option>
          </select>
          <span class="field__help">选中后会自动填好名称与 base_url，只需再填 API Key</span>
        </div>

        <div v-if="preset && preset.note" class="callout">
          <AppIcon name="info" :size="15" />
          <div>
            {{ preset.note }}
            <template v-if="preset.homepage">
              <br />申请 Key：<a :href="preset.homepage" target="_blank" rel="noreferrer">{{ preset.homepage }}</a>
            </template>
          </div>
        </div>

        <div class="field">
          <label class="field__label" for="p-name">名称</label>
          <input id="p-name" v-model="providerForm.name" class="input" placeholder="例如 智谱 AI (GLM)" />
        </div>
        <div class="field">
          <label class="field__label" for="p-url">base_url</label>
          <input id="p-url" v-model="providerForm.baseUrl" class="input" placeholder="https://open.bigmodel.cn/api/paas/v4" />
          <span class="field__help">需包含 /v1（或厂商对应的前缀），程序会自动请求 /chat/completions 与 /models</span>
        </div>
        <div class="field">
          <label class="field__label" for="p-key">API Key</label>
          <input
            id="p-key"
            v-model="providerForm.apiKey"
            class="input"
            type="password"
            autocomplete="off"
            placeholder="粘贴你的 API Key"
          />
          <span class="field__help">只保存在本机数据库；Ollama 等本地服务可留空</span>
        </div>
        <label class="switch">
          <input v-model="providerForm.enabled" type="checkbox" />
          <span class="switch__track" />
          <span>启用该供应商</span>
        </label>
      </div>
      <template #foot>
        <button class="btn" type="button" @click="providerModal = false">取消</button>
        <button class="btn btn--primary" type="button" @click="submitProvider">
          <AppIcon name="check" :size="14" />
          保存并自动获取模型
        </button>
      </template>
    </AppModal>

    <AppModal :open="modelModal" title="模型" @close="modelModal = false">
      <div class="stack">
        <div class="field">
          <label class="field__label" for="m-provider">所属供应商</label>
          <select id="m-provider" v-model.number="modelForm.providerId" class="select">
            <option v-for="p in providers" :key="p.id" :value="p.id">{{ p.name }}</option>
          </select>
        </div>
        <div class="field">
          <label class="field__label" for="m-model">模型 ID</label>
          <input id="m-model" v-model="modelForm.model" class="input" placeholder="例如 glm-4-flash" />
        </div>
        <div class="field">
          <label class="field__label" for="m-display">显示名称（可选）</label>
          <input id="m-display" v-model="modelForm.displayName" class="input" placeholder="留空则使用模型 ID" />
        </div>
        <div class="grid grid--2">
          <div class="field">
            <label class="field__label" for="m-temp">温度</label>
            <input id="m-temp" v-model.number="modelForm.temperature" class="input" type="number" step="0.1" min="0" max="2" />
          </div>
          <div class="field">
            <label class="field__label" for="m-max">最大 Token</label>
            <input id="m-max" v-model.number="modelForm.maxTokens" class="input" type="number" min="64" step="64" />
          </div>
        </div>
        <div class="row row--wrap" style="gap: 18px">
          <label class="switch">
            <input v-model="modelForm.enabled" type="checkbox" />
            <span class="switch__track" />
            <span>启用</span>
          </label>
          <label class="switch">
            <input v-model="modelForm.isDefault" type="checkbox" />
            <span class="switch__track" />
            <span>设为默认模型</span>
          </label>
        </div>
      </div>
      <template #foot>
        <button class="btn" type="button" @click="modelModal = false">取消</button>
        <button class="btn btn--primary" type="button" @click="submitModel">保存</button>
      </template>
    </AppModal>
  </div>
</template>
