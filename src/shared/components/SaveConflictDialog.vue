<script setup lang="ts">
import { ref, computed } from 'vue'
import { ElMessage } from 'element-plus'
type Content = { id?: string; title: string; content: string; [key: string]: any }
const props = defineProps<{
  local: () => Content
  latest: () => Promise<Content>
  copy: (snapshot: Content) => Promise<unknown>
  apply: (snapshot: Content) => void
  prepare?: () => void
}>()
const opened = ref(false), busy = ref(false), error = ref('')
const latest = ref<Content | null>(null)
const savedCopy = ref('')
const local = computed(() => props.local())
const protectedCopy = computed(() => savedCopy.value === JSON.stringify(local.value))
function plain(content: string) {
  const doc = new DOMParser().parseFromString(content, 'text/html')
  return doc.body.textContent || ''
}
async function refresh() {
  if (busy.value) return
  busy.value = true; error.value = ''; latest.value = null
  try { latest.value = await props.latest() }
  catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
function open() { opened.value = true; void refresh() }
async function preserve() {
  if (busy.value) return
  busy.value = true; error.value = ''
  try {
    props.prepare?.()
    const snapshot = JSON.parse(JSON.stringify(props.local()))
    await props.copy(snapshot)
    savedCopy.value = JSON.stringify(snapshot)
    ElMessage.success('本地修改已另存为副本，原文书未被覆盖')
  } catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
function apply() {
  try { props.prepare?.() } catch (cause) { error.value = String(cause); return }
  if (!latest.value || !protectedCopy.value || busy.value) return
  props.apply(latest.value)
  opened.value = false; savedCopy.value = ''
}
defineExpose({ open })
</script>
<template>
  <el-dialog v-model="opened" title="处理保存冲突" width="min(960px, 94vw)" :close-on-click-modal="false" :close-on-press-escape="!busy" :show-close="!busy">
    <p>其他页面已修改这份内容。当前输入仍保留，请先另存本地副本，再载入最新版本继续编辑。</p>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <div class="conflict-columns">
      <section><h3>本地修改 · {{ local.title }}</h3><pre>{{ plain(local.content) }}</pre></section>
      <section><h3>已保存的最新版本 · {{ latest?.title }}</h3><pre>{{ latest ? plain(latest.content) : busy ? '正在读取…' : '读取失败，请重试' }}</pre></section>
    </div>
    <template #footer>
      <el-button :disabled="busy" @click="opened = false">继续编辑本地内容</el-button>
      <el-button :disabled="busy" @click="refresh">刷新最新版本</el-button>
      <el-button :loading="busy" :disabled="protectedCopy" @click="preserve">{{ protectedCopy ? '本地副本已保存' : '另存本地副本' }}</el-button>
      <el-button type="primary" :disabled="busy || !latest || !protectedCopy" @click="apply">载入最新版本</el-button>
    </template>
  </el-dialog>
</template>
<style scoped>
.conflict-columns{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin-top:16px}.conflict-columns section{min-width:0}.conflict-columns h3{font-size:14px;overflow-wrap:anywhere}.conflict-columns pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.7;height:40vh;overflow:auto;border:1px solid var(--c-border);border-radius:8px;padding:12px;background:var(--c-bg-page)}@media(max-width:640px){.conflict-columns{grid-template-columns:1fr}.conflict-columns pre{height:22vh}}
</style>
