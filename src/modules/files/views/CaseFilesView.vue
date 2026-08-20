<script setup>
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useCasesStore } from '../../../stores/cases'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'

const route = useRoute()
const router = useRouter()
const casesStore = useCasesStore()

const caseId = ref(route.params.caseId)
const caseData = ref(null)
const loading = ref(false)
const activeCategory = ref('all')

// 7 个子目录分类
const categories = [
  { key: 'all', label: '全部', icon: '📁' },
  { key: 'summons', label: '传票/通知书', icon: '📩' },
  { key: 'evidence', label: '证据材料', icon: '📎' },
  { key: 'submitted', label: '提交文件', icon: '📤' },
  { key: 'received', label: '接收文件', icon: '📥' },
  { key: 'internal', label: '内部文件', icon: '📋' },
  { key: 'correspondence', label: '往来函件', icon: '✉️' },
  { key: 'other', label: '其他', icon: '📄' },
]

// 文件列表
const files = ref([])
const filesLoading = ref(false)

// 上传相关
const uploading = ref(false)

async function loadCase() {
  if (!caseId.value) return
  loading.value = true
  const result = await casesStore.loadCase(caseId.value)
  if (result.ok) {
    caseData.value = result.data
  }
  loading.value = false
}

async function loadFiles() {
  if (!caseId.value) return
  filesLoading.value = true
  const result = await casyContext.files.list(
    caseId.value,
    activeCategory.value !== 'all' ? activeCategory.value : undefined
  )
  if (result.ok) {
    files.value = result.data || []
  }
  filesLoading.value = false
}

// 上传文件
async function uploadFile() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({
    multiple: true,
  })
  if (!selected) return

  uploading.value = true
  const paths = Array.isArray(selected) ? selected : [selected]

  for (const filePath of paths) {
    const fileName = filePath.split('/').pop() || filePath.split('\\').pop()
    const result = await casyContext.files.add(
      caseId.value,
      filePath,
      activeCategory.value === 'all' ? 'other' : activeCategory.value
    )
    if (!result.ok) {
      ElMessage.error(`上传失败: ${fileName}`)
    }
  }

  uploading.value = false
  ElMessage.success('文件已添加')
  await loadFiles()
}

// 删除文件
async function deleteFile(file) {
  try {
    await ElMessageBox.confirm(
      `确定删除文件「${file.fileName}」？`,
      '确认删除',
      { type: 'warning' }
    )
  } catch {
    return
  }

  const result = await casyContext.files.remove(file.id)
  if (result.ok) {
    ElMessage.success('已删除')
    await loadFiles()
  } else {
    ElMessage.error(result.error || '删除失败')
  }
}

// 格式化文件大小
function formatSize(bytes) {
  if (!bytes) return '-'
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
}

// 按分类统计
const categoryCounts = computed(() => {
  const counts = { all: files.value.length }
  for (const cat of categories.slice(1)) {
    counts[cat.key] = files.value.filter(f => f.category === cat.key).length
  }
  return counts
})

// 过滤后的文件
const filteredFiles = computed(() => {
  if (activeCategory.value === 'all') return files.value
  return files.value.filter(f => f.category === activeCategory.value)
})

function onCategoryChange(key) {
  activeCategory.value = key
}

onMounted(() => {
  loadCase()
  loadFiles()
})
</script>

<template>
  <div class="case-files-view">
    <!-- 案件信息头部 -->
    <div class="case-header" v-if="caseData">
      <el-page-header @back="router.back()">
        <template #content>
          <span class="case-title">{{ caseData.caseName }}</span>
          <el-tag v-if="caseData.caseNo" size="small" style="margin-left: 8px">{{ caseData.caseNo }}</el-tag>
        </template>
      </el-page-header>
    </div>

    <div class="files-body">
      <!-- 左侧分类导航 -->
      <div class="category-nav">
        <div
          v-for="cat in categories"
          :key="cat.key"
          :class="['category-item', { active: activeCategory === cat.key }]"
          @click="onCategoryChange(cat.key)"
        >
          <span class="cat-icon">{{ cat.icon }}</span>
          <span class="cat-label">{{ cat.label }}</span>
          <el-badge
            :value="categoryCounts[cat.key] || 0"
            :max="99"
            class="cat-count"
            type="info"
          />
        </div>
      </div>

      <!-- 右侧文件列表 -->
      <div class="files-panel">
        <div class="files-toolbar">
          <el-button type="primary" :loading="uploading" @click="uploadFile">
            📎 上传文件
          </el-button>
          <el-button @click="loadFiles" :loading="filesLoading">刷新</el-button>
        </div>

        <el-table
          :data="filteredFiles"
          v-loading="filesLoading"
          stripe
          style="width: 100%"
          empty-text="暂无文件"
        >
          <el-table-column prop="fileName" label="文件名" min-width="200" show-overflow-tooltip />
          <el-table-column label="分类" width="120">
            <template #default="{ row }">
              <el-tag size="small">{{ categories.find(c => c.key === row.category)?.label || row.category }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="大小" width="100" align="right">
            <template #default="{ row }">{{ formatSize(row.fileSize) }}</template>
          </el-table-column>
          <el-table-column prop="createdAt" label="添加时间" width="170" />
          <el-table-column label="操作" width="120" fixed="right">
            <template #default="{ row }">
              <el-button type="danger" size="small" text @click="deleteFile(row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.case-files-view {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 100px);
}

.case-header {
  padding-bottom: 16px;
  border-bottom: 1px solid #e0e0e0;
}

.case-title {
  font-size: 18px;
  font-weight: 600;
}

.files-body {
  flex: 1;
  display: flex;
  gap: 16px;
  padding-top: 16px;
  overflow: hidden;
}

.category-nav {
  width: 180px;
  flex-shrink: 0;
  overflow-y: auto;
}

.category-item {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.2s;
  margin-bottom: 4px;
}

.category-item:hover {
  background-color: #f5f7fa;
}

.category-item.active {
  background-color: #ecf5ff;
  color: #409eff;
}

.cat-icon {
  margin-right: 8px;
  font-size: 16px;
}

.cat-label {
  flex: 1;
  font-size: 14px;
}

.cat-count {
  margin-left: auto;
}

.files-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.files-toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
</style>
