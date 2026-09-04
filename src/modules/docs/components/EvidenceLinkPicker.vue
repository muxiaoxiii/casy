<template>
  <el-dialog
    v-model="visible"
    title="插入证据链接"
    width="560px"
    append-to-body
    destroy-on-close
    class="evidence-link-picker"
  >
    <!-- 第一步：目标类型 -->
    <div class="picker-section">
      <div class="picker-label">目标类型</div>
      <div class="type-cards">
        <div
          v-for="t in targetTypes"
          :key="t.value"
          :class="['type-card', { active: targetType === t.value }]"
          @click="switchType(t.value)"
        >
          <el-icon :size="16"><component :is="t.icon" /></el-icon>
          <span>{{ t.label }}</span>
        </div>
      </div>
    </div>

    <!-- 第二步：选择具体实体 -->
    <div class="picker-section">
      <div class="picker-label">选择{{ currentTypeLabel }}</div>

      <!-- 卷宗文件：先选案件，再列文件 -->
      <template v-if="targetType === 'file'">
        <el-select
          v-model="fileCaseId"
          filterable
          placeholder="选择所属案件..."
          style="width: 100%; margin-bottom: 8px"
          @change="loadCaseFiles"
        >
          <el-option
            v-for="c in caseOptions"
            :key="c.id"
            :label="c.caseName || c.caseNo || '未命名案件'"
            :value="c.id"
          />
        </el-select>
        <el-input
          v-model="query"
          placeholder="输入文件名过滤..."
          clearable
          :prefix-icon="Search"
        />
      </template>

      <!-- 知识/任务：后端检索；案件：本地过滤案件列表 -->
      <template v-else>
        <div class="search-row">
          <el-input
            v-model="query"
            :placeholder="`输入关键词检索${currentTypeLabel}...`"
            clearable
            :prefix-icon="Search"
            @keyup.enter="searchEntities"
          />
          <el-button
            v-if="targetType !== 'case'"
            type="primary"
            plain
            :loading="searching"
            @click="searchEntities"
          >检索</el-button>
        </div>
      </template>

      <!-- 候选列表 -->
      <div class="entity-list" v-loading="searching || filesLoading">
        <template v-if="displayEntities.length">
          <div
            v-for="e in displayEntities"
            :key="e.id"
            :class="['entity-item', { active: selected?.id === e.id }]"
            @click="selectEntity(e)"
          >
            <span class="entity-name">{{ e.name }}</span>
            <span v-if="e.meta" class="entity-meta">{{ e.meta }}</span>
          </div>
        </template>
        <div v-else class="entity-empty">{{ emptyHint }}</div>
      </div>
    </div>

    <!-- 第三步：定位与展示文本 -->
    <div class="picker-section">
      <div class="picker-row">
        <div v-if="targetType === 'file'" class="picker-field">
          <div class="picker-label">页码（可选）</div>
          <el-input-number v-model="pageNo" :min="1" :max="9999" placeholder="页码" style="width: 140px" />
        </div>
        <div class="picker-field" style="flex: 1">
          <div class="picker-label">锚文本（可选，默认取目标名）</div>
          <el-input v-model="label" placeholder="正文中显示的链接文字" maxlength="60" />
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :disabled="!selected" :loading="creating" @click="confirmInsert">
        插入链接
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Search, Folder, Collection, Checked, Briefcase } from '@element-plus/icons-vue'
import { tauriCall } from '../../../core/tauriBridge'
import type { EvidenceLinkAttrs, EvidenceTargetType } from '../extensions/EvidenceLink'

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

interface CaseFileDto {
  id: string
  caseId: string
  fileName: string
  category: string
  fileType: string | null
  fileSize: number | null
  createdAt: string | null
}

interface SearchKnowledgeDto {
  id: string
  title: string
  category: string
  tags: string | null
  lawName: string | null
}

interface SearchTaskDto {
  id: string
  taskName: string
  dueDate: string | null
  completed: number
}

interface CaseLite {
  id: string
  caseName?: string | null
  caseNo?: string | null
}

interface EntityOption {
  id: string
  name: string
  meta?: string
  caseId?: string
}

const props = defineProps<{
  modelValue: boolean
  /** 当前文书草稿 id（双链 source） */
  sourceId: string | null
  /** 当前文书关联案件 id（卷宗文件默认案件） */
  caseId: string | null
  /** 已加载案件列表（避免重复请求；案件选择走本地过滤） */
  allCases: CaseLite[]
}>()

const emit = defineEmits<{
  'update:modelValue': [val: boolean]
  insert: [attrs: EvidenceLinkAttrs]
}>()

const visible = computed({
  get: () => props.modelValue,
  set: v => emit('update:modelValue', v),
})

const targetTypes = [
  { value: 'file' as const, label: '卷宗文件', icon: Folder },
  { value: 'knowledge' as const, label: '知识条目', icon: Collection },
  { value: 'task' as const, label: '任务', icon: Checked },
  { value: 'case' as const, label: '案件', icon: Briefcase },
]

const targetType = ref<EvidenceTargetType>('file')
const query = ref('')
const searching = ref(false)
const creating = ref(false)
const selected = ref<EntityOption | null>(null)
const label = ref('')
const prevAutoLabel = ref('')
const pageNo = ref<number | undefined>(undefined)

// ── 卷宗文件流 ──
const fileCaseId = ref<string | null>(null)
const caseFiles = ref<CaseFileDto[]>([])
const filesLoading = ref(false)

// ── 检索结果（知识 / 任务） ──
const searchResults = ref<EntityOption[]>([])

const caseOptions = computed(() => props.allCases || [])

const currentTypeLabel = computed(
  () => targetTypes.find(t => t.value === targetType.value)?.label || '目标'
)

// 候选实体统一视图
const displayEntities = computed<EntityOption[]>(() => {
  const q = query.value.trim().toLowerCase()
  if (targetType.value === 'file') {
    const list = caseFiles.value.map(f => ({
      id: f.id,
      name: f.fileName,
      meta: f.category || undefined,
      caseId: f.caseId,
    }))
    return q ? list.filter(e => e.name.toLowerCase().includes(q)) : list
  }
  if (targetType.value === 'case') {
    const list = caseOptions.value.map(c => ({
      id: c.id,
      name: c.caseName || c.caseNo || '未命名案件',
      meta: c.caseNo || undefined,
    }))
    return q ? list.filter(e => e.name.toLowerCase().includes(q)) : list
  }
  return searchResults.value
})

const emptyHint = computed(() => {
  if (targetType.value === 'file') {
    return fileCaseId.value ? '该案件下暂无登记文件' : '请先选择所属案件'
  }
  if (targetType.value === 'case') {
    return caseOptions.value.length ? '无匹配案件' : '暂无案件数据'
  }
  return query.value.trim() ? '未检索到匹配结果' : '输入关键词后点击检索'
})

function switchType(t: EvidenceTargetType) {
  targetType.value = t
  selected.value = null
  query.value = ''
  searchResults.value = []
  if (t === 'file' && fileCaseId.value) loadCaseFiles()
}

async function loadCaseFiles() {
  if (!fileCaseId.value) {
    caseFiles.value = []
    return
  }
  filesLoading.value = true
  const data = await tauriCall('list_case_files', {
    caseId: fileCaseId.value,
    category: null,
  })
  caseFiles.value = data || []
  filesLoading.value = false
}

async function searchEntities() {
  const q = query.value.trim()
  if (!q) {
    ElMessage.warning('请输入检索关键词')
    return
  }
  searching.value = true
  try {
    if (targetType.value === 'knowledge') {
      const data = await tauriCall('search_knowledge', { query: q })
      searchResults.value = (data || []).map(k => ({
        id: k.id,
        name: k.title,
        meta: k.lawName || k.tags || undefined,
      }))
    } else if (targetType.value === 'task') {
      const data = await tauriCall('search_tasks', { query: q })
      searchResults.value = (data || []).map(t => ({
        id: t.id,
        name: t.taskName,
        meta: t.dueDate ? `截止 ${t.dueDate}` : undefined,
      }))
    } else if (targetType.value === 'case') {
      // allCases 为空时回退到后端 FTS 检索
      const data = await tauriCall('search_cases', { query: q })
      searchResults.value = (data || []).map(c => ({
        id: c.id,
        name: c.caseName || c.caseNo || '未命名案件',
        meta: c.caseNo || undefined,
      }))
    }
  } finally {
    searching.value = false
  }
}

function selectEntity(e: EntityOption) {
  selected.value = e
  // 换选实体时重置锚文本为新实体名（避免残留上一个实体名）；用户已手改的保留
  if (!label.value || label.value === prevAutoLabel.value) label.value = e.name
  prevAutoLabel.value = e.name
}

async function confirmInsert() {
  if (!selected.value) return
  if (!props.sourceId) {
    ElMessage.warning('当前文书尚未保存为草稿，无法建立链接')
    return
  }
  creating.value = true
  const anchor =
    targetType.value === 'file' && pageNo.value ? `page:${pageNo.value}` : null
  const finalLabel = label.value.trim() || selected.value.name

  const link = await tauriCall('create_link', {
    sourceType: 'doc',
    sourceId: props.sourceId,
    targetType: targetType.value,
    targetId: selected.value.id,
    anchor,
    label: finalLabel,
  })
  creating.value = false

  if (!link) return // tauriCall 已弹出错误提示

  emit('insert', {
    linkId: link.id,
    targetType: targetType.value,
    targetId: selected.value.id,
    anchor,
    label: finalLabel,
    caseId: targetType.value === 'file' ? fileCaseId.value : null,
  })
  ElMessage.success('证据链接已插入')
  visible.value = false
}

// 打开时：初始化默认案件 / 重置状态
watch(visible, v => {
  if (!v) return
  selected.value = null
  label.value = ''
  pageNo.value = undefined
  query.value = ''
  searchResults.value = []
  fileCaseId.value = props.caseId || fileCaseId.value
  if (targetType.value === 'file' && fileCaseId.value) loadCaseFiles()
})
</script>

<style scoped>
.picker-section {
  margin-bottom: 16px;
}

.picker-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--c-text-secondary);
  margin-bottom: 6px;
}

.type-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}

.type-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 10px 4px;
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12.5px;
  cursor: pointer;
  transition: all var(--motion-fast) var(--ease-out);
}

.type-card:hover {
  border-color: var(--c-primary);
  background: var(--c-bg-hover);
}

.type-card.active {
  border-color: var(--c-primary);
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}

.search-row {
  display: flex;
  gap: 8px;
}

.entity-list {
  margin-top: 8px;
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-subtle);
}

.entity-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--c-border-light);
  transition: background var(--motion-fast);
}

.entity-item:last-child {
  border-bottom: none;
}

.entity-item:hover {
  background: var(--c-bg-hover);
}

.entity-item.active {
  background: var(--c-bg-selected);
}

.entity-name {
  font-size: 13px;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.entity-item.active .entity-name {
  color: var(--c-primary);
  font-weight: 600;
}

.entity-meta {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--c-text-secondary);
}

.entity-empty {
  padding: 24px 0;
  text-align: center;
  font-size: 12px;
  color: var(--c-text-placeholder);
}

.picker-row {
  display: flex;
  gap: 16px;
  align-items: flex-end;
}

.picker-field {
  display: flex;
  flex-direction: column;
}
</style>
