<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { api, ApiError, type AccessToken, type ModelSettings, type Page, type Provider, type TokenSecret, type User } from './api'
import Modal from './Modal.vue'
import GitHubLink from './GitHubLink.vue'

type Tab = 'overview' | 'connection' | 'questions' | 'records' | 'users' | 'audit'
const loading = ref(true), authenticated = ref(false), busy = ref(false), loadingPage = ref(false)
const error = ref(''), notice = ref(''), pageError = ref('')
const user = ref<User | null>(null)
const bootstrap = reactive({ registrationOpen: false, publicUrl: '', providers: [] as Provider[] })
const authMode = ref<'login' | 'register'>('login')
const auth = reactive({ email: '', password: '' })
const tab = ref<Tab>('overview')
const nav = computed(() => [
  { id: 'overview' as Tab, title: '工作概览', index: '01', glyph: '◫' },
  { id: 'connection' as Tab, title: '模型与接入', index: '02', glyph: '⌘' },
  { id: 'questions' as Tab, title: '我的题库', index: '03', glyph: '▤' },
  { id: 'records' as Tab, title: '答题记录', index: '04', glyph: '≡' },
  ...(user.value?.role === 'admin' ? [
    { id: 'users' as Tab, title: '用户管理', index: '05', glyph: '♧' },
    { id: 'audit' as Tab, title: '操作审计', index: '06', glyph: '↗' },
  ] : []),
])
const title = computed(() => nav.value.find(n => n.id === tab.value)?.title || '工作概览')
const stats = ref<Record<string, any>>({})
const model = reactive<ModelSettings>({ providerId: '', model: '', hasKey: false, temperature: 0.2, maxTokens: 2048 })
const savedProvider = ref(''), apiKey = ref('')
const selectedProvider = computed(() => bootstrap.providers.find(p => p.id === model.providerId))
const tokens = ref<AccessToken[]>([]), tokenName = ref('OCS 网课助手')
const activeTokens = computed(() => tokens.value.filter(t => !t.revoked))
const secret = ref<TokenSecret | null>(null), savedSecret = ref(false)
const detail = ref<Record<string, any> | null>(null)
const questions = ref<Page<any>>({ items: [], total: 0, page: 1, pageSize: 20 })
const records = ref<Page<any>>({ items: [], total: 0, page: 1, pageSize: 20 })
const questionQuery = ref(''), searchQuery = ref('')
const users = ref<User[]>([]), userQuery = ref(''), audit = ref<any[]>([])
const filteredUsers = computed(() => users.value.filter(u => u.email.toLowerCase().includes(userQuery.value.toLowerCase())))
const userEditor = ref<'create' | User | null>(null)
const userForm = reactive({ email: '', password: '', dailyLimit: 200, enabled: true })
const pendingConfirm = ref<{ title: string; text: string; action: () => Promise<void> } | null>(null)
const usagePercent = computed(() => !user.value || user.value.dailyLimit <= 0 ? 0 : Math.min(100, user.value.usedToday / user.value.dailyLimit * 100))
let loadSequence = 0

function message(cause: unknown) { return cause instanceof Error ? cause.message : '请求失败，请稍后重试。' }
function formatNumber(value: unknown) { return Number(value || 0).toLocaleString('zh-CN') }
function formatDate(value: unknown) {
  if (!value) return '尚未使用'
  const raw = String(value)
  const normalized = raw.replace(' ', 'T')
  const date = new Date(/(?:Z|[+-]\d\d:\d\d)$/i.test(normalized) ? normalized : `${normalized}Z`)
  return Number.isNaN(date.getTime()) ? raw : date.toLocaleString('zh-CN', { hour12: false })
}
function qtype(value: unknown) { return ({ single: '单选题', multiple: '多选题', judgement: '判断题', judge: '判断题', completion: '填空题' } as Record<string, string>)[String(value)] || String(value || '未标注题型') }
function options(value: unknown): string[] {
  if (Array.isArray(value)) return value.map(String)
  if (typeof value !== 'string' || !value) return []
  try { const parsed = JSON.parse(value); if (Array.isArray(parsed)) return parsed.map(String) } catch { /* Legacy text options remain readable. */ }
  return value.split(/\r?\n/).filter(Boolean)
}
function clearSecrets() { apiKey.value = ''; auth.password = ''; userForm.password = ''; secret.value = null; savedSecret.value = false }
function resetSession() {
  ++loadSequence; loadingPage.value = false
  clearSecrets(); user.value = null; authenticated.value = false; tokens.value = []
  users.value = []; audit.value = []; stats.value = {}; detail.value = null
  questions.value = { items: [], total: 0, page: 1, pageSize: 20 }; records.value = { items: [], total: 0, page: 1, pageSize: 20 }
  model.hasKey = false; model.model = ''; savedProvider.value = ''; tab.value = 'overview'
  pendingConfirm.value = null; userEditor.value = null
}
async function initialize() {
  loading.value = true; error.value = ''
  try {
    Object.assign(bootstrap, await api('/bootstrap'))
    try {
      user.value = await api<User>('/me'); authenticated.value = true
      await loadPage()
    } catch (cause) { if (!(cause instanceof ApiError && cause.status === 401)) throw cause }
  } catch (cause) { error.value = message(cause) }
  finally { loading.value = false }
}
async function signIn() {
  if (busy.value) return
  busy.value = true; error.value = ''
  try {
    await api(`/auth/${authMode.value}`, 'POST', { email: auth.email.trim(), password: auth.password })
    auth.password = ''; user.value = await api('/me'); authenticated.value = true
    await loadPage()
  } catch (cause) { error.value = message(cause); auth.password = '' }
  finally { busy.value = false }
}
async function signOut() {
  if (busy.value) return
  busy.value = true
  try { await api('/auth/logout', 'POST', {}); resetSession(); notice.value = ''; error.value = '' }
  catch (cause) { error.value = message(cause) }
  finally { busy.value = false }
}
async function loadPage() {
  const sequence = ++loadSequence
  const currentTab = tab.value
  loadingPage.value = true; pageError.value = ''
  try {
    const me = await api<User>('/me')
    if (sequence !== loadSequence) return
    user.value = me
    if (currentTab === 'overview' || currentTab === 'connection') {
      const [s, m, ts] = await Promise.all([api('/stats'), api<ModelSettings>('/model'), api<{ items: AccessToken[] }>('/tokens')])
      if (sequence !== loadSequence) return
      stats.value = s; Object.assign(model, m); savedProvider.value = m.providerId
      if (!model.providerId) model.providerId = bootstrap.providers[0]?.id || ''
      tokens.value = ts.items
    } else if (currentTab === 'questions') {
      const data = await api<Page<any>>(`/questions?page=${questions.value.page}&pageSize=20&q=${encodeURIComponent(searchQuery.value)}`)
      if (sequence === loadSequence) questions.value = data
    } else if (currentTab === 'records') {
      const data = await api<Page<any>>(`/records?page=${records.value.page}&pageSize=20`)
      if (sequence === loadSequence) records.value = data
    } else if (currentTab === 'users') {
      const data = await api<{ items: User[] }>('/admin/users'); if (sequence === loadSequence) users.value = data.items
    } else if (currentTab === 'audit') {
      const data = await api<{ items: any[] }>('/admin/audit'); if (sequence === loadSequence) audit.value = data.items
    }
  } catch (cause) {
    if (sequence !== loadSequence) return
    if (cause instanceof ApiError && [401, 403].includes(cause.status)) { resetSession(); error.value = '登录已过期或账户访问权限已改变，请重新登录。' }
    else pageError.value = message(cause)
  } finally { if (sequence === loadSequence) loadingPage.value = false }
}
function navigate(next: Tab) { if (next === tab.value) return; apiKey.value = ''; tab.value = next; notice.value = ''; void loadPage() }
async function saveModel() {
  if (busy.value) return
  busy.value = true; error.value = ''; notice.value = ''
  try {
    await api('/model', 'PUT', { providerId: model.providerId, model: model.model.trim(), temperature: Number(model.temperature), maxTokens: Number(model.maxTokens), ...(apiKey.value ? { apiKey: apiKey.value } : {}) })
    apiKey.value = ''; model.hasKey = true; savedProvider.value = model.providerId
    notice.value = '模型配置已保存。仅收到答题请求时才调用模型。'
  } catch (cause) { error.value = message(cause) }
  finally { apiKey.value = ''; busy.value = false }
}
async function createToken() {
  if (busy.value) return
  busy.value = true; error.value = ''
  try {
    secret.value = await api<TokenSecret>('/tokens', 'POST', { name: tokenName.value.trim() })
    savedSecret.value = false
    const response = await api<{ items: AccessToken[] }>('/tokens'); tokens.value = response.items
  } catch (cause) { error.value = message(cause) }
  finally { busy.value = false }
}
function closeSecret() {
  if (!savedSecret.value) return
  secret.value = null; savedSecret.value = false
}
function confirmRevoke(token: AccessToken) {
  pendingConfirm.value = { title: '撤销访问令牌', text: `撤销「${token.name}」后，使用此令牌的 OCS 配置会立即失效。需要继续使用时，请生成新令牌并替换配置。`, action: async () => {
    await api(`/tokens/${token.id}`, 'DELETE'); token.revoked = true; notice.value = '访问令牌已撤销。'
  } }
}
async function confirmAction() {
  if (!pendingConfirm.value || busy.value) return
  busy.value = true; error.value = ''
  try { await pendingConfirm.value.action(); pendingConfirm.value = null }
  catch (cause) { error.value = message(cause) }
  finally { busy.value = false }
}
async function copy(value: string, success: string) {
  try { await navigator.clipboard.writeText(value); notice.value = success }
  catch { notice.value = '剪贴板不可用，请选中文本手动复制。' }
}
function downloadConfig() {
  if (!secret.value) return
  const url = URL.createObjectURL(new Blob([JSON.stringify(secret.value.ocsConfig, null, 2)], { type: 'application/json' }))
  const anchor = document.createElement('a'); anchor.href = url; anchor.download = 'ai2tiku-ocs-config.json'; anchor.click()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
  notice.value = '配置已下载，请妥善保存其中的访问令牌。'
}
function openUserEditor(value: 'create' | User) {
  userEditor.value = value
  Object.assign(userForm, { email: value === 'create' ? '' : value.email, password: '', dailyLimit: value === 'create' ? 200 : value.dailyLimit, enabled: value === 'create' ? true : value.enabled })
}
async function saveUser() {
  if (!userEditor.value || busy.value) return
  busy.value = true; error.value = ''
  try {
    if (userEditor.value === 'create') await api('/admin/users', 'POST', { email: userForm.email.trim(), password: userForm.password, dailyLimit: Number(userForm.dailyLimit) })
    else await api(`/admin/users/${userEditor.value.id}`, 'PATCH', { enabled: userForm.enabled, dailyLimit: Number(userForm.dailyLimit), ...(userForm.password ? { password: userForm.password } : {}) })
    userEditor.value = null; userForm.password = ''; notice.value = '用户信息已保存。'
    await loadPage()
  } catch (cause) { error.value = message(cause) }
  finally { userForm.password = ''; busy.value = false }
}
function searchQuestions() { questions.value.page = 1; searchQuery.value = questionQuery.value.trim(); void loadPage() }
function changePage(kind: 'questions' | 'records', delta: number) {
  const target = kind === 'questions' ? questions : records
  target.value.page += delta; void loadPage()
}
onMounted(() => { void initialize(); window.addEventListener('pagehide', clearSecrets) })
onBeforeUnmount(() => { clearSecrets(); window.removeEventListener('pagehide', clearSecrets) })
</script>

<template>
  <div id="workspace" class="site-shell">
    <template v-if="!authenticated">
      <header class="landing-header"><a class="brand" href="/" aria-label="无界题库首页"><img src="/infinity-brush.png" alt="" /><span>无界题库<small>AI2TIKU / CLOUD</small></span></a><div class="header-actions"><span class="edition">私人模型 · 专属题库</span><GitHubLink /></div></header>
      <main class="login-layout">
        <section class="manifesto"><span class="eyebrow">连接知识，不设边界</span><h1>让你的模型，<br />成为你的<span>题库。</span></h1><p class="manifesto-copy">接入自己的模型 Key，生成独立访问令牌。<br />把可直接使用的配置交给 OCS，余下的在这里管理。</p><div class="manifesto-foot"><span>01 / 配置模型</span><span>02 / 生成令牌</span><span>03 / 接入 OCS</span></div><img class="ink-mark" src="/infinity-brush.png" alt="" aria-hidden="true" /></section>
        <section class="login-card" aria-labelledby="login-title"><span class="eyebrow">YOUR WORKSPACE</span><h2 id="login-title">{{ authMode === 'login' ? '登录工作台' : '创建你的账户' }}</h2><p class="muted">{{ authMode === 'login' ? '题库、模型和访问权限，一处管理。' : '使用账户密码保护你的模型配置与专属题库。' }}</p>
          <div v-if="loading" role="status" class="loading-line">正在连接服务…</div>
          <form v-else @submit.prevent="signIn" class="stack-form">
            <label>邮箱<input v-model="auth.email" name="email" type="email" autocomplete="username" required maxlength="254" placeholder="you@example.com" /></label>
            <label>密码<input v-model="auth.password" name="password" type="password" :autocomplete="authMode === 'register' ? 'new-password' : 'current-password'" required minlength="12" maxlength="128" placeholder="请输入账户密码" /></label>
            <p v-if="error" class="inline-error" role="alert">{{ error }}</p>
            <button class="button primary full" :disabled="busy">{{ busy ? '正在处理…' : authMode === 'login' ? '进入工作台 →' : '创建账户 →' }}</button>
          </form>
          <button v-if="bootstrap.registrationOpen && !loading" class="text-button auth-switch" @click="authMode = authMode === 'login' ? 'register' : 'login'; error = ''">{{ authMode === 'login' ? '还没有账户？创建账户' : '已有账户，返回登录' }}</button>
          <p v-else-if="!loading" class="login-note">账户由管理员开通。<br />模型 Key 与题库访问令牌分别管理。</p>
          <button v-if="error && !bootstrap.providers.length" class="text-button" @click="initialize">重新连接服务</button>
        </section>
      </main><footer class="landing-footer"><span>无界 · 学而不止</span><span>自托管题库服务</span></footer>
    </template>

    <template v-else>
      <aside class="sidebar"><a class="brand" href="/" aria-label="无界题库首页"><img src="/infinity-brush.png" alt="" /><span>无界题库<small>PERSONAL CONSOLE</small></span></a><div class="workspace-label">你的工作空间</div><nav aria-label="主导航"><button v-for="item in nav" :key="item.id" :class="['nav-item', { active: tab === item.id }]" :aria-current="tab === item.id ? 'page' : undefined" @click="navigate(item.id)"><span class="nav-glyph" aria-hidden="true">{{ item.glyph }}</span><span>{{ item.title }}</span><small>{{ item.index }}</small></button></nav><div class="sidebar-foot"><div class="avatar">{{ user?.email.slice(0, 1).toUpperCase() }}</div><div class="account-caption"><strong :title="user?.email">{{ user?.email }}</strong><small>{{ user?.role === 'admin' ? '管理员账户' : '个人账户' }}</small></div><button class="icon-button" title="退出登录" aria-label="退出登录" :disabled="busy" @click="signOut">↪</button></div></aside>
      <main class="console-main">
        <header class="page-header"><div><span class="eyebrow">AI2TIKU / {{ user?.role === 'admin' && ['users', 'audit'].includes(tab) ? 'ADMINISTRATION' : 'WORKSPACE' }}</span><h1>{{ title }}<span class="heading-dot">.</span></h1></div><div class="header-actions"><GitHubLink /><button class="button subtle" :disabled="loadingPage || busy" @click="loadPage"><span aria-hidden="true" :class="{ spinning: loadingPage }">↻</span>{{ loadingPage ? '刷新中' : '刷新' }}</button></div></header>
        <div v-if="error && !secret && !userEditor && !pendingConfirm" class="banner error" role="alert">{{ error }}<button aria-label="关闭错误提示" class="icon-button" @click="error = ''">×</button></div>
        <div v-if="notice && !secret" class="banner" role="status">{{ notice }}<button aria-label="关闭提示" class="icon-button" @click="notice = ''">×</button></div>
        <div v-if="pageError" class="banner error" role="alert">{{ pageError }}<button class="text-button" @click="loadPage">重试</button></div>
        <div v-if="loadingPage" class="progress-track" role="status" aria-label="正在加载数据"><span /></div>

        <template v-if="tab === 'overview'">
          <section class="overview-intro"><div><span class="section-kicker">从一次连接开始</span><h2>你的模型，你的答案。</h2><p>访问令牌负责连接，模型 Key 留在服务端。<br />所有题库与答题记录仅在你的账户下管理。</p><button class="button primary" @click="navigate('connection')">{{ model.hasKey ? '管理模型与令牌' : '开始配置模型' }} <span aria-hidden="true">↗</span></button></div><div class="intro-mark" aria-hidden="true"><img src="/infinity-brush.png" alt="" /><span>CONNECTED KNOWLEDGE</span></div></section>
          <section class="metric-grid" aria-label="题库统计"><article class="metric"><span>已积累题目</span><strong>{{ formatNumber(stats.questionTotal) }}<small>题</small></strong><p>当前账户独立题库</p></article><article class="metric"><span>今日请求</span><strong>{{ formatNumber(user?.usedToday) }}<small>次</small></strong><p>每日上限 {{ formatNumber(user?.dailyLimit) }} 次</p></article><article class="metric"><span>请求成功率</span><strong>{{ (Number(stats.successRate || 0) * 100).toFixed(1) }}<small>%</small></strong><p>成功返回答案的请求比例</p></article><article class="metric"><span>累计 Token</span><strong>{{ formatNumber(stats.totalTokens) }}</strong><p>模型返回的用量统计</p></article></section>
          <div class="two-columns"><section class="panel"><header class="panel-heading"><h2>接入准备</h2><span class="tag">{{ model.hasKey && activeTokens.length ? '已就绪' : '待完善' }}</span></header><ol class="setup-list"><li><span :class="['step-number', { done: model.hasKey }]">{{ model.hasKey ? '✓' : '1' }}</span><div><strong>连接自己的模型</strong><p>{{ model.hasKey ? `${model.model} · Key 已保存` : '选择供应商，填入模型名称与 Key。' }}</p></div></li><li><span :class="['step-number', { done: activeTokens.length }]">{{ activeTokens.length ? '✓' : '2' }}</span><div><strong>生成题库访问令牌</strong><p>{{ activeTokens.length ? `已有 ${activeTokens.length} 个有效令牌` : '令牌和 OCS 配置仅在创建时展示一次。' }}</p></div></li><li><span class="step-number">3</span><div><strong>将配置导入 OCS</strong><p>在 OCS 题库配置中粘贴生成的 JSON。</p></div></li></ol><button class="text-button" @click="navigate('connection')">前往配置 <span aria-hidden="true">→</span></button></section><section class="panel"><header class="panel-heading"><h2>今日额度</h2><span class="tag">每日 08:00 重置（北京时间）</span></header><div class="quota-number">{{ formatNumber(user?.usedToday) }}<span>/ {{ formatNumber(user?.dailyLimit) }}</span></div><div class="quota-track" role="progressbar" :aria-valuenow="user?.usedToday" :aria-valuemax="user?.dailyLimit" aria-valuemin="0" aria-label="今日请求额度"><span :style="{ width: `${usagePercent}%` }" /></div><p class="muted small">额度用于限制题库接口请求；模型实际费用由你的模型供应商结算。</p><div class="fine-divider" /><div class="definition-row"><span>成功 / 失败</span><strong>{{ formatNumber(stats.successCount) }} / {{ formatNumber(stats.failureCount) }}</strong></div><div class="definition-row"><span>本地题库命中</span><strong>{{ formatNumber(stats.cacheHits) }} 次</strong></div></section></div>
        </template>

        <template v-if="tab === 'connection'">
          <div class="connection-grid"><section class="panel"><header class="panel-heading"><div><span class="section-kicker">01 / MODEL CONNECTION</span><h2>模型连接</h2></div><span :class="['tag', { solid: model.hasKey }]">{{ model.hasKey ? 'Key 已配置' : '尚未配置' }}</span></header><form class="stack-form" @submit.prevent="saveModel"><label>模型供应商<select v-model="model.providerId" required @change="model.model = selectedProvider?.models?.[0] || ''; apiKey = ''"><option disabled value="">选择供应商</option><option v-for="provider in bootstrap.providers" :key="provider.id" :value="provider.id">{{ provider.name }}</option></select></label><p class="field-note endpoint">{{ selectedProvider?.baseUrl || '由管理员配置允许连接的供应商' }}</p><label>模型名称<input v-model="model.model" list="provider-models" required maxlength="160" placeholder="填写供应商支持的模型名称" /><datalist id="provider-models"><option v-for="name in selectedProvider?.models || []" :key="name" :value="name" /></datalist></label><label>模型 API Key<input v-model="apiKey" type="password" name="model-api-key" autocomplete="off" :required="!model.hasKey || savedProvider !== model.providerId" maxlength="4096" :placeholder="model.hasKey && savedProvider === model.providerId ? '已保存；留空保留现有 Key' : '粘贴自己的模型 Key'" spellcheck="false" /></label><p class="field-note">保存后不回显原始 Key。更换供应商时需重新填写。</p><div class="form-row"><label>温度<input v-model.number="model.temperature" type="number" min="0" max="2" step="0.1" required /></label><label>最大输出 Token<input v-model.number="model.maxTokens" type="number" min="64" max="8192" step="1" required /></label></div><button class="button primary" :disabled="busy || loadingPage">{{ busy ? '正在保存…' : '保存模型配置' }}</button><p class="field-note">保存配置不会调用模型或产生模型费用。</p></form></section><section class="panel token-create"><span class="section-kicker">02 / ACCESS TOKEN</span><h2>一次生成，专属接入。</h2><p class="muted">OCS 使用专属访问令牌连接你的题库。<br />这个令牌与你的模型 API Key 分开管理。</p><form class="stack-form" @submit.prevent="createToken"><label>令牌名称<input v-model="tokenName" maxlength="80" required placeholder="例如：我的 OCS" /></label><button class="button primary" :disabled="busy || !model.hasKey || loadingPage">生成令牌与 OCS 配置 <span aria-hidden="true">↗</span></button><p v-if="!model.hasKey" class="field-note">请先完成左侧模型配置。</p></form><div class="important-note"><strong>只展示一次</strong><p>完整令牌和 OCS 配置只在创建成功后展示。关闭后无法找回，请及时复制或下载。丢失后撤销旧令牌，再生成新的配置。</p></div></section></div>
          <section class="panel table-panel"><header class="panel-heading"><h2>访问令牌</h2><span class="muted small">{{ activeTokens.length }} 个有效</span></header><div v-if="!tokens.length" class="empty-state"><span aria-hidden="true">⌘</span><h3>尚未创建访问令牌</h3><p>配置模型后，生成第一份 OCS 接入配置。</p></div><div v-else class="table-scroll"><table><thead><tr><th>名称 / 令牌标识</th><th>创建时间</th><th>最近使用</th><th>状态</th><th class="align-right">操作</th></tr></thead><tbody><tr v-for="token in tokens" :key="token.id"><td><strong>{{ token.name }}</strong><code class="table-code">{{ token.prefix }}••••</code></td><td>{{ formatDate(token.createdAt) }}</td><td>{{ formatDate(token.lastUsedAt) }}</td><td><span class="tag" :class="{ solid: !token.revoked }">{{ token.revoked ? '已撤销' : '有效' }}</span></td><td class="align-right"><button v-if="!token.revoked" class="button small-button" :disabled="busy" @click="confirmRevoke(token)">撤销</button><span v-else class="muted">—</span></td></tr></tbody></table></div></section>
        </template>

        <template v-if="tab === 'questions' || tab === 'records'">
          <section class="panel table-panel"><header class="panel-heading"><div><h2>{{ tab === 'questions' ? '个人题库' : '请求流水' }}</h2><p class="muted small">{{ tab === 'questions' ? '仅显示当前账户积累的题目。' : '查看成功与失败请求，定位模型和接入问题。' }}</p></div><form v-if="tab === 'questions'" class="inline-search" @submit.prevent="searchQuestions"><input v-model="questionQuery" type="search" aria-label="搜索题干" placeholder="搜索题干…" maxlength="200" /><button class="button" :disabled="loadingPage">搜索</button></form></header><div v-if="!(tab === 'questions' ? questions.items : records.items).length" class="empty-state"><span aria-hidden="true">▤</span><h3>{{ searchQuery && tab === 'questions' ? '没有找到匹配题目' : '这里还没有记录' }}</h3><p>{{ tab === 'questions' ? '使用 OCS 发起答题后，题目会逐渐积累在这里。' : '使用访问令牌发起的答题请求会记录在这里。' }}</p></div><div v-else class="table-scroll"><table><thead><tr><th>题目</th><th>{{ tab === 'questions' ? '题型' : '结果' }}</th><th>{{ tab === 'questions' ? '答案' : '来源 / 模型' }}</th><th>{{ tab === 'questions' ? '更新时间' : '请求时间' }}</th><th class="align-right">详情</th></tr></thead><tbody><tr v-for="row in tab === 'questions' ? questions.items : records.items" :key="row.id"><td class="question-cell"><button class="question-link" @click="detail = { ...row, kind: tab }">{{ row.title || row.question || '未记录题干' }}</button><small v-if="row.error" class="error-preview">{{ row.error }}</small></td><td><span class="tag" :class="{ failure: tab === 'records' && row.status === 'failure' }">{{ tab === 'questions' ? qtype(row.qtype) : row.status === 'failure' ? '失败' : '成功' }}</span></td><td><span class="answer-preview">{{ tab === 'questions' ? row.answer : row.model || row.source || '本地题库' }}</span></td><td>{{ formatDate(tab === 'questions' ? row.updatedAt || row.createdAt : row.createdAt) }}</td><td class="align-right"><button class="icon-button" :aria-label="`查看题目详情：${row.title || row.question || ''}`" @click="detail = { ...row, kind: tab }">↗</button></td></tr></tbody></table></div><footer class="pagination"><span>共 {{ formatNumber(tab === 'questions' ? questions.total : records.total) }} 条</span><div><button class="button small-button" :disabled="loadingPage || (tab === 'questions' ? questions.page : records.page) <= 1" @click="changePage(tab as 'questions' | 'records', -1)">上一页</button><span>{{ tab === 'questions' ? questions.page : records.page }} / {{ Math.max(1, Math.ceil((tab === 'questions' ? questions.total : records.total) / 20)) }}</span><button class="button small-button" :disabled="loadingPage || (tab === 'questions' ? questions.page * 20 >= questions.total : records.page * 20 >= records.total)" @click="changePage(tab as 'questions' | 'records', 1)">下一页</button></div></footer></section>
        </template>

        <template v-if="tab === 'users' && user?.role === 'admin'">
          <div class="section-intro"><p>管理所有账户的访问权限与每日请求额度。<br /><span class="muted">用户的模型 Key 与完整访问令牌不会在管理后台展示。</span></p><button class="button primary" @click="openUserEditor('create')">＋ 创建用户</button></div><section class="panel table-panel"><header class="panel-heading"><h2>全部用户 <span class="count-badge">{{ users.length }}</span></h2><input v-model="userQuery" class="search-input" type="search" aria-label="搜索用户邮箱" placeholder="搜索邮箱…" /></header><div v-if="!filteredUsers.length" class="empty-state"><h3>没有匹配的用户</h3></div><div v-else class="table-scroll"><table><thead><tr><th>账户</th><th>角色</th><th>今日用量 / 额度</th><th>状态</th><th class="align-right">操作</th></tr></thead><tbody><tr v-for="account in filteredUsers" :key="account.id"><td><strong>{{ account.email }}</strong><small v-if="account.id === user?.id" class="table-code">当前账户</small></td><td>{{ account.role === 'admin' ? '管理员' : '用户' }}</td><td>{{ formatNumber(account.usedToday) }} / {{ formatNumber(account.dailyLimit) }}</td><td><span class="tag" :class="{ solid: account.enabled }">{{ account.enabled ? '正常' : '已停用' }}</span></td><td class="align-right"><button class="button small-button" @click="openUserEditor(account)">管理</button></td></tr></tbody></table></div></section>
        </template>
        <template v-if="tab === 'audit' && user?.role === 'admin'">
          <section class="panel table-panel"><header class="panel-heading"><h2>管理操作审计</h2><span class="muted small">记录账户、令牌与配置操作</span></header><div v-if="!audit.length" class="empty-state"><h3>暂无操作记录</h3></div><div v-else class="table-scroll"><table><thead><tr><th>操作</th><th>操作者 ID</th><th>目标 ID</th><th>时间</th></tr></thead><tbody><tr v-for="entry in audit" :key="entry.id"><td><code>{{ entry.action }}</code></td><td>{{ entry.actorId ?? '系统' }}</td><td>{{ entry.targetId ?? '—' }}</td><td>{{ formatDate(entry.createdAt) }}</td></tr></tbody></table></div></section>
        </template>
        <footer class="console-footer"><span>无界 · 学而不止</span><span>AI2TIKU / SELF-HOSTED</span></footer>
      </main>
    </template>
  </div>

  <Modal v-if="secret" title="访问令牌已生成" wide :closeable="false">
    <p class="modal-lead">完整令牌与 OCS 配置仅展示这一次。请先复制或下载，再关闭窗口。</p><div class="secret-notice">关闭后无法重新查看；丢失后需要撤销并生成新令牌。</div><label class="secret-label">题库访问令牌<input readonly :value="secret.token" autocomplete="off" spellcheck="false" @focus="($event.target as HTMLInputElement).select()" /></label><div class="button-row"><button class="button" @click="copy(secret.token, '访问令牌已复制。')">复制访问令牌</button><button class="button" @click="copy(JSON.stringify(secret.ocsConfig, null, 2), 'OCS 配置已复制，可粘贴到 OCS 题库设置。')">复制 OCS 配置</button><button class="button primary" @click="downloadConfig">下载 OCS 配置</button></div><label class="secret-label">OCS 题库配置<textarea class="config-code" readonly rows="9" :value="JSON.stringify(secret.ocsConfig, null, 2)" spellcheck="false" @focus="($event.target as HTMLTextAreaElement).select()" /></label><p v-if="notice" role="status" class="inline-status">{{ notice }}</p><p v-if="error" role="alert" class="inline-error">{{ error }}</p><label class="checkbox-label"><input v-model="savedSecret" type="checkbox" />我已保存，知道关闭后无法再次查看</label><footer class="modal-actions"><button class="button primary" :disabled="!savedSecret" @click="closeSecret">完成并关闭</button></footer>
  </Modal>
  <Modal v-else-if="pendingConfirm" :title="pendingConfirm.title" :closeable="!busy" @close="pendingConfirm = null; error = ''"><p class="modal-lead">{{ pendingConfirm.text }}</p><p v-if="error" role="alert" class="inline-error">{{ error }}</p><footer class="modal-actions"><button class="button" :disabled="busy" autofocus @click="pendingConfirm = null">取消</button><button class="button danger" :disabled="busy" @click="confirmAction">{{ busy ? '处理中…' : '确认撤销' }}</button></footer></Modal>
  <Modal v-else-if="userEditor" :title="userEditor === 'create' ? '创建用户' : '管理用户'" :closeable="!busy" @close="userEditor = null; userForm.password = ''; error = ''"><form class="stack-form" @submit.prevent="saveUser"><label>邮箱<input v-model="userForm.email" type="email" required maxlength="254" :disabled="userEditor !== 'create'" autocomplete="off" /></label><label>{{ userEditor === 'create' ? '初始密码' : '设置新密码（留空保留）' }}<input v-model="userForm.password" type="password" :required="userEditor === 'create'" minlength="12" maxlength="128" autocomplete="new-password" :placeholder="userEditor === 'create' ? '至少 12 个字符' : '填写后使现有登录会话失效'" /></label><label>每日请求额度<input v-model.number="userForm.dailyLimit" type="number" min="1" max="1000000" step="1" required /></label><p class="field-note">每日额度为 1–1,000,000 次；停用账户可暂停访问。</p><label v-if="userEditor !== 'create'" class="checkbox-label"><input v-model="userForm.enabled" type="checkbox" :disabled="userEditor.id === user?.id" />允许账户登录与使用题库</label><p v-if="error" role="alert" class="inline-error">{{ error }}</p><footer class="modal-actions"><button type="button" class="button" :disabled="busy" @click="userEditor = null; userForm.password = ''">取消</button><button class="button primary" :disabled="busy">{{ busy ? '保存中…' : '保存用户' }}</button></footer></form></Modal>
  <Modal v-else-if="detail" title="题目详情" wide @close="detail = null"><div class="detail-meta"><span class="tag">{{ qtype(detail.qtype) }}</span><span v-if="detail.kind === 'records'" class="tag">{{ detail.status === 'failure' ? '失败请求' : '成功请求' }}</span><span class="muted small">{{ formatDate(detail.createdAt || detail.updatedAt) }}</span></div><h3 class="full-question">{{ detail.title || detail.question || '未记录题干' }}</h3><ol v-if="options(detail.options).length" class="question-options"><li v-for="(option, index) in options(detail.options)" :key="index"><span>{{ String.fromCharCode(65 + index) }}</span><p>{{ option }}</p></li></ol><div v-if="detail.answer" class="answer-block"><span class="section-kicker">返回答案</span><p>{{ detail.answer }}</p></div><div v-if="detail.explanation" class="detail-block"><h3>解析</h3><p>{{ detail.explanation }}</p></div><div v-if="detail.error" class="detail-block inline-error"><h3>失败原因</h3><p>{{ detail.error }}</p></div><div v-if="detail.model || detail.source" class="detail-block small muted">来源：{{ detail.model || detail.source }}<span v-if="detail.totalTokens"> · {{ formatNumber(detail.totalTokens) }} Token</span></div><footer class="modal-actions"><button class="button" @click="detail = null">关闭详情</button></footer></Modal>
</template>
