<template>
  <div class="copilot-sidebar">
    <!-- 顶部搜索框 -->
    <div class="sidebar-search">
      <el-input
        :model-value="searchQuery"
        placeholder="🔍 搜索我的知识库..."
        clearable
        @input="onSearchInput"
        @clear="onSearchClear"
        class="search-input"
      >
        <template #prefix>
          <el-icon v-if="searching" class="is-loading"><Loading /></el-icon>
          <el-icon v-else><Search /></el-icon>
        </template>
      </el-input>
    </div>

    <!-- 内容区域 -->
    <div class="sidebar-content">
      <!-- 相关段落 -->
      <div class="sidebar-section">
        <div
          class="section-header"
          @click="toggleSection('paragraphs')"
        >
          <span class="section-icon">📋</span>
          <span class="section-title">相关段落</span>
          <span class="section-count">{{ searchResults.paragraphs.length }}</span>
          <el-icon class="section-arrow" :class="{ expanded: expandedSections.paragraphs }">
            <ArrowDown />
          </el-icon>
        </div>
        <div v-show="expandedSections.paragraphs" class="section-body">
          <div v-if="!searchQuery" class="section-empty">
            输入关键词或在编辑器中移动光标自动检索
          </div>
          <div v-else-if="searching" class="section-empty">
            检索中...
          </div>
          <div v-else-if="searchResults.paragraphs.length === 0" class="section-empty">
            未找到相关段落
          </div>
          <div
            v-for="item in searchResults.paragraphs"
            :key="item.id"
            class="knowledge-card"
            :class="{ expanded: expandedItemId === item.id }"
          >
            <div class="card-header" @click="toggleExpand(item.id)">
              <div class="card-title-row">
                <span class="card-icon">{{ getCategoryIcon(item.category) }}</span>
                <span class="card-title">{{ item.title }}</span>
              </div>
              <div class="card-meta">
                <el-tag size="small" type="info">{{ getCategoryLabel(item.category) }}</el-tag>
                <span v-if="item.score" class="card-score">{{ formatScore(item.score) }}</span>
                <span v-if="item.source" class="card-source">{{ item.source }}</span>
              </div>
            </div>
            <div v-if="expandedItemId === item.id" class="card-body">
              <div class="card-content">{{ item.content }}</div>
            </div>
            <div class="card-actions">
              <el-button size="small" type="primary" text @click.stop="emitInsert(item)">
                插入
              </el-button>
              <el-button size="small" text @click.stop="emitCitation(item)">
                引用
              </el-button>
              <el-button size="small" text @click.stop="emitCopy(item)">
                复制
              </el-button>
            </div>
          </div>
        </div>
      </div>

      <!-- 相关法条 -->
      <div class="sidebar-section">
        <div
          class="section-header"
          @click="toggleSection('laws')"
        >
          <span class="section-icon">📜</span>
          <span class="section-title">相关法条</span>
          <span class="section-count">{{ searchResults.laws.length }}</span>
          <el-icon class="section-arrow" :class="{ expanded: expandedSections.laws }">
            <ArrowDown />
          </el-icon>
        </div>
        <div v-show="expandedSections.laws" class="section-body">
          <div v-if="!searchQuery" class="section-empty">
            输入关键词检索相关法条
          </div>
          <div v-else-if="searchResults.laws.length === 0" class="section-empty">
            未找到相关法条
          </div>
          <div
            v-for="item in searchResults.laws"
            :key="item.id"
            class="knowledge-card law-card"
            :class="{ expanded: expandedItemId === item.id }"
          >
            <div class="card-header" @click="toggleExpand(item.id)">
              <div class="card-title-row">
                <span class="card-icon">📖</span>
                <span class="card-title">
                  {{ item.lawName ? `《${item.lawName}》` : '' }}{{ item.articleNo || '' }}
                </span>
              </div>
              <div class="card-meta">
                <el-tag size="small" type="warning">法条</el-tag>
                <span v-if="item.score" class="card-score">{{ formatScore(item.score) }}</span>
              </div>
            </div>
            <div class="card-subtitle">{{ item.title }}</div>
            <div v-if="expandedItemId === item.id" class="card-body">
              <div class="card-content">{{ item.content }}</div>
            </div>
            <div class="card-actions">
              <el-button size="small" type="primary" text @click.stop="emitInsert(item)">
                插入
              </el-button>
              <el-button size="small" text @click.stop="emitCopy(item)">
                复制引用
              </el-button>
            </div>
          </div>
        </div>
      </div>

      <!-- 相关判例 -->
      <div class="sidebar-section">
        <div
          class="section-header"
          @click="toggleSection('cases')"
        >
          <span class="section-icon">⚖️</span>
          <span class="section-title">相关判例</span>
          <span class="section-count">{{ searchResults.cases.length }}</span>
          <el-icon class="section-arrow" :class="{ expanded: expandedSections.cases }">
            <ArrowDown />
          </el-icon>
        </div>
        <div v-show="expandedSections.cases" class="section-body">
          <div v-if="!searchQuery" class="section-empty">
            输入关键词检索相关判例
          </div>
          <div v-else-if="searchResults.cases.length === 0" class="section-empty">
            未找到相关判例
          </div>
          <div
            v-for="item in searchResults.cases"
            :key="item.id"
            class="knowledge-card case-card"
            :class="{ expanded: expandedItemId === item.id }"
          >
            <div class="card-header" @click="toggleExpand(item.id)">
              <div class="card-title-row">
                <span class="card-icon">⚖️</span>
                <span class="card-title">{{ item.title }}</span>
              </div>
              <div class="card-meta">
                <el-tag size="small" type="success">判例</el-tag>
                <span v-if="item.score" class="card-score">{{ formatScore(item.score) }}</span>
              </div>
            </div>
            <div v-if="expandedItemId === item.id" class="card-body">
              <div class="card-content">{{ item.content }}</div>
            </div>
            <div class="card-actions">
              <el-button size="small" type="primary" text @click.stop="emitInsert(item)">
                插入
              </el-button>
              <el-button size="small" text @click.stop="emitCitation(item)">
                引用
              </el-button>
              <el-button size="small" text @click.stop="emitCopy(item)">
                复制
              </el-button>
            </div>
          </div>
        </div>
      </div>

      <!-- AI 写作辅助 -->
      <div class="sidebar-section ai-section">
        <div
          class="section-header"
          @click="toggleSection('ai')"
        >
          <span class="section-icon">✨</span>
          <span class="section-title">AI 写作辅助</span>
          <el-tag size="small" type="primary" class="ai-badge">Ctrl+K</el-tag>
          <el-icon class="section-arrow" :class="{ expanded: expandedSections.ai }">
            <ArrowDown />
          </el-icon>
        </div>
        <div v-show="expandedSections.ai" class="section-body">
          <div class="ai-quick-actions">
            <el-button size="small" @click="emitAiAction('生成损害赔偿计算的事实与理由')">
              💰 损害赔偿
            </el-button>
            <el-button size="small" @click="emitAiAction('生成专利侵权的技术比对分析')">
              🔍 技术比对
            </el-button>
            <el-button size="small" @click="emitAiAction('生成诉讼请求')">
              📝 诉讼请求
            </el-button>
            <el-button size="small" @click="emitAiAction('总结案件事实')">
              📋 案件事实
            </el-button>
          </div>
          <div class="ai-custom">
            <el-input
              v-model="aiIntent"
              type="textarea"
              :rows="2"
              placeholder="描述你的写作意图..."
              resize="none"
            />
            <el-button
              type="primary"
              size="small"
              :loading="generating"
              :disabled="!aiIntent.trim()"
              @click="emitAiAction(aiIntent)"
              style="margin-top: 8px; width: 100%"
            >
              {{ generating ? '生成中...' : '✨ 生成建议' }}
            </el-button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { reactive, ref, watch } from 'vue'
import { Search, ArrowDown, Loading } from '@element-plus/icons-vue'

const props = defineProps({
  /** 搜索查询 */
  searchQuery: { type: String, default: '' },
  /** 检索结果 { paragraphs: [], laws: [], cases: [] } */
  searchResults: {
    type: Object,
    default: () => ({ paragraphs: [], laws: [], cases: [] }),
  },
  /** 是否正在检索 */
  searching: { type: Boolean, default: false },
  /** 是否正在生成 AI 建议 */
  generating: { type: Boolean, default: false },
  /** 当前展开的知识条目 ID */
  expandedItemId: { type: String, default: null },
  /** 知识条目分类标签映射 */
  getCategoryLabel: { type: Function, default: () => '' },
  /** 知识条目分类图标映射 */
  getCategoryIcon: { type: Function, default: () => '📝' },
})

const emit = defineEmits([
  'update:searchQuery',
  'search',
  'insert',
  'citation',
  'copy',
  'toggle-expand',
  'ai-action',
])

// 区域展开状态
const expandedSections = reactive({
  paragraphs: true,
  laws: true,
  cases: true,
  ai: true,
})

// AI 自定义意图
const aiIntent = ref('')

function toggleSection(key) {
  expandedSections[key] = !expandedSections[key]
}

function onSearchInput(val) {
  emit('update:searchQuery', val)
  emit('search', val)
}

function onSearchClear() {
  emit('update:searchQuery', '')
  emit('search', '')
}

function emitInsert(item) {
  emit('insert', item)
}

function emitCitation(item) {
  emit('citation', item)
}

function emitCopy(item) {
  emit('copy', item)
}

function emitAiAction(intent) {
  if (intent && intent.trim()) {
    emit('ai-action', intent.trim())
  }
}

/** 格式化相关度分数为百分比 */
function formatScore(score) {
  if (typeof score !== 'number') return ''
  // RRF score 通常很小，映射为百分比显示
  if (score < 1) {
    return `${Math.round(score * 100)}%`
  }
  return score.toFixed(2)
}
</script>

<style scoped>
.copilot-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #fafbfc;
  border-left: 1px solid #e4e7ed;
  overflow: hidden;
}

.sidebar-search {
  padding: 12px;
  border-bottom: 1px solid #e4e7ed;
  background: #fff;
}

.search-input :deep(.el-input__inner) {
  font-size: 13px;
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

/* 区域样式 */
.sidebar-section {
  border-bottom: 1px solid #f0f0f0;
}

.section-header {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  cursor: pointer;
  user-select: none;
  transition: background 0.15s;
  gap: 6px;
}

.section-header:hover {
  background: #f0f2f5;
}

.section-icon {
  font-size: 14px;
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  color: #303133;
  flex: 1;
}

.section-count {
  font-size: 11px;
  color: #909399;
  background: #f0f2f5;
  padding: 1px 6px;
  border-radius: 10px;
  min-width: 18px;
  text-align: center;
}

.section-arrow {
  font-size: 12px;
  color: #909399;
  transition: transform 0.2s;
}

.section-arrow.expanded {
  transform: rotate(180deg);
}

.section-body {
  padding: 0 8px 8px;
}

.section-empty {
  padding: 12px 8px;
  font-size: 12px;
  color: #909399;
  text-align: center;
}

/* 知识卡片 */
.knowledge-card {
  background: #fff;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  margin-bottom: 6px;
  transition: box-shadow 0.15s;
  overflow: hidden;
}

.knowledge-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.card-header {
  padding: 8px 10px;
  cursor: pointer;
}

.card-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.card-icon {
  font-size: 13px;
  flex-shrink: 0;
}

.card-title {
  font-size: 13px;
  font-weight: 500;
  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-subtitle {
  padding: 0 10px 6px;
  font-size: 12px;
  color: #606266;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}

.card-score {
  font-size: 11px;
  color: #67c23a;
  font-weight: 500;
}

.card-source {
  font-size: 11px;
  color: #909399;
}

.card-body {
  padding: 0 10px 8px;
}

.card-content {
  font-size: 12px;
  line-height: 1.6;
  color: #606266;
  max-height: 200px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
  background: #f5f7fa;
  padding: 8px;
  border-radius: 4px;
}

.card-actions {
  display: flex;
  padding: 0 6px 6px;
  gap: 2px;
}

/* AI 区域 */
.ai-section .section-header {
  background: linear-gradient(90deg, #ecf5ff 0%, #fafbfc 100%);
}

.ai-badge {
  font-size: 10px;
}

.ai-quick-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 8px;
}

.ai-quick-actions .el-button {
  font-size: 12px;
}

.ai-custom {
  padding: 0 2px;
}

.ai-custom :deep(.el-textarea__inner) {
  font-size: 12px;
  padding: 8px;
}
</style>
