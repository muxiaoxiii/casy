<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import type { CasyTool } from '../../../core/plugin/types'
import { useSettingsStore } from '../../../stores/settings'

type Policy = ReturnType<typeof casyContext.getToolPolicy>
const settings = useSettingsStore()
const tools = ref<CasyTool[]>([])
const policy = ref<Policy>({ disabled: [], writeApproval: {} })
const query = ref('')
const loading = ref(true)
const saving = ref(false)
const error = ref('')
const status = ref('')
let revision = 0
const categories: Record<string, string> = { cases: '案件', tasks: '任务', docs: '文书', knowledge: '知识库', calendar: '日历', files: '卷宗', inbox: '收件箱', reminder: '提醒', workspace: '工作区', projects: '项目', settings: '设置', sync: '同步', backup: '备份' }
const filtered = computed(() => tools.value.filter(t => `${t.name} ${t.description} ${categories[t.category] || t.category}`.toLowerCase().includes(query.value.trim().toLowerCase())))
const groups = computed(() => [...new Set(filtered.value.map(t => t.category))].map(category => ({ category, tools: filtered.value.filter(t => t.category === category) })))
async function load() {
  const request = ++revision
  loading.value = true
  error.value = ''
  try {
    const result = await casyContext.settings.get()
    if (request !== revision) return
    if (!result.ok) throw new Error(result.error || '工具策略加载失败')
    const raw = result.data?.ai_tool_policy as Policy | undefined
    policy.value = { disabled: Array.isArray(raw?.disabled) ? raw.disabled : [], writeApproval: raw?.writeApproval || {} }
    tools.value = casyContext.getRegisteredTools()
  } catch (e) { if (request === revision) error.value = String(e) }
  finally { if (request === revision) loading.value = false }
}
async function update(name: string, enabled?: boolean, approval?: string) {
  if (saving.value || loading.value) return
  const next: Policy = { disabled: [...policy.value.disabled], writeApproval: { ...policy.value.writeApproval } }
  if (enabled !== undefined) next.disabled = enabled ? next.disabled.filter(n => n !== name) : [...new Set([...next.disabled, name])]
  if (approval) next.writeApproval[name] = approval as Policy['writeApproval'][string]
  saving.value = true
  error.value = ''; status.value = '正在保存…'
  try {
    const result = await casyContext.settings.save({ ai_tool_policy: next })
    if (!result.ok) throw new Error(result.error || '保存失败，请重试')
    casyContext.setToolPolicy(next)
    // Keep other settings forms from writing an older policy back later.
    Object.assign(settings.$state, { ai_tool_policy: next })
    policy.value = next
    status.value = '已保存并生效'
  } catch (e) { error.value = String(e); status.value = '未保存，已保留原设置' }
  finally { saving.value = false }
}
const stop = casyContext.on('plugins:ready', () => { if (!saving.value) void load() })
onMounted(load)
onBeforeUnmount(() => { revision++; stop() })
</script>

<template>
  <section class="tool-policy" :aria-busy="loading || saving">
    <header><h2>AI 工具策略</h2><p>决定内置 AI 可以使用哪些能力。关闭后，AI 将无法发现或调用该工具。</p></header>
    <p class="policy-note">涉及重要修改的操作仍需确认。外部 MCP 的写入继续在「SMTP / MCP」中审批，此处仅管理内置 AI。</p>
    <el-input v-model="query" placeholder="搜索工具、功能或模块" aria-label="搜索 AI 工具" clearable />
    <div class="policy-status" role="status">{{ status || (loading ? '正在加载…' : `${tools.length - policy.disabled.filter(n => tools.some(t => t.name === n)).length} / ${tools.length} 个工具已启用 · 修改自动保存`) }}</div>
    <el-alert v-if="error" :title="error" type="error" :closable="false"><el-button v-if="!tools.length" text @click="load">重新加载</el-button></el-alert>
    <div v-for="group in groups" :key="group.category" class="tool-group">
      <h3>{{ categories[group.category] || group.category }}</h3>
      <div v-for="tool in group.tools" :key="tool.name" class="tool-row">
        <div class="tool-copy"><strong>{{ tool.policy?.title || tool.name }}</strong><p>{{ tool.description }}</p><small>{{ typeof tool.policy?.write !== 'boolean' ? '未声明策略，AI 不可自动调用' : tool.policy.write ? '可修改数据' : '读取与查询' }}</small></div>
        <el-select v-if="tool.policy?.write" :model-value="policy.writeApproval[tool.name] || 'always_ask'" :disabled="loading || saving || policy.disabled.includes(tool.name)" :aria-label="`${tool.policy?.title || tool.name}审批方式`" @update:model-value="update(tool.name, undefined, String($event))">
          <el-option label="每次询问" value="always_ask" />
          <el-option v-if="tool.policy.level !== 'L2' && tool.policy.level !== 'L3'" label="自动批准" value="always_approve" />
          <el-option label="禁止写入" value="always_reject" />
        </el-select>
        <el-switch :model-value="!policy.disabled.includes(tool.name)" :disabled="loading || saving" :aria-label="`启用 ${tool.policy?.title || tool.name}`" @update:model-value="update(tool.name, Boolean($event))" />
      </div>
    </div>
    <el-empty v-if="!loading && !error && !filtered.length" :description="query ? '没有匹配的工具，试试其他关键词' : '工具正在初始化，请稍后重试'"><el-button v-if="!query" @click="load">重新加载</el-button></el-empty>
  </section>
</template>
<style scoped>
.tool-policy{max-width:960px}.tool-policy h2{font-size:20px;margin:0 0 8px}.tool-policy header p,.policy-note,.policy-status{font-size:13px;line-height:1.7;color:var(--c-text-secondary)}.policy-note{padding:12px 16px;background:var(--c-bg-subtle);border-radius:8px}.policy-status{min-height:32px;padding-top:8px}.tool-group h3{font-size:14px;padding:16px 0 8px;margin:0;border-bottom:1px solid var(--c-border)}.tool-row{display:flex;align-items:center;gap:20px;padding:16px 0;border-bottom:1px solid var(--c-border-light)}.tool-copy{flex:1;min-width:0}.tool-copy strong{font-size:13px;overflow-wrap:anywhere}.tool-copy p{font-size:12px;color:var(--c-text-secondary);line-height:1.6;margin:5px 0}.tool-copy small{font-size:11px;color:var(--c-text-secondary)}.tool-row .el-select{width:140px;flex-shrink:0}@media(max-width:700px){.tool-row{flex-wrap:wrap;gap:10px}.tool-copy{flex-basis:100%}}
</style>
