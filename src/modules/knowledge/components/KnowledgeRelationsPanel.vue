<script setup>
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Delete, Link, Plus } from '../../../shared/icons'
import { tauriCallSafe } from '../../../core/tauriBridge'
import BacklinksPanel from './BacklinksPanel.vue'

const props = defineProps({ note: { type: Object, default: null }, notes: { type: Array, default: () => [] } })
const emit = defineEmits(['navigate'])
const outgoing = ref([])
const loading = ref(false)
const targetId = ref('')
const backlinksRef = ref(null)

const candidates = computed(() => props.notes.filter(item => item.id !== props.note?.id && item.blockType !== 'block'))
const titleMap = computed(() => Object.fromEntries(props.notes.map(item => [item.id, item.title || '无标题笔记'])))

async function load() {
  if (!props.note?.id) { outgoing.value = []; return }
  loading.value = true
  const result = await tauriCallSafe('list_links_for', { sourceType: 'knowledge', sourceId: props.note.id }, { silent: true })
  outgoing.value = result.ok && Array.isArray(result.data) ? result.data.filter(item => item.targetType === 'knowledge') : []
  loading.value = false
}

async function addRelation() {
  if (!targetId.value || !props.note?.id) return
  if (outgoing.value.some(item => item.targetId === targetId.value)) return ElMessage.info('已经关联该笔记')
  const result = await tauriCallSafe('create_link', {
    sourceType: 'knowledge', sourceId: props.note.id, targetType: 'knowledge', targetId: targetId.value,
    anchor: 'manual', label: '相关笔记',
  })
  if (!result.ok) return ElMessage.error(result.error || '建立关联失败')
  targetId.value = ''
  await load()
  backlinksRef.value?.reload?.()
  ElMessage.success('已建立双向可追踪关联')
}

async function remove(link) {
  const result = await tauriCallSafe('remove_link', { id: link.id })
  if (!result.ok) return ElMessage.error(result.error || '解除关联失败')
  await load()
}

watch(() => props.note?.id, load, { immediate: true })
defineExpose({ reload: async () => { await load(); backlinksRef.value?.reload?.() } })
</script>

<template>
  <div class="relations-panel">
    <div class="section-title"><Link /> 指向的笔记 <span>{{ outgoing.length }}</span></div>
    <div class="relation-create">
      <el-select v-model="targetId" filterable placeholder="选择要关联的笔记" size="small">
        <el-option v-for="item in candidates" :key="item.id" :label="item.title || '无标题笔记'" :value="item.id" />
      </el-select>
      <el-button :icon="Plus" size="small" :disabled="!targetId" @click="addRelation" />
    </div>
    <div v-loading="loading" class="outgoing-list">
      <button v-for="link in outgoing" :key="link.id" class="relation-item" @click="emit('navigate', link.targetId)">
        <span class="relation-kind">{{ link.anchor?.startsWith('wiki:') ? '[[]]' : link.anchor === 'pageindex:structure' ? '结构' : '关联' }}</span>
        <span class="relation-name">{{ link.targetTitle || titleMap[link.targetId] || link.label }}</span>
        <el-icon @click.stop="remove(link)"><Delete /></el-icon>
      </button>
      <div v-if="!loading && !outgoing.length" class="empty">在正文输入 <code>[[笔记标题]]</code>，或手动添加关联。</div>
    </div>
    <BacklinksPanel ref="backlinksRef" target-type="knowledge" :target-id="note?.id" />
  </div>
</template>

<style scoped>
.relations-panel{padding:14px}.section-title{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:700;color:var(--c-text-heading);margin-bottom:10px}.section-title svg{width:14px}.section-title span{margin-left:auto;color:var(--c-text-secondary)}.relation-create{display:grid;grid-template-columns:1fr auto;gap:6px}.outgoing-list{min-height:50px;margin-top:10px}.relation-item{width:100%;display:flex;align-items:center;gap:7px;padding:8px;border:0;border-bottom:1px solid var(--c-border);background:transparent;color:inherit;text-align:left;cursor:pointer}.relation-item:hover{background:var(--c-bg-subtle)}.relation-kind{font-size:9px;color:var(--c-primary);border:1px solid color-mix(in srgb,var(--c-primary) 30%,transparent);padding:1px 4px;border-radius:4px}.relation-name{flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12px}.relation-item svg{width:13px;color:var(--c-text-secondary)}.empty{font-size:11px;color:var(--c-text-secondary);line-height:1.5;padding:10px 2px}.empty code{color:var(--c-primary)}
</style>
