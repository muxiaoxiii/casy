<script setup lang="ts">
import { useFormBaseline } from '../../../composables/useFormBaseline'
import { computed, onMounted, ref, watch } from 'vue'
import { Plus, Delete, Connection, Check, RefreshLeft } from '../../../shared/icons'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useAiSettingsStore } from '../../../stores/aiSettings'
import { AI_PROMPTS } from '../../../core/prompts'
import { tauriCallSafe } from '../../../core/tauriBridge'
import AIStatusBadge from '../../../shared/components/AIStatusBadge.vue'
import { useRouter } from 'vue-router'
const router = useRouter()

const store = useAiSettingsStore()
const selectedId = ref('')
const selected = computed(() => store.config.profiles.find(p => p.id === selectedId.value))
const testing = ref(false)
const testResult = ref('')
const testFailed = ref(false)
const embeddingTesting = ref(false)
const embeddingResult = ref('')
const embeddingFailed = ref(false)
const embeddingEnabled = computed({
  get: () => !!store.config.embedding,
  set: enabled => { store.config.embedding = enabled ? { profileId: 'builtin-e5-base', model: 'multilingual-e5-base-int8', chunkChars: 384 } : null; embeddingResult.value = '' },
})
function changeEmbeddingProfile(id: string) {
  if (!store.config.embedding) return
  store.config.embedding.model = id === 'builtin-e5-base' ? 'multilingual-e5-base-int8' : ''
  store.config.embedding.chunkChars = id === 'builtin-e5-base' ? 384 : 1000
}
watch(() => JSON.stringify(selected.value), () => { testResult.value = '' })
const configDraft = useFormBaseline('AI 配置', () => store.config, () => store.loading, value => { store.config = value })
const legacyAvailable = ref(!!localStorage.getItem('casy_ai_settings'))
async function reload() {
  if (!(await configDraft.canLeave())) return
  if (await store.load()) { selectedId.value = store.config.activeId || store.config.profiles[0]?.id || ''; configDraft.markSaved() }
}
onMounted(reload)
function add() {
  const id = crypto.randomUUID()
  store.config.profiles.push({ id, name: '新配置', mode: 'openai', apiUrl: '', model: '', hasApiKey: false })
  selectedId.value = id
  if (store.config.profiles.length === 1) store.config.activeId = id
  testResult.value = ''
}
async function remove() {
  if (!selected.value) return
  try { await ElMessageBox.confirm(`删除配置“${selected.value.name}”？`, '删除 AI 配置', { type: 'warning' }) } catch { return }
  store.config.profiles = store.config.profiles.filter(p => p.id !== selectedId.value)
  if (store.config.activeId === selectedId.value) store.config.activeId = null
  if (store.config.embedding?.profileId === selectedId.value) store.config.embedding = null
  selectedId.value = store.config.profiles[0]?.id || ''
}
async function save() {
  if (await store.save()) { configDraft.markSaved(); ElMessage.success('AI 配置已保存') }
}
async function test() {
  if (!selected.value) return
  testing.value = true
  testResult.value = ''
  const result = await tauriCallSafe('test_ai_profile', { profile: { ...selected.value } })
  testing.value = false
  testFailed.value = !result.ok
  testResult.value = result.ok ? result.data || 'API 未返回结果' : result.error || '连接失败'
}
async function testEmbedding() {
  embeddingTesting.value = true
  embeddingResult.value = ''
  try {
    if (!(await store.save())) return
    configDraft.markSaved()
    const result = await tauriCallSafe('test_embedding_connection', {})
    embeddingFailed.value = !result.ok
    embeddingResult.value = result.ok ? result.data || '向量接口未返回测试结果' : result.error || '向量接口连接失败'
  } finally { embeddingTesting.value = false }
}
async function migrateLegacy() {
  try {
    const old = JSON.parse(localStorage.getItem('casy_ai_settings') || '{}')
    const id = crypto.randomUUID()
    store.config.profiles.push({ id, name: '旧版浏览器配置', mode: old.provider === 'local' ? 'ollama' : 'openai',
      apiUrl: old.provider === 'local' ? (old.baseUrl || 'http://localhost:11434').replace(/\/v1\/?$/, '') : old.baseUrl || '',
      model: old.model || '', apiKey: old.apiKey || '', hasApiKey: false })
    store.config.activeId = id
    store.config.systemPrompt = old.systemPrompt || AI_PROMPTS.SYSTEM_DEFAULT
    selectedId.value = id
    if (await store.save()) {
      configDraft.markSaved()
      localStorage.removeItem('casy_ai_settings')
      legacyAvailable.value = false
      ElMessage.success('旧配置已迁移')
    }
  } catch { ElMessage.error('旧配置格式无效') }
}
</script>

<template>
  <section class="ai-settings" v-loading="store.loading" :inert="store.loading || undefined">
    <header><h3>AI 接口与模型</h3><el-button :icon="Plus" @click="add" :disabled="!store.loaded">添加配置</el-button></header>
    <el-alert v-if="store.error" :title="store.error" type="error" :closable="false"><el-button v-if="!store.loaded" @click="reload">重新读取配置</el-button></el-alert>
    <el-button v-if="legacyAvailable" :icon="RefreshLeft" @click="migrateLegacy">迁移旧版配置</el-button>
    <el-form label-position="top" :disabled="!store.loaded || store.loading || testing || embeddingTesting" @submit.prevent="save">
      <el-form-item label="默认 AI 配置">
        <el-select v-model="store.config.activeId" clearable @clear="store.config.activeId = null" placeholder="未启用">
          <el-option v-for="p in store.config.profiles" :key="p.id" :value="p.id" :label="p.name" />
        </el-select>
      </el-form-item>
      <div v-if="store.config.profiles.length" class="profile-tabs" role="tablist">
        <button v-for="p in store.config.profiles" :key="p.id" type="button" role="tab" :aria-selected="selectedId === p.id" @click="selectedId = p.id; testResult = ''">{{ p.name }}</button>
      </div>
      <template v-if="selected">
        <div class="fields">
          <el-form-item label="配置名称"><el-input v-model="selected.name" /></el-form-item>
          <el-form-item label="接口协议"><el-select v-model="selected.mode"><el-option label="OpenAI 兼容" value="openai" /><el-option label="Ollama" value="ollama" /></el-select></el-form-item>
          <el-form-item class="wide" label="API 基础地址"><el-input v-model="selected.apiUrl" :placeholder="selected.mode === 'ollama' ? 'http://localhost:11434' : 'https://api.example.com/v1'" /></el-form-item>
          <el-form-item label="API Key"><el-input :model-value="selected.apiKey" @update:model-value="selected.apiKey = $event || undefined" type="password" show-password autocomplete="new-password" :placeholder="selected.hasApiKey ? '已保存；留空保留' : '未设置'" /><el-button v-if="selected.hasApiKey" link type="danger" @click="selected.apiKey = ''; selected.hasApiKey = false">清除密钥</el-button></el-form-item>
          <el-form-item label="模型 ID"><el-input v-model="selected.model" /></el-form-item>
        </div>
        <div class="profile-actions"><el-button :icon="Connection" :loading="testing" @click="test">测试连接</el-button><el-button :icon="Delete" title="删除配置" aria-label="删除配置" @click="remove" /></div>
        <el-alert v-if="testResult" :title="testResult" :type="testFailed ? 'error' : 'success'" :closable="false" />
      </template>
      <el-divider />
      <el-form-item label="知识库语义检索"><el-switch v-model="embeddingEnabled" /></el-form-item>
      <div v-if="store.config.embedding" class="fields">
        <el-form-item label="向量模型来源"><el-select v-model="store.config.embedding.profileId" @change="changeEmbeddingProfile"><el-option label="内置本地 E5-base（多语言）" value="builtin-e5-base" /><el-option v-for="p in store.config.profiles" :key="p.id" :label="p.name" :value="p.id" /></el-select></el-form-item>
        <el-form-item v-if="store.config.embedding.profileId !== 'builtin-e5-base'" label="向量模型 ID"><el-input v-model="store.config.embedding.model" /></el-form-item>
        <el-form-item label="每段字数"><el-input-number v-model="store.config.embedding.chunkChars" :min="128" :max="4000" :step="128" :precision="0" /></el-form-item>
        <el-form-item><el-button :icon="Connection" :loading="embeddingTesting" @click="testEmbedding">保存并测试向量模型</el-button></el-form-item>
      </div>
      <el-alert v-if="embeddingResult" :title="embeddingResult" :type="embeddingFailed ? 'error' : 'success'" :closable="false" />
      <el-form-item label="每日调用上限"><el-input-number v-model="store.config.dailyLimit" :min="0" :max="100000" :precision="0" /><AIStatusBadge class="usage-status" /></el-form-item>
      <el-form-item label="系统提示词"><el-input v-model="store.config.systemPrompt" type="textarea" :rows="5" /></el-form-item>
      <el-button :icon="RefreshLeft" @click="store.config.systemPrompt = AI_PROMPTS.SYSTEM_DEFAULT">恢复默认提示词</el-button>
      <el-button type="primary" :icon="Check" :loading="store.loading" @click="save">保存配置</el-button>
    </el-form>
    <el-divider />
    <div class="tool-policy-entry"><div><h3>AI 工具策略</h3><p>管理各模块工具的启用状态与写入审批。</p></div><el-button @click="router.push({ path: '/settings', query: { tab: 'tools' } })">管理工具策略</el-button></div>
  </section>
</template>

<style scoped>
.tool-policy-entry{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap}.tool-policy-entry h3{font-size:14px;margin:0 0 6px}.tool-policy-entry p{font-size:12px;color:var(--c-text-secondary);margin:0}

.ai-settings { max-width: 780px; }
.usage-status { margin-left: 12px; }
header { display: flex; justify-content: space-between; align-items: center; gap: 16px; margin-bottom: 20px; }
h3 { font-size: 18px; margin: 0; }
.el-select { width: 100%; }
.fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0 20px; }
.wide { grid-column: 1 / -1; }
.profile-tabs { display: flex; gap: 16px; flex-wrap: wrap; border-bottom: 1px solid var(--c-border); margin-bottom: 20px; }
.profile-tabs button { border: 0; border-bottom: 2px solid transparent; background: transparent; color: var(--c-text); padding: 10px 0; font: inherit; cursor: pointer; overflow-wrap: anywhere; max-width: 100%; }
.profile-tabs button[aria-selected="true"] { border-bottom-color: var(--el-color-primary); color: var(--el-color-primary); }
.profile-actions { display: flex; gap: 8px; margin-bottom: 20px; }
.el-alert { margin-bottom: 16px; }
@media (max-width: 640px) { .fields { grid-template-columns: minmax(0, 1fr); } }
</style>
