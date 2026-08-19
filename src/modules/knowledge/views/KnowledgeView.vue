<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage } from 'element-plus'

const route = useRoute()
const router = useRouter()

const loading = ref(false)
const knowledgeList = ref([])
const selectedItem = ref(null)
const versions = ref([])
const showVersionDialog = ref(false)
const showDiffDialog = ref(false)
const diffResult = ref(null)
const selectedVersions = ref([])

// 筛选条件
const filter = ref({
  category: '',
  search: '',
  lawName: '',
})

// 6 职能分类（设计哲学 §8.2）
const categories = [
  { value: 'inspiration', label: '灵感', color: '#6C6A9C' },
  { value: 'method', label: '方法', color: '#3E5C9A' },
  { value: 'reference', label: '参考', color: '#4C8067' },
  { value: 'question', label: '问题', color: '#B4554F' },
  { value: 'experience', label: '经验', color: '#B0823A' },
  { value: 'log', label: '日志', color: '#9BA2AF' },
]

// 职能颜色映射
const categoryColorMap = {
  inspiration: '#6C6A9C',
  method: '#3E5C9A',
  reference: '#4C8067',
  question: '#B4554F',
  experience: '#B0823A',
  log: '#9BA2AF',
}

// 职能标签映射
const categoryLabelMap = {
  inspiration: '灵感',
  method: '方法',
  reference: '参考',
  question: '问题',
  experience: '经验',
  log: '日志',
}

// 获取职能颜色
function getCategoryColor(category) {
  return categoryColorMap[category] || '#9BA2AF'
}

// 获取职能标签
function getCategoryLabel(category) {
  return categoryLabelMap[category] || category
}

// 块级引用类型（设计哲学 §8.2）
const blockTypeLabels = {
  paragraph: '段落',
  list: '列表',
  quote: '引用',
  code: '代码',
  table: '表格',
  law_article: '法条',
  case_note: '案例笔记',
  experience: '经验总结',
}

// 跳转到父级条目（get_knowledge_with_blocks 返回 { item, blocks }）
async function jumpToParent(parentId) {
  const result = await tauriCallSafe('get_knowledge_with_blocks', { id: parentId })
  if (result.ok && result.data?.item) {
    selectItem(result.data.item)
  }
}

async function loadKnowledge() {
  loading.value = true
  const result = await tauriCallSafe('list_knowledge', {
    filter: {
      category: filter.value.category || null,
      search: filter.value.search || null,
      lawName: filter.value.lawName || null,
    },
  })
  if (result.ok) {
    knowledgeList.value = result.data
  }
  loading.value = false
}

async function loadVersions(itemId) {
  const result = await tauriCallSafe('list_knowledge_versions', { itemId })
  if (result.ok) {
    versions.value = result.data
  }
}

// ============================================================
// 块级结构（get_knowledge_with_blocks / create_knowledge 带 parentId+blockType）
// ============================================================
const childBlocks = ref([])       // 当前条目的子块（扁平，含 parentId）
const blocksLoading = ref(false)
const showAddBlockDialog = ref(false)
const addingBlock = ref(false)
const addBlockForm = ref({ title: '', content: '' })

// 顶层列表默认不显示块级条目
const topLevelList = computed(() => knowledgeList.value.filter(k => k.blockType !== 'block'))

// 带缩进层级的块列表（按 parentId 链计算深度，父块先于子块返回）
const blocksWithDepth = computed(() => {
  const depthMap = {}
  return childBlocks.value.map(b => {
    const depth = b.parentId && depthMap[b.parentId] !== undefined ? depthMap[b.parentId] + 1 : 0
    depthMap[b.id] = depth
    return { ...b, depth }
  })
})

async function loadBlocks(itemId) {
  blocksLoading.value = true
  const result = await tauriCallSafe('get_knowledge_with_blocks', { id: itemId })
  if (result.ok && result.data) {
    childBlocks.value = result.data.blocks || []
  } else {
    childBlocks.value = []
  }
  blocksLoading.value = false
}

function openAddBlockDialog() {
  addBlockForm.value = { title: '', content: '' }
  showAddBlockDialog.value = true
}

async function confirmAddBlock() {
  if (!addBlockForm.value.title.trim()) {
    ElMessage.warning('请填写子块标题')
    return
  }
  addingBlock.value = true
  const result = await tauriCallSafe('create_knowledge', {
    data: {
      title: addBlockForm.value.title.trim(),
      category: selectedItem.value?.category || 'reference',
      content: addBlockForm.value.content || '',
      parentId: selectedItem.value?.id || null,
      blockType: 'block',
      status: 'current',
    },
  })
  addingBlock.value = false
  if (result.ok) {
    ElMessage.success('子块已添加')
    showAddBlockDialog.value = false
    loadBlocks(selectedItem.value.id)
  } else {
    ElMessage.error(result.error || '添加失败')
  }
}

function getBlockTypeLabel(blockType) {
  return blockTypeLabels[blockType] || blockType || '块'
}

function selectItem(item) {
  if (!item) return
  selectedItem.value = item
  childBlocks.value = []
  loadVersions(item.id)
  loadBlocks(item.id)
}

async function showVersionHistory() {
  if (!selectedItem.value) {
    ElMessage.warning('请先选择一个知识条目')
    return
  }
  await loadVersions(selectedItem.value.id)
  showVersionDialog.value = true
}

async function compareWithCurrent(versionId) {
  if (!selectedItem.value) return

  const result = await tauriCallSafe('diff_knowledge_with_current', {
    versionId,
    itemId: selectedItem.value.id,
  })
  if (result.ok) {
    diffResult.value = result.data
    showDiffDialog.value = true
  } else {
    ElMessage.error(result.error || '对比失败')
  }
}

async function compareVersions() {
  if (selectedVersions.value.length !== 2) {
    ElMessage.warning('请选择两个版本进行对比')
    return
  }

  const result = await tauriCallSafe('diff_knowledge_versions', {
    versionId1: selectedVersions.value[0],
    versionId2: selectedVersions.value[1],
  })
  if (result.ok) {
    diffResult.value = result.data
    showDiffDialog.value = true
  } else {
    ElMessage.error(result.error || '对比失败')
  }
}

function toggleVersionSelect(versionId) {
  const idx = selectedVersions.value.indexOf(versionId)
  if (idx >= 0) {
    selectedVersions.value.splice(idx, 1)
  } else {
    if (selectedVersions.value.length >= 2) {
      selectedVersions.value.shift()
    }
    selectedVersions.value.push(versionId)
  }
}

function formatDate(dateStr) {
  if (!dateStr) return '-'
  return dateStr.replace('T', ' ').substring(0, 19)
}

onMounted(async () => {
  await loadKnowledge()
  // 支持从知识图谱跳转定位：/knowledge?select=<id>
  const selectId = route.query.select
  if (selectId) {
    const target = knowledgeList.value.find(k => k.id === selectId)
    if (target) selectItem(target)
  }
})
</script>

<template>
  <div class="knowledge-page">
    <div class="knowledge-header">
      <h2>知识库</h2>
      <div class="header-actions">
        <el-button @click="router.push({ name: 'knowledge-graph' })">知识图谱</el-button>
        <el-input
          v-model="filter.search"
          placeholder="搜索知识条目..."
          clearable
          style="width: 200px"
          @keyup.enter="loadKnowledge"
        />
        <el-button type="primary" @click="loadKnowledge">搜索</el-button>
      </div>
    </div>

    <!-- 职能标签页 -->
    <div class="category-tabs">
      <div
        v-for="cat in categories"
        :key="cat.value"
        :class="['category-tab', { active: filter.category === cat.value }]"
        :style="{ '--tab-color': cat.color }"
        @click="filter.category = filter.category === cat.value ? '' : cat.value; loadKnowledge()"
      >
        <span class="tab-dot" :style="{ backgroundColor: cat.color }"></span>
        <span class="tab-label">{{ cat.label }}</span>
      </div>
    </div>

    <div class="knowledge-content">
      <!-- 左侧列表 -->
      <div class="knowledge-list">
        <el-table
          :data="topLevelList"
          v-loading="loading"
          stripe
          highlight-current-row
          @current-change="selectItem"
          style="width: 100%"
        >
          <el-table-column prop="title" label="标题" min-width="200" />
          <el-table-column prop="category" label="职能" width="100">
            <template #default="{ row }">
              <el-tag
                size="small"
                :color="getCategoryColor(row.category)"
                style="color: #fff; border: none;"
              >
                {{ getCategoryLabel(row.category) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="lawName" label="法律名称" width="150" />
          <el-table-column prop="updatedAt" label="更新时间" width="150">
            <template #default="{ row }">
              {{ formatDate(row.updatedAt) }}
            </template>
          </el-table-column>
        </el-table>
      </div>

      <!-- 右侧详情 -->
      <div class="knowledge-detail" v-if="selectedItem">
        <el-card>
          <template #header>
            <div class="detail-header">
              <h3>{{ selectedItem.title }}</h3>
              <div class="detail-actions">
                <el-button size="small" @click="openAddBlockDialog">添加子块</el-button>
                <el-button type="primary" size="small" @click="showVersionHistory">
                  📜 版本历史
                </el-button>
              </div>
            </div>
          </template>

          <el-descriptions :column="2" border size="small">
            <el-descriptions-item label="职能">
              <el-tag
                size="small"
                :color="getCategoryColor(selectedItem.category)"
                style="color: #fff; border: none;"
              >
                {{ getCategoryLabel(selectedItem.category) }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="状态">{{ selectedItem.status }}</el-descriptions-item>
            <el-descriptions-item label="法律名称">{{ selectedItem.lawName || '-' }}</el-descriptions-item>
            <el-descriptions-item label="条款号">{{ selectedItem.articleNo || '-' }}</el-descriptions-item>
            <el-descriptions-item label="生效日期">{{ selectedItem.effectiveDate || '-' }}</el-descriptions-item>
            <el-descriptions-item label="关联案件">{{ selectedItem.linkedCaseId || '-' }}</el-descriptions-item>
            <el-descriptions-item label="块类型" v-if="selectedItem.blockType">
              <el-tag size="small" type="info">{{ blockTypeLabels[selectedItem.blockType] || selectedItem.blockType }}</el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="父级条目" v-if="selectedItem.parentId">
              <el-link type="primary" @click="jumpToParent(selectedItem.parentId)">
                {{ selectedItem.parentTitle || selectedItem.parentId }}
              </el-link>
            </el-descriptions-item>
          </el-descriptions>

          <div class="content-section">
            <h4>内容</h4>
            <div class="content-text">{{ selectedItem.content }}</div>
          </div>

          <div v-if="selectedItem.tags" class="tags-section">
            <h4>标签</h4>
            <div class="tags-list">
              <el-tag v-for="tag in selectedItem.tags.split(',')" :key="tag" size="small" style="margin-right: 4px">
                {{ tag.trim() }}
              </el-tag>
            </div>
          </div>

          <!-- 子块树 -->
          <div v-if="blocksLoading || childBlocks.length > 0" class="blocks-section" v-loading="blocksLoading">
            <h4>子块</h4>
            <div
              v-for="block in blocksWithDepth"
              :key="block.id"
              class="block-item"
              :style="{ paddingLeft: (block.depth * 16) + 'px' }"
            >
              <div class="block-head">
                <el-tag size="small" type="info">{{ getBlockTypeLabel(block.blockType) }}</el-tag>
                <span class="block-title">{{ block.title || '（无标题）' }}</span>
              </div>
              <div class="block-content">{{ block.content }}</div>
            </div>
          </div>
        </el-card>
      </div>
    </div>

    <!-- 添加子块对话框 -->
    <el-dialog v-model="showAddBlockDialog" title="添加子块" width="520px">
      <el-form label-width="70px">
        <el-form-item label="标题">
          <el-input v-model="addBlockForm.title" placeholder="子块标题" maxlength="100" />
        </el-form-item>
        <el-form-item label="内容">
          <el-input
            v-model="addBlockForm.content"
            type="textarea"
            :rows="6"
            placeholder="子块内容"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddBlockDialog = false">取消</el-button>
        <el-button type="primary" :loading="addingBlock" @click="confirmAddBlock">添加</el-button>
      </template>
    </el-dialog>

    <!-- 版本历史对话框 -->
    <el-dialog v-model="showVersionDialog" title="版本历史" width="800px">
      <div class="version-header">
        <p>知识条目: <strong>{{ selectedItem?.title }}</strong></p>
        <p class="tip">选择两个版本进行对比，或点击"对比当前"查看与当前内容的差异</p>
      </div>

      <el-table :data="versions" stripe size="small" @selection-change="(rows) => selectedVersions = rows.map(r => r.id)">
        <el-table-column type="selection" width="55" />
        <el-table-column prop="changedAt" label="修改时间" width="180">
          <template #default="{ row }">
            {{ formatDate(row.changedAt) }}
          </template>
        </el-table-column>
        <el-table-column prop="changeReason" label="修改原因" width="120" />
        <el-table-column prop="content" label="内容预览" min-width="200">
          <template #default="{ row }">
            <span class="content-preview">{{ row.content.substring(0, 100) }}...</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120">
          <template #default="{ row }">
            <el-button type="primary" link size="small" @click="compareWithCurrent(row.id)">
              对比当前
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <template #footer>
        <el-button @click="showVersionDialog = false">关闭</el-button>
        <el-button type="primary" @click="compareVersions" :disabled="selectedVersions.length !== 2">
          对比选中版本
        </el-button>
      </template>
    </el-dialog>

    <!-- 差异对比对话框 -->
    <el-dialog v-model="showDiffDialog" title="版本差异对比" width="900px">
      <div v-if="diffResult" class="diff-view">
        <div class="diff-header">
          <div v-if="diffResult.version1" class="diff-version-info">
            <h4>版本 1</h4>
            <p>时间: {{ formatDate(diffResult.version1.changedAt) }}</p>
            <p>原因: {{ diffResult.version1.changeReason || '-' }}</p>
          </div>
          <div v-if="diffResult.version" class="diff-version-info">
            <h4>历史版本</h4>
            <p>时间: {{ formatDate(diffResult.version.changedAt) }}</p>
            <p>原因: {{ diffResult.version.changeReason || '-' }}</p>
          </div>
          <div v-if="diffResult.version2" class="diff-version-info">
            <h4>版本 2</h4>
            <p>时间: {{ formatDate(diffResult.version2.changedAt) }}</p>
            <p>原因: {{ diffResult.version2.changeReason || '-' }}</p>
          </div>
        </div>

        <el-divider />

        <div class="diff-content">
          <div v-for="(line, idx) in diffResult.diffs" :key="idx" :class="['diff-line', line.type]">
            <span class="line-number">{{ line.line }}</span>
            <span class="line-text">{{ line.text }}</span>
          </div>
        </div>
      </div>

      <template #footer>
        <el-button @click="showDiffDialog = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.knowledge-page {
  padding: 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.knowledge-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.knowledge-header h2 {
  margin: 0;
}

.header-actions {
  display: flex;
  gap: 12px;
}

/* 职能标签页 */
.category-tabs {
  display: flex;
  gap: 12px;
  margin-bottom: 16px;
  padding: 12px;
  background: #f5f7fa;
  border-radius: 8px;
}

.category-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  background: #fff;
  border: 1px solid #e4e7ed;
}

.category-tab:hover {
  border-color: var(--tab-color);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.category-tab.active {
  background: var(--tab-color);
  border-color: var(--tab-color);
  color: #fff;
}

.category-tab.active .tab-label {
  color: #fff;
}

.tab-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.tab-label {
  font-size: 14px;
  color: #606266;
}

.knowledge-content {
  flex: 1;
  display: flex;
  gap: 20px;
  overflow: hidden;
}

.knowledge-list {
  flex: 1;
  overflow: auto;
}

.knowledge-detail {
  width: 400px;
  overflow: auto;
}

.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.detail-header h3 {
  margin: 0;
}

.detail-actions {
  display: flex;
  gap: 8px;
}

/* 子块树 */
.blocks-section {
  margin-top: 16px;
}

.blocks-section h4 {
  margin: 0 0 8px;
  font-size: 14px;
  color: #606266;
}

.block-item {
  padding: 8px 0 8px 0;
  border-bottom: 1px solid #f0f2f5;
}

.block-item:last-child {
  border-bottom: none;
}

.block-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.block-title {
  font-size: 13px;
  font-weight: 600;
  color: #303133;
}

.block-content {
  font-size: 13px;
  color: #606266;
  white-space: pre-wrap;
  line-height: 1.6;
}

.content-section {
  margin-top: 16px;
}

.content-section h4 {
  margin: 0 0 8px;
  font-size: 14px;
  color: #606266;
}

.content-text {
  padding: 12px;
  background: #f5f7fa;
  border-radius: 4px;
  white-space: pre-wrap;
  font-size: 14px;
  line-height: 1.6;
}

.tags-section {
  margin-top: 16px;
}

.tags-section h4 {
  margin: 0 0 8px;
  font-size: 14px;
  color: #606266;
}

.tags-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.version-header {
  margin-bottom: 16px;
}

.version-header p {
  margin: 4px 0;
}

.tip {
  color: #909399;
  font-size: 13px;
}

.content-preview {
  font-size: 12px;
  color: #606266;
}

.diff-view {
  max-height: 600px;
  overflow: auto;
}

.diff-header {
  display: flex;
  gap: 24px;
  margin-bottom: 16px;
}

.diff-version-info {
  flex: 1;
  padding: 12px;
  background: #f5f7fa;
  border-radius: 4px;
}

.diff-version-info h4 {
  margin: 0 0 8px;
  font-size: 14px;
}

.diff-version-info p {
  margin: 4px 0;
  font-size: 13px;
}

.diff-content {
  font-family: monospace;
  font-size: 13px;
  line-height: 1.6;
}

.diff-line {
  display: flex;
  padding: 2px 8px;
}

.diff-line.equal {
  background: #fff;
}

.diff-line.added {
  background: #e6ffed;
}

.diff-line.removed {
  background: #ffeef0;
}

.line-number {
  width: 40px;
  color: #909399;
  text-align: right;
  margin-right: 12px;
  user-select: none;
}

.line-text {
  flex: 1;
  white-space: pre-wrap;
}
</style>
