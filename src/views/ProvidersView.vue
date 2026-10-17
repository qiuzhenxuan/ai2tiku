<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { api } from "../api";
import type { AiModel, ModelInput, Provider, ProviderInput } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import AppModal from "../components/AppModal.vue";
import { useToast } from "../composables/useToast";

const toast = useToast();

const providers = ref<Provider[]>([]);
const models = ref<AiModel[]>([]);
const testing = ref<number | null>(null);

const providerModal = ref(false);
const providerForm = reactive<ProviderInput>({
  id: null,
  name: "",
  baseUrl: "https://api.deepseek.com/v1",
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
  maxTokens: 1024,
  enabled: true,
  isDefault: false,
});

const providerName = computed(() => {
  const map: Record<number, string> = {};
  providers.value.forEach((p) => (map[p.id] = p.name));
  return map;
});

async function load() {
  try {
    const [p, m] = await Promise.all([api.listProviders(), api.listModels()]);
    providers.value = p;
    models.value = m;
  } catch (e) {
    toast.fromError(e);
  }
}

function openProvider(item?: Provider) {
  Object.assign(providerForm, {
    id: item?.id ?? null,
    name: item?.name ?? "",
    baseUrl: item?.baseUrl ?? "https://api.deepseek.com/v1",
    apiKey: item?.apiKey ?? "",
    enabled: item?.enabled ?? true,
  });
  providerModal.value = true;
}

async function submitProvider() {
  try {
    await api.saveProvider({ ...providerForm });
    providerModal.value = false;
    toast.ok("供应商已保存");
    await load();
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
    maxTokens: item?.maxTokens ?? 1024,
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
    <div class="callout">
      <AppIcon name="info" :size="16" />
      <div>
        支持任何 <strong>OpenAI 兼容</strong> 的服务，例如 DeepSeek、通义千问、Kimi、OneAPI、Ollama 本地模型。
        base_url 需写到 <code class="mono">/v1</code> 这一层，程序会自动拼接
        <code class="mono">/chat/completions</code>。
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
        没有它，题库只能靠手动录入的题目作答
      </div>
      <div v-else class="table-wrap">
        <table class="table">
          <thead>
            <tr>
              <th>名称</th>
              <th>base_url</th>
              <th>密钥</th>
              <th>状态</th>
              <th class="num">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="p in providers" :key="p.id">
              <td><strong>{{ p.name }}</strong></td>
              <td class="mono">{{ p.baseUrl }}</td>
              <td class="mono faint">{{ p.apiKey ? "••••" + p.apiKey.slice(-4) : "未填写" }}</td>
              <td>
                <span class="badge" :class="p.enabled ? 'badge--ok' : 'badge--off'">
                  <span class="dot" />
                  {{ p.enabled ? "启用" : "停用" }}
                </span>
              </td>
              <td>
                <div class="row" style="justify-content: flex-end">
                  <button class="btn btn--ghost btn--sm" type="button" :disabled="testing === p.id" @click="test(p)">
                    <AppIcon name="refresh" :size="13" :class="{ spin: testing === p.id }" />
                    测试
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" @click="openProvider(p)">
                    <AppIcon name="edit" :size="13" />
                    编辑
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
        <span class="card__hint">默认模型用于 AI 作答</span>
        <button class="btn btn--primary btn--sm" style="margin-left: auto" type="button" @click="openModel()">
          <AppIcon name="plus" :size="13" />
          新增模型
        </button>
      </div>

      <div v-if="models.length === 0" class="empty">
        <div class="empty__title">还没有可用模型</div>
        添加后记得把其中一个设为默认
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
                  <button v-if="!m.isDefault" class="btn btn--ghost btn--sm" type="button" @click="api.saveModel({ ...m, isDefault: true }).then(load)">
                    设为默认
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button" @click="openModel(m)">
                    <AppIcon name="edit" :size="13" />
                    编辑
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
          <label class="field__label" for="p-name">名称</label>
          <input id="p-name" v-model="providerForm.name" class="input" placeholder="例如 DeepSeek" />
        </div>
        <div class="field">
          <label class="field__label" for="p-url">base_url</label>
          <input id="p-url" v-model="providerForm.baseUrl" class="input" placeholder="https://api.deepseek.com/v1" />
          <span class="field__help">需包含 /v1 前缀，程序会自动请求 /chat/completions</span>
        </div>
        <div class="field">
          <label class="field__label" for="p-key">API Key</label>
          <input
            id="p-key"
            v-model="providerForm.apiKey"
            class="input"
            type="password"
            autocomplete="off"
            placeholder="sk-..."
          />
          <span class="field__help">仅保存在本机数据库，不会上传</span>
        </div>
        <label class="switch">
          <input v-model="providerForm.enabled" type="checkbox" />
          <span class="switch__track" />
          <span>启用该供应商</span>
        </label>
      </div>
      <template #foot>
        <button class="btn" type="button" @click="providerModal = false">取消</button>
        <button class="btn btn--primary" type="button" @click="submitProvider">保存</button>
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
          <input id="m-model" v-model="modelForm.model" class="input" placeholder="例如 deepseek-chat" />
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
