<template>
  <div class="backlinks-panel">
    <h4 class="panel-title">
      <el-icon :size="14"><Link /></el-icon>
      被引用于
      <span v-if="backlinks.length" class="panel-count">{{ backlinks.length }}</span>
    </h4>

    <div v-loading="loading" class="panel-body">
      <template v-if="backlinks.length">
        <div
          v-for="link in backlinks"
          :key="link.id"
          class="backlink-item"
          :class="{ clickable: isNavigable(link) }"
          @click="goToSource(link)"
        >
          <el-icon class="bl-icon" :size="14">
            <component :is="sourceIcon(link.sourceType)" />
          </el-icon>
          <div class="bl-main">
            <div class="bl-name-row">
              <span class="bl-name">{{ sourceName(link) }}</span>
              <el-tag size="small" effect="plain" class="bl-type-tag">
                {{ sourceTypeLabel(link.sourceType) }}
              </el-tag>
            </div>
            <div class="bl-sub">
              <span v-if="link.label" class="bl-label">{{ link.label }}</span>
              <span v-if="anchorText(link)" class="bl-anchor">{{ anchorText(link) }}</span>
              <span class="bl-time">{{ formatTime(link.createdAt) }}</span>
            </div>
          </div>
          <el-button
            class="bl-delete"
            type="danger"
            text
            size="small"
            :icon="Delete"
            @click.stop="confirmRemove(link)"
          />
        </div>
      </template>
      <div v-else-if="!loading" class="panel-empty">暂无其他模块引用此条目</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Link, Delete, Document, Collection, Checked, Briefcase, Folder } from '@element-plus/icons-vue'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'

// ── 本地 DTO（双链后端，commandMap 未收录，动态泛型调用） ──
interface LinkDto {
  id: string
  sourceType: string
  sourceId: string
  targetType: string
  targetId: string
  anchor: string | null
  label: string | null
  createdAt: string | null
  targetTitle: string | null
}

interface DraftLite {
  id: string
  title?: string | null
}

interface KnowledgeLite {
  id: string
  title?: string | null
}

interface KnowledgeListResultLite {
  items?: KnowledgeLite[]
}

interface TaskLite {
  id: string
  taskName?: string | null
}

interface CaseLite {
  id: string
  caseName?: string | null
  caseNo?: string | null
}

interface CaseListResultLite {
  items?: CaseLite[]
}

const props = defineProps<{
  targetType: string
  targetId: string | null | undefined
}>()

const router = useRouter()
const backlinks = ref<LinkDto[]>([])
const loading = ref(false)
// 来源名缓存：'<type>:<id>' → 展示名
const nameMap = ref<Record<string, string>>({})

const SOURCE_TYPE_LABELS: Record<string, string> = {
  doc: '文书',
  knowledge: '知识',
  task: '任务',
  case: '案件',
  file: '文件',
}

const SOURCE_ICONS: Record<string, unknown> = {
  doc: Document,
  knowledge: Collection,
  task: Checked,
  case: Briefcase,
  file: Folder,
}

function sourceTypeLabel(t: string) {
  return SOURCE_TYPE_LABELS[t] || t
}

function sourceIcon(t: string) {
  return SOURCE_ICONS[t] || Link
}

function sourceName(link: LinkDto): string {
  return nameMap.value[`${link.sourceType}:${link.sourceId}`]
    || link.label
    || `${sourceTypeLabel(link.sourceType)} ${link.sourceId.slice(0, 8)}`
}

function anchorText(link: LinkDto): string {
  if (!link.anchor) return ''
  const m = /^page:(\d+)$/.exec(link.anchor)
  return m ? `第 ${m[1]} 页` : link.anchor
}

function formatTime(t: string | null): string {
  if (!t) return ''
  return t.replace('T', ' ').substring(0, 16)
}

/** 按来源类型批量解析来源展示名（links 表只存 ID，名称尽力解析） */
async function resolveSourceNames(links: LinkDto[]) {
  const types = [...new Set(links.map(l => l.sourceType))]
  const map: Record<string, string> = { ...nameMap.value }

  await Promise.all(types.map(async t => {
    if (t === 'doc') {
      const drafts = await tauriCall('list_drafts', {}, { silent: true })
      for (const d of drafts || []) map[`doc:${d.id}`] = d.title || '未命名文书'
    } else if (t === 'knowledge') {
      const res = await tauriCall('list_knowledge', { filter: null }, { silent: true })
      for (const k of res || []) map[`knowledge:${k.id}`] = k.title || '未命名知识'
    } else if (t === 'task') {
      const tasks = await tauriCall('list_tasks', { filter: {} }, { silent: true })
      for (const task of tasks || []) map[`task:${task.id}`] = task.taskName || '未命名任务'
    } else if (t === 'case') {
      const res = await tauriCall('list_cases', { filter: {} }, { silent: true })
      const items = Array.isArray(res) ? res : (res?.items || [])
      for (const c of items) map[`case:${c.id}`] = c.caseName || c.caseNo || c.id
    }
    // file 来源：list_case_files 需要 caseId，反链侧无法反查，走 label 兜底
  }))

  nameMap.value = map
}

let loadSeq = 0

async function loadBacklinks() {
  if (!props.targetId) {
    backlinks.value = []
    return
  }
  // 竞态防护：快速切换 targetId 时旧响应不得覆盖新数据
  const reqId = ++loadSeq
  loading.value = true
  const data = await tauriCall('get_backlinks', {
    targetType: props.targetType,
    targetId: props.targetId,
  }, { silent: true })
  if (reqId !== loadSeq) return
  backlinks.value = data || []
  loading.value = false
  if (backlinks.value.length) resolveSourceNames(backlinks.value)
}

function isNavigable(link: LinkDto): boolean {
  // file 来源缺少 caseId 无法定位案卷库，其余类型均可跳转
  return link.sourceType !== 'file'
}

function goToSource(link: LinkDto) {
  switch (link.sourceType) {
    case 'doc':
      // 文书工坊无按草稿定位的路由参数，落地到工坊首页
      router.push({ name: 'docs' })
      break
    case 'knowledge':
      router.push({ name: 'knowledge', query: { select: link.sourceId } })
      break
    case 'task':
      router.push({ name: 'tasks', query: { edit: link.sourceId } })
      break
    case 'case':
      router.push({ name: 'case-detail', params: { id: link.sourceId } })
      break
    default:
      break
  }
}

async function confirmRemove(link: LinkDto) {
  try {
    await ElMessageBox.confirm(
      `确定删除来自「${sourceName(link)}」的这条引用吗？仅解除链接关系，不影响双方实体。`,
      '删除反链',
      { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' }
    )
  } catch {
    return // 用户取消
  }
  const result = await tauriCallSafe('remove_link', { id: link.id })
  if (result.ok) {
    backlinks.value = backlinks.value.filter(l => l.id !== link.id)
    ElMessage.success('反链已删除')
  } else {
    ElMessage.error(result.error || '删除反链失败')
  }
}

watch(() => [props.targetType, props.targetId], loadBacklinks, { immediate: true })

defineExpose({ reload: loadBacklinks })
</script>

<style scoped>
.backlinks-panel {
  margin-top: 16px;
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.panel-count {
  min-width: 18px;
  height: 16px;
  padding: 0 5px;
  border-radius: 8px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-size: 10.5px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.panel-body {
  min-height: 40px;
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-subtle);
  overflow: hidden;
}

.backlink-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--c-border-light);
  transition: background var(--motion-fast) var(--ease-out);
}

.backlink-item:last-child {
  border-bottom: none;
}

.backlink-item.clickable {
  cursor: pointer;
}

.backlink-item.clickable:hover {
  background: var(--c-bg-hover);
}

.bl-icon {
  flex-shrink: 0;
  color: var(--c-text-secondary);
}

.bl-main {
  flex: 1;
  min-width: 0;
}

.bl-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bl-name {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.backlink-item.clickable:hover .bl-name {
  color: var(--c-primary);
}

.bl-type-tag {
  flex-shrink: 0;
}

.bl-sub {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 2px;
  font-size: 11px;
  color: var(--c-text-secondary);
}

.bl-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 160px;
}

.bl-anchor {
  flex-shrink: 0;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}

.bl-time {
  flex-shrink: 0;
  color: var(--c-text-placeholder);
}

.bl-delete {
  flex-shrink: 0;
  opacity: 0;
  transition: opacity var(--motion-fast);
}

.backlink-item:hover .bl-delete {
  opacity: 1;
}

.panel-empty {
  padding: 18px 0;
  text-align: center;
  font-size: 12px;
  color: var(--c-text-placeholder);
}
</style>
