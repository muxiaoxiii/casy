<template>
  <div class="doc-workshop">
    <!-- 左侧面板 -->
    <div class="draft-sidebar">
      <div class="sidebar-tabs">
        <div
          :class="['tab-item', { active: activeTab === 'drafts' }]"
          @click="activeTab = 'drafts'"
        >
          📝 草稿箱
        </div>
        <div
          :class="['tab-item', { active: activeTab === 'templates' }]"
          @click="activeTab = 'templates'"
        >
          📄 模板库
        </div>
      </div>

      <!-- 草稿列表 -->
      <template v-if="activeTab === 'drafts'">
        <div class="draft-header">
          <h3>草稿箱</h3>
          <el-button size="small" type="primary" @click="createNewDraft">
            + 新建
          </el-button>
        </div>

      <el-input
        v-model="searchText"
        placeholder="搜索草稿..."
        clearable
        size="small"
        class="draft-search"
      />

      <div class="draft-list" v-loading="loading">
        <div
          v-for="draft in filteredDrafts"
          :key="draft.id"
          :class="['draft-item', { active: currentDraftId === draft.id }]"
          @click="selectDraft(draft.id)"
        >
          <div class="draft-title">{{ draft.title || '无标题' }}</div>
          <div class="draft-meta">
            <span class="draft-status" :class="draft.status">
              {{ statusLabel(draft.status) }}
            </span>
            <span class="draft-time">{{ formatTime(draft.updatedAt) }}</span>
          </div>
          <el-button
            class="draft-delete"
            size="small"
            type="danger"
            text
            @click.stop="deleteDraft(draft.id)"
          >
            删除
          </el-button>
        </div>

        <el-empty
          v-if="!loading && filteredDrafts.length === 0"
          description="暂无草稿"
          :image-size="60"
        />
      </div>
      </template>

      <!-- 模板浏览器 -->
      <template v-else-if="activeTab === 'templates'">
        <TemplateBrowser @select="onTemplateSelect" />
      </template>
    </div>

    <!-- 右侧编辑器 -->
    <div class="editor-panel">
      <template v-if="currentDraft">
        <div class="editor-header">
          <el-input
            v-model="currentDraft.title"
            placeholder="草稿标题"
            class="title-input"
            @input="scheduleSave"
          />
          <div class="editor-actions">
            <el-select
              v-model="currentDraft.status"
              size="small"
              @change="scheduleSave"
              style="width: 100px"
            >
              <el-option label="草稿" value="draft" />
              <el-option label="定稿" value="final" />
              <el-option label="归档" value="archived" />
            </el-select>
            <el-select
              v-model="currentDraft.caseId"
              filterable
              clearable
              size="small"
              placeholder="关联案件"
              @change="scheduleSave"
              style="width: 200px"
            >
              <el-option
                v-for="c in cases"
                :key="c.id"
                :label="c.caseName || c.caseNo || c.id"
                :value="c.id"
              />
            </el-select>
          </div>
        </div>

        <LegalEditor
          v-model="currentDraft.content"
          :case-data="linkedCaseData"
          :all-cases="cases"
          @update:model-value="scheduleSave"
        />

        <div class="editor-statusbar">
          <span>字数: {{ wordCount }}</span>
          <span :class="['save-status', saveStatus]">{{ saveStatusText }}</span>
          <span v-if="currentDraft.updatedAt">
            最后保存: {{ formatTime(currentDraft.updatedAt) }}
          </span>
        </div>
      </template>

      <div v-else class="no-draft">
        <el-empty description="选择或新建一份草稿" :image-size="80">
          <el-button type="primary" @click="createNewDraft">新建草稿</el-button>
        </el-empty>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import LegalEditor from '../components/LegalEditor.vue'
import TemplateBrowser from './TemplateBrowser.vue'

const drafts = ref([])
const cases = ref([])
const currentDraftId = ref(null)
const currentDraft = ref(null)
const loading = ref(false)
const searchText = ref('')
const saveStatus = ref('idle') // idle | saving | saved | error
const saveTimer = ref(null)
const activeTab = ref('drafts') // drafts | templates

// 过滤草稿列表
const filteredDrafts = computed(() => {
  const keyword = searchText.value.toLowerCase()
  if (!keyword) return drafts.value
  return drafts.value.filter(d =>
    (d.title || '').toLowerCase().includes(keyword)
  )
})

// 关联案件数据
const linkedCaseData = computed(() => {
  if (!currentDraft.value?.caseId) return {}
  return cases.value.find(c => c.id === currentDraft.value.caseId) || {}
})

// 字数统计
const wordCount = computed(() => {
  if (!currentDraft.value?.content) return 0
  // 去除 HTML 标签后计算字符数
  const text = currentDraft.value.content.replace(/<[^>]*>/g, '').trim()
  return text.length
})

const saveStatusText = computed(() => {
  switch (saveStatus.value) {
    case 'saving': return '保存中...'
    case 'saved': return '已保存'
    case 'error': return '保存失败'
    default: return ''
  }
})

// 加载草稿列表
async function loadDrafts() {
  loading.value = true
  const result = await casyContext.docs.listDrafts()
  if (result.ok) {
    drafts.value = result.data || []
  }
  loading.value = false
}

// 加载案件列表（用于关联选择）
async function loadCases() {
  const result = await casyContext.cases.list({ page: 1, perPage: 500 })
  if (result.ok) {
    cases.value = result.data?.items || []
  }
}

// 选择草稿
async function selectDraft(id) {
  // 先保存当前草稿
  if (currentDraft.value && saveStatus.value === 'saving') {
    await saveDraft()
  }

  currentDraftId.value = id
  const result = await casyContext.docs.getDraft(id)
  if (result.ok) {
    currentDraft.value = result.data
    saveStatus.value = 'idle'
  }
}

// 新建草稿
async function createNewDraft() {
  const result = await casyContext.docs.createDraft({
    title: '未命名草稿',
    content: '',
  })
  if (result.ok) {
    await loadDrafts()
    selectDraft(result.data.id)
  }
}

// 保存草稿（防抖调用）
async function saveDraft() {
  if (!currentDraft.value) return

  saveStatus.value = 'saving'
  const result = await casyContext.docs.updateDraft(currentDraft.value.id, {
    title: currentDraft.value.title,
    content: currentDraft.value.content,
    status: currentDraft.value.status,
    caseId: currentDraft.value.caseId || null,
  })

  if (result.ok) {
    saveStatus.value = 'saved'
    // 更新列表中的草稿信息
    const idx = drafts.value.findIndex(d => d.id === currentDraft.value.id)
    if (idx >= 0) {
      drafts.value[idx] = { ...drafts.value[idx], ...currentDraft.value }
    }
    // 2 秒后重置状态
    setTimeout(() => {
      if (saveStatus.value === 'saved') saveStatus.value = 'idle'
    }, 2000)
  } else {
    saveStatus.value = 'error'
  }
}

// 2 秒防抖自动保存
function scheduleSave() {
  if (saveTimer.value) clearTimeout(saveTimer.value)
  saveTimer.value = setTimeout(() => {
    saveDraft()
  }, 2000)
}

// 删除草稿
async function deleteDraft(id) {
  const result = await casyContext.docs.deleteDraft(id)
  if (result.ok) {
    if (currentDraftId.value === id) {
      currentDraftId.value = null
      currentDraft.value = null
    }
    await loadDrafts()
  }
}

// 格式化时间
function formatTime(timeStr) {
  if (!timeStr) return ''
  const d = new Date(timeStr)
  if (isNaN(d.getTime())) return timeStr
  const now = new Date()
  const diff = now - d
  if (diff < 60000) return '刚刚'
  if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`
  return d.toLocaleDateString('zh-CN')
}

function statusLabel(status) {
  switch (status) {
    case 'draft': return '草稿'
    case 'final': return '定稿'
    case 'archived': return '归档'
    default: return status
  }
}

onMounted(async () => {
  await Promise.all([loadDrafts(), loadCases()])
})

// 模板选择回调：创建新草稿并切换到编辑模式
async function onTemplateSelect(template) {
  // 创建新草稿，关联模板
  const result = await casyContext.docs.createDraft({
    title: template.name,
    content: '',
    templatePath: template.path,
  })
  if (result.ok) {
    await loadDrafts()
    selectDraft(result.data.id)
    activeTab.value = 'drafts'
  }
}

onUnmounted(() => {
  if (saveTimer.value) clearTimeout(saveTimer.value)
  // 离开前保存
  if (currentDraft.value) saveDraft()
})
</script>

<style scoped>
.doc-workshop {
  display: flex;
  height: calc(100vh - 100px);
  gap: 0;
}

.draft-sidebar {
  width: 300px;
  min-width: 300px;
  border-right: 1px solid #e0e0e0;
  display: flex;
  flex-direction: column;
  background: #fafafa;
}

.sidebar-tabs {
  display: flex;
  border-bottom: 1px solid #e0e0e0;
}

.tab-item {
  flex: 1;
  padding: 10px 16px;
  text-align: center;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s;
  border-bottom: 2px solid transparent;
}

.tab-item:hover {
  background: #ecf5ff;
}

.tab-item.active {
  color: #409eff;
  border-bottom-color: #409eff;
  background: #fff;
}

.draft-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid #e0e0e0;
}

.draft-header h3 {
  margin: 0;
  font-size: 15px;
}

.draft-search {
  padding: 8px 12px;
}

.draft-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.draft-item {
  position: relative;
  padding: 10px 16px;
  cursor: pointer;
  transition: background 0.15s;
  border-left: 3px solid transparent;
}

.draft-item:hover {
  background: #ecf5ff;
}

.draft-item.active {
  background: #ecf5ff;
  border-left-color: #409eff;
}

.draft-title {
  font-size: 14px;
  color: #333;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding-right: 24px;
}

.draft-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  font-size: 12px;
  color: #999;
}

.draft-status {
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
}

.draft-status.draft { background: #fdf6ec; color: #e6a23c; }
.draft-status.final { background: #f0f9eb; color: #67c23a; }
.draft-status.archived { background: #f4f4f5; color: #909399; }

.draft-delete {
  position: absolute;
  top: 8px;
  right: 8px;
  opacity: 0;
  transition: opacity 0.15s;
}

.draft-item:hover .draft-delete {
  opacity: 1;
}

.editor-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  border-bottom: 1px solid #e0e0e0;
}

.title-input {
  flex: 1;
}

.title-input :deep(.el-input__inner) {
  font-size: 16px;
  font-weight: 500;
  border: none;
  padding: 0;
}

.editor-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.editor-panel :deep(.legal-editor) {
  flex: 1;
  border: none;
  border-radius: 0;
}

.editor-statusbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 6px 16px;
  border-top: 1px solid #e0e0e0;
  font-size: 12px;
  color: #999;
  background: #fafafa;
}

.save-status.saving { color: #e6a23c; }
.save-status.saved { color: #67c23a; }
.save-status.error { color: #f56c6c; }

.no-draft {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
