<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
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

const providerName = computed(() => {
  const map: Record<number, string> = {};
  providers.value.forEach((p) => (map[p.id] = p.name));
  return map;
});

const modelCount = computed(() => {
  const map: Record<number, number> = {};
  models.value.forEach((m) => (map[m.providerId] = (map[m.providerId] ?? 0) + 1));
  return map;
});

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
  if (!window.confirm(`删除供应商「${item.name}」及其下所有模型？`)) return;
  try {
    await api.deleteProvider(item.id);
    toast.ok("已删除");
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

function openModel(item?: AiModel) {
  if (providers.value.length === 0) {
    toast.info("请先添加一个 AI 供应商");
    return;
  }
  Object.assign(modelForm, {
    id: item?.id ?? null,
    providerId: item?.providerId ?? providers.value[0].id,
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
  if (!window.confirm(`删除模型「${item.displayName || item.model}」？`)) return;
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

onMounted(load);
</script>

<template>
  <div class="page">
    <div class="callout callout--info">
      <AppIcon name="info" :size="16" />
      <div>
        <div class="callout__title">只需填 Key 就能用</div>
        在「新增供应商」里选一个预置厂商，名称与 <code class="mono">base_url</code> 会自动填好，
        你只要粘贴 API Key。<strong>保存后会自动拉取该厂商的可用模型并填入</strong>。
        预置没有的其他服务选「自定义」，手动填 <code class="mono">base_url</code>（写到
        <code class="mono">/v1</code> 这一层）即可。
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="globe" :size="15" />
        <span class="card__title">AI 供应商</span>
        <span class="card__hint">{{ providers.length }} 个</span>
        <button class="btn btn--primary btn--sm" style="margin-left: auto" type="button" @click="openProvider()">
          <AppIcon name="plus" :size="13" />
          新增供应商
        </button>
      </div>

      <div v-if="providers.length === 0" class="empty">
        <div class="empty__title">还没有配置 AI 供应商</div>
        选一个预置厂商、填上 Key 即可，base_url 与模型都会自动就位
      </div>
      <div v-else class="table-wrap">
        <table class="table">
          <thead>
            <tr>
              <th>名称</th>
              <th>base_url</th>
              <th>密钥</th>
              <th class="num">模型</th>
              <th>状态</th>
              <th class="num">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="p in providers" :key="p.id">
              <td><strong>{{ p.name }}</strong></td>
              <td class="mono">{{ p.baseUrl }}</td>
              <td class="mono faint">{{ p.apiKey ? "••••" + p.apiKey.slice(-4) : "未填写" }}</td>
              <td class="num">{{ modelCount[p.id] ?? 0 }}</td>
              <td>
                <span class="badge" :class="p.enabled ? 'badge--ok' : 'badge--off'">
                  <span class="dot" />
                  {{ p.enabled ? "启用" : "停用" }}
                </span>
              </td>
              <td>
                <div class="row" style="justify-content: flex-end">
                  <button
                    class="btn btn--ghost btn--sm"
                    type="button"
                    :disabled="syncing === p.id"
                    title="重新拉取该供应商的模型列表"
                    @click="syncModels(p.id)"
                  >
                    <AppIcon name="refresh" :size="13" :class="{ spin: syncing === p.id }" />
                    拉取模型
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" :disabled="testing === p.id" @click="test(p)">
                    <AppIcon name="bolt" :size="13" />
                    测试
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" @click="openProvider(p)">
                    <AppIcon name="edit" :size="13" />
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" @click="removeProvider(p)">
                    <AppIcon name="trash" :size="13" />
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <div class="card__head">
        <AppIcon name="sparkles" :size="15" />
        <span class="card__title">模型</span>
        <span class="card__hint">默认模型用于 AI 作答，未设默认时自动取第一个</span>
        <button class="btn btn--primary btn--sm" style="margin-left: auto" type="button" @click="openModel()">
          <AppIcon name="plus" :size="13" />
          新增模型
        </button>
      </div>

      <div v-if="models.length === 0" class="empty">
        <div class="empty__title">还没有可用模型</div>
        添加供应商时若自动拉取成功，模型会出现在这里
      </div>
      <div v-else class="table-wrap">
        <table class="table">
          <thead>
            <tr>
              <th>模型</th>
              <th>供应商</th>
              <th class="num">温度</th>
              <th class="num">最大 Token</th>
              <th>状态</th>
              <th class="num">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="m in models" :key="m.id">
              <td>
                <div class="row">
                  <strong>{{ m.displayName || m.model }}</strong>
                  <span v-if="m.isDefault" class="badge badge--ai">默认</span>
                </div>
                <div v-if="m.displayName && m.displayName !== m.model" class="mono faint small">
                  {{ m.model }}
                </div>
              </td>
              <td class="faint">{{ providerName[m.providerId] ?? "-" }}</td>
              <td class="num">{{ m.temperature }}</td>
              <td class="num">{{ m.maxTokens }}</td>
              <td>
                <span class="badge" :class="m.enabled ? 'badge--ok' : 'badge--off'">
                  <span class="dot" />
                  {{ m.enabled ? "启用" : "停用" }}
                </span>
              </td>
              <td>
                <div class="row" style="justify-content: flex-end">
                  <button
                    v-if="!m.isDefault"
                    class="btn btn--ghost btn--sm"
                    type="button"
                    @click="api.saveModel({ ...m, isDefault: true }).then(load)"
                  >
                    设为默认
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" @click="openModel(m)">
                    <AppIcon name="edit" :size="13" />
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" @click="removeModel(m)">
                    <AppIcon name="trash" :size="13" />
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <AppModal v-if="providerModal" title="AI 供应商" @close="providerModal = false">
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

    <AppModal v-if="modelModal" title="模型" @close="modelModal = false">
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
