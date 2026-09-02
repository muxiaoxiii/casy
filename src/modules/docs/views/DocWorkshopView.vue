<template>
  <div class="doc-workshop">
    <!-- 左侧面板 -->
    <div class="draft-sidebar">
      <div class="sidebar-tabs">
        <div
          :class="['tab-item', { active: activeTab === 'drafts' }]"
          @click="activeTab = 'drafts'"
        >
          {{ $t('docs.drafts') }}
        </div>
        <div
          :class="['tab-item', { active: activeTab === 'templates' }]"
          @click="activeTab = 'templates'"
        >
          {{ $t('docs.templates') }}
        </div>
      </div>

      <!-- 草稿列表 -->
      <template v-if="activeTab === 'drafts'">
        <div class="draft-header">
          <h3>{{ $t('docs.drafts') }}</h3>
          <button class="btn-new-draft" @click="createNewDraft">
            + {{ $t('common.create') }}
          </button>
        </div>

        <el-input
          v-model="searchText"
          :placeholder="$t('common.search')"
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
            <div class="draft-title">{{ draft.title || '未命名文书' }}</div>
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

    <!-- 右侧编辑器面板 -->
    <div class="editor-panel">
      <template v-if="currentDraft">
        <div class="editor-header">
          <input
            v-model="currentDraft.title"
            :placeholder="$t('docs.draft_title')"
            class="notion-title-input"
            @input="scheduleSave"
          />
          <div class="editor-actions">
            <el-select
              v-model="currentDraft.status"
              size="small"
              @change="scheduleSave"
              style="width: 100px"
            >
              <el-option :label="$t('docs.status_draft')" value="draft" />
              <el-option :label="$t('docs.status_final')" value="final" />
              <el-option :label="$t('docs.status_archived')" value="archived" />
            </el-select>
            <el-select
              v-model="currentDraft.caseId"
              filterable
              clearable
              size="small"
              placeholder="关联案件..."
              @change="scheduleSave"
              style="width: 220px"
            >
              <el-option
                v-for="c in cases"
                :key="c.id"
                :label="c.caseName || c.caseNo || c.id"
                :value="c.id"
              />
            </el-select>
            <el-button type="primary" size="small" :loading="exporting" @click="exportToDocx">
              {{ $t('docs.export_word') }}
            </el-button>
            <el-button plain size="small" @click="openEvidenceLinkPicker">
              <el-icon><Link /></el-icon> 证据链接
            </el-button>
            <el-button plain size="small" @click="showKnowledgeSidebar = !showKnowledgeSidebar">
              <el-icon><Collection /></el-icon> 知识抽屉
            </el-button>
          </div>
        </div>

        <!-- 块编辑器主体 -->
        <LegalEditor
          ref="legalEditorRef"
          v-model="currentDraft.content"
          :case-data="linkedCaseData"
          :all-cases="cases"
          :case-id="currentDraft.caseId"
          :source-id="currentDraft.id"
          @update:model-value="scheduleSave"
          @open-knowledge-drawer="showKnowledgeSidebar = true"
        />

        <div class="editor-statusbar">
          <span>{{ $t('docs.word_count', { count: wordCount }) }}</span>
          <span :class="['save-status', saveStatus]">{{ saveStatusText }}</span>
          <span v-if="currentDraft.updatedAt">
            {{ $t('docs.last_saved') }} {{ formatTime(currentDraft.updatedAt) }}
          </span>
        </div>
      </template>

      <div v-else class="no-draft">
        <el-empty :description="$t('docs.new_draft')" :image-size="80">
          <el-button type="primary" @click="createNewDraft">{{ $t('docs.new_draft') }}</el-button>
        </el-empty>
      </div>
    </div>

    <!-- 右侧悬浮知识侧边栏 -->
    <div class="knowledge-drawer-container" v-show="showKnowledgeSidebar">
      <KnowledgeSidebar @close="showKnowledgeSidebar = false" />
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { Collection, Link } from '@element-plus/icons-vue'
import LegalEditor from '../components/LegalEditor.vue'
import KnowledgeSidebar from '../../knowledge/components/KnowledgeSidebar.vue'
import TemplateBrowser from './TemplateBrowser.vue'
import { saveAs } from 'file-saver'
import { ElMessage } from 'element-plus'
import debounce from 'lodash-es/debounce'

const showKnowledgeSidebar = ref(false)

// 证据链接（W4 双链）：转发到 LegalEditor 暴露的选择器
const legalEditorRef = ref(null)
function openEvidenceLinkPicker() {
  legalEditorRef.value?.openEvidenceLinkPicker()
}

const props = defineProps([])
const drafts = ref([])
const cases = ref([])
const currentDraftId = ref(null)
const currentDraft = ref(null)
const loading = ref(false)
const searchText = ref('')
const saveStatus = ref('idle') // idle | saving | saved | error
const saveTimer = ref(null)
const activeTab = ref('drafts') // drafts | templates
const exporting = ref(false)

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
  const text = currentDraft.value.content.replace(/<[^>]*>/g, '').trim()
  return text.length
})

const saveStatusText = computed(() => {
  switch (saveStatus.value) {
    case 'saving': return '● 正在保存...'
    case 'saved': return '✓ 已保存至本地数据库'
    case 'error': return '✕ 保存失败'
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

// 加载案件列表
async function loadCases() {
  const result = await casyContext.cases.list({ page: 1, perPage: 500 })
  if (result.ok) {
    cases.value = Array.isArray(result.data) ? result.data : (result.data?.items || [])
  }
}

// 选择草稿
async function selectDraft(id) {
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

// 新建草稿（开箱即写）
async function createNewDraft() {
  const result = await casyContext.docs.createDraft({
    title: '未命名法律文书',
    content: '<p>在此输入 <code>/</code> 唤出 Notion 块菜单，或直接输入 Markdown 快速起草...</p>',
  })
  if (result.ok && result.data) {
    await loadDrafts()
    selectDraft(result.data.id)
  }
}

// 保存草稿
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
    const idx = drafts.value.findIndex(d => d.id === currentDraft.value.id)
    if (idx >= 0) {
      drafts.value[idx] = { ...drafts.value[idx], ...currentDraft.value }
    }
    setTimeout(() => {
      if (saveStatus.value === 'saved') saveStatus.value = 'idle'
    }, 2500)
  } else {
    saveStatus.value = 'error'
  }
}

function scheduleSave() {
  if (saveTimer.value) clearTimeout(saveTimer.value)
  saveTimer.value = setTimeout(() => {
    saveDraft()
  }, 1500)
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
    if (drafts.value.length > 0) {
      selectDraft(drafts.value[0].id)
    } else {
      await createNewDraft()
    }
  }
}

// 导出为 Docx
async function exportToDocx() {
  if (!currentDraft.value) return
  exporting.value = true
  try {
    const htmlToDocx = (await import('html-to-docx')).default
    let processedHtml = currentDraft.value.content

    // EvidenceLink 脚注映射后处理
    if (processedHtml.includes('data-type="evidence-link"')) {
      const parser = new DOMParser()
      const doc = parser.parseFromString(processedHtml, 'text/html')
      const links = doc.querySelectorAll('span[data-type="evidence-link"]')
      const references = []
      
      links.forEach((link, index) => {
        const num = index + 1
        const label = link.getAttribute('data-label') || '未知引用'
        const anchor = link.getAttribute('data-anchor') || ''
        
        references.push(`<li>${label} ${anchor ? '(' + anchor + ')' : ''}</li>`)
        
        const sup = doc.createElement('sup')
        sup.textContent = `[${num}]`
        link.parentNode?.replaceChild(sup, link)
      })
      
      if (references.length > 0) {
        const refSection = doc.createElement('div')
        refSection.innerHTML = `<hr/><h3>参考证据列表</h3><ol>${references.join('')}</ol>`
        doc.body.appendChild(refSection)
      }
      processedHtml = doc.body.innerHTML
    }

    const htmlString = `<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body>${processedHtml}</body></html>`
    const fileBuffer = await htmlToDocx(htmlString, null, {
      table: { row: { cantSplit: true } },
      footer: true,
      pageNumber: true,
    })
    const blob = new Blob([fileBuffer], { type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' })
    saveAs(blob, `${currentDraft.value.title || '未命名文书'}.docx`)
    ElMessage.success('Word 文档导出成功')
  } catch (err) {
    console.error('导出 Word 失败', err)
    ElMessage.error('导出失败')
  } finally {
    exporting.value = false
  }
}

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
    case 'archived': return '已归档'
    default: return status
  }
}

onMounted(async () => {
  await Promise.all([loadDrafts(), loadCases()])
  if (drafts.value.length > 0) {
    selectDraft(drafts.value[0].id)
  } else {
    await createNewDraft()
  }
})

// 模板选择回调
async function onTemplateSelect(template) {
  const result = await casyContext.docs.createDraft({
    title: template.name,
    content: `<p>基于模板 <strong>${template.name}</strong> 创建</p>`,
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
  if (currentDraft.value) saveDraft()
})
</script>

<style scoped>
.doc-workshop {
  display: flex;
  height: calc(100vh - 64px);
  background: var(--c-bg-page);
  overflow: hidden;
}

.draft-sidebar {
  width: 260px;
  background: var(--c-bg-sidebar);
  border-right: 1px solid var(--c-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-tabs {
  display: flex;
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
}

.tab-item {
  flex: 1;
  text-align: center;
  padding: 10px 0;
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text-secondary);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all var(--motion-fast);
}

.tab-item.active {
  color: var(--c-primary);
  font-weight: 600;
  border-bottom-color: var(--c-primary);
  background: var(--c-bg-sidebar);
}

.draft-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px 8px;
}

.draft-header h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.btn-new-draft {
  padding: 4px 10px;
  border-radius: var(--c-radius-md);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-primary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-new-draft:hover {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
}

.draft-search {
  padding: 0 14px 8px;
}

.draft-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.draft-item {
  position: relative;
  padding: 10px 12px;
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  transition: all var(--motion-fast);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.draft-item:hover {
  background: var(--c-bg-hover);
}

.draft-item.active {
  background: var(--c-bg-selected);
}

.draft-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-heading);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding-right: 28px;
}

.draft-item.active .draft-title {
  color: var(--c-primary);
}

.draft-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
}

.draft-status {
  padding: 1px 5px;
  border-radius: 3px;
  font-weight: 500;
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}

.draft-status.final {
  background: var(--bg-success-weak);
  color: var(--status-success);
}

.draft-time {
  color: var(--slate-gray-light);
}

.draft-delete {
  position: absolute;
  right: 8px;
  top: 8px;
  opacity: 0;
  transition: opacity var(--motion-fast);
}

.draft-item:hover .draft-delete {
  opacity: 1;
}

.editor-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--c-bg-card);
  min-width: 0;
}

.editor-header {
  padding: 16px 36px 12px;
  border-bottom: 1px solid var(--c-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  background: var(--c-bg-card);
}

.notion-title-input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 20px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.3px;
}

.notion-title-input::placeholder {
  color: var(--slate-gray-light);
  opacity: 0.6;
}

.editor-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.editor-statusbar {
  padding: 8px 36px;
  border-top: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11.5px;
  color: var(--slate-gray-light);
}

.editor-statusbar span {
  margin-left: 16px;
}

.save-status.saved {
  color: var(--el-color-success);
}

.save-status.saving {
  color: var(--el-color-primary);
}

.save-status.error {
  color: var(--status-risk);
}

.knowledge-drawer-container {
  height: 100vh;
  position: relative;
  flex-shrink: 0;
  transition: all 0.3s ease;
  z-index: 10;
}

.no-draft {
  flex: 1;
  display: grid;
  place-items: center;
}
</style>
