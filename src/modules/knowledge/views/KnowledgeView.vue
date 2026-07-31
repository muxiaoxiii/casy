<script setup>
import { ref, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage } from 'element-plus'

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

const categories = [
  { value: 'law', label: '法律法规' },
  { value: 'case_precedent', label: '案例precedent' },
  { value: 'legal_opinion', label: '法律意见' },
  { value: 'template', label: '模板' },
  { value: 'other', label: '其他' },
]

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

function selectItem(item) {
  selectedItem.value = item
  loadVersions(item.id)
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

onMounted(() => {
  loadKnowledge()
})
</script>

<template>
  <div class="knowledge-page">
    <div class="knowledge-header">
      <h2>知识库</h2>
      <div class="header-actions">
        <el-input
          v-model="filter.search"
          placeholder="搜索知识条目..."
          clearable
          style="width: 200px"
          @keyup.enter="loadKnowledge"
        />
        <el-select v-model="filter.category" placeholder="分类筛选" clearable style="width: 120px">
          <el-option v-for="cat in categories" :key="cat.value" :label="cat.label" :value="cat.value" />
        </el-select>
        <el-button type="primary" @click="loadKnowledge">搜索</el-button>
      </div>
    </div>

    <div class="knowledge-content">
      <!-- 左侧列表 -->
      <div class="knowledge-list">
        <el-table
          :data="knowledgeList"
          v-loading="loading"
          stripe
          highlight-current-row
          @current-change="selectItem"
          style="width: 100%"
        >
          <el-table-column prop="title" label="标题" min-width="200" />
          <el-table-column prop="category" label="分类" width="100">
            <template #default="{ row }">
              <el-tag size="small">{{ row.category }}</el-tag>
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
              <el-button type="primary" size="small" @click="showVersionHistory">
                📜 版本历史
              </el-button>
            </div>
          </template>

          <el-descriptions :column="2" border size="small">
            <el-descriptions-item label="分类">{{ selectedItem.category }}</el-descriptions-item>
            <el-descriptions-item label="状态">{{ selectedItem.status }}</el-descriptions-item>
            <el-descriptions-item label="法律名称">{{ selectedItem.lawName || '-' }}</el-descriptions-item>
            <el-descriptions-item label="条款号">{{ selectedItem.articleNo || '-' }}</el-descriptions-item>
            <el-descriptions-item label="生效日期">{{ selectedItem.effectiveDate || '-' }}</el-descriptions-item>
            <el-descriptions-item label="关联案件">{{ selectedItem.linkedCaseId || '-' }}</el-descriptions-item>
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
        </el-card>
      </div>
    </div>

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
  margin-bottom: 20px;
}

.knowledge-header h2 {
  margin: 0;
}

.header-actions {
  display: flex;
  gap: 12px;
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
