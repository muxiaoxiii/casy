<script setup>
import { ref, computed, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import { 
  ChatDotRound, 
  List, 
  Document, 
  DataBoard,
  Setting 
} from '@element-plus/icons-vue'
import AIChatPanel from '../components/AIChatPanel.vue'
import AIAuditView from './AIAuditView.vue'
import DecisionsView from './DecisionsView.vue'
import ConfirmDialog from '../components/ConfirmDialog.vue'

const activeTab = ref('chat')
const aiConfig = ref(null)
const aiUsage = ref(null)
const loading = ref(false)

// 工具统计
const toolStats = computed(() => {
  const tools = casyContext.getTools()
  const categories = {}
  tools.forEach(t => {
    const cat = t.category || 'other'
    categories[cat] = (categories[cat] || 0) + 1
  })
  return { total: tools.length, categories }
})

// AI 状态摘要
const statusSummary = computed(() => {
  const todayCalls = aiUsage.value?.todayCalls ?? 0
  const dailyLimit = aiConfig.value?.dailyLimit ?? 50
  const remaining = dailyLimit === 0 ? '不限' : Math.max(0, dailyLimit - todayCalls)
  const mode = aiConfig.value?.mode ?? 'noop'

  return {
    todayCalls,
    dailyLimit,
    remaining,
    mode,
    modeLabel: mode === 'noop' ? '规则匹配' : mode === 'ollama' ? 'Ollama' : 'OpenAI',
    modeColor: mode === 'noop' ? '#909399' : '#67C23A',
  }
})

async function loadAIData() {
  loading.value = true
  const [configResult, usageResult] = await Promise.all([
    tauriCallSafe('get_ai_config'),
    tauriCallSafe('get_ai_usage'),
  ])

  if (configResult.ok) {
    aiConfig.value = configResult.data
  }
  if (usageResult.ok) {
    aiUsage.value = usageResult.data
  }
  loading.value = false
}

onMounted(() => {
  loadAIData()
})
</script>

<template>
  <div class="ai-companion-page">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h3>AI 智伴</h3>
        <span class="header-desc">智能对话 · 工具调用 · 审计追踪</span>
      </div>
    </div>

    <!-- AI 状态摘要 -->
    <div class="status-bar">
      <div class="status-item">
        <span class="status-label">AI 模式</span>
        <el-tag :color="statusSummary.modeColor" effect="dark" size="small">
          {{ statusSummary.modeLabel }}
        </el-tag>
      </div>
      <el-divider direction="vertical" />
      <div class="status-item">
        <span class="status-label">今日调用</span>
        <span class="status-value">{{ statusSummary.todayCalls }}</span>
      </div>
      <el-divider direction="vertical" />
      <div class="status-item">
        <span class="status-label">剩余配额</span>
        <span class="status-value" :class="{ 'text-warning': statusSummary.remaining !== '不限' && statusSummary.remaining < 10 }">
          {{ statusSummary.remaining }}
        </span>
      </div>
      <el-divider direction="vertical" />
      <div class="status-item">
        <span class="status-label">已注册工具</span>
        <span class="status-value">{{ toolStats.total }}</span>
      </div>
    </div>

    <!-- 主要内容区 -->
    <el-tabs v-model="activeTab" class="main-tabs">
      <el-tab-pane name="chat">
        <template #label>
          <el-icon><ChatDotRound /></el-icon>
          <span>AI 对话</span>
        </template>
        <AIChatPanel />
      </el-tab-pane>

      <el-tab-pane name="audit">
        <template #label>
          <el-icon><List /></el-icon>
          <span>审计日志</span>
        </template>
        <AIAuditView />
      </el-tab-pane>

      <el-tab-pane name="decisions">
        <template #label>
          <el-icon><Document /></el-icon>
          <span>决策记录</span>
        </template>
        <DecisionsView />
      </el-tab-pane>

      <el-tab-pane name="tools">
        <template #label>
          <el-icon><Setting /></el-icon>
          <span>工具管理</span>
        </template>
        <div class="tools-section">
          <el-card shadow="never">
            <template #header>
              <div class="card-header">
                <span>已注册工具</span>
                <el-tag size="small">{{ toolStats.total }} 个</el-tag>
              </div>
            </template>
            
            <div class="tool-categories">
              <el-tag 
                v-for="(count, category) in toolStats.categories" 
                :key="category"
                class="category-tag"
              >
                {{ category }}: {{ count }}
              </el-tag>
            </div>
            
            <div class="tool-list">
              <div 
                v-for="tool in casyContext.getTools()" 
                :key="tool.name"
                class="tool-item"
              >
                <div class="tool-info">
                  <span class="tool-name">{{ tool.name }}</span>
                  <span class="tool-category">{{ tool.category || 'other' }}</span>
                </div>
                <div class="tool-description">{{ tool.description }}</div>
              </div>
            </div>
          </el-card>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- 确认对话框 -->
    <ConfirmDialog
      v-model:visible="showConfirmDialog"
      :recommendation="currentRecommendation"
      :level="currentConfirmLevel"
      @confirm="handleConfirm"
      @reject="handleReject"
    />
  </div>
</template>

<style scoped>
.ai-companion-page {
  padding: 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.page-header h3 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
}

.header-desc {
  color: #6b7280;
  font-size: 13px;
  margin-left: 12px;
}

.status-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: #f9fafb;
  border-radius: 8px;
  margin-bottom: 16px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-label {
  color: #6b7280;
  font-size: 13px;
}

.status-value {
  font-weight: 500;
}

.text-warning {
  color: #e6a23c;
}

.main-tabs {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.main-tabs :deep(.el-tabs__content) {
  flex: 1;
  overflow: hidden;
}

.main-tabs :deep(.el-tab-pane) {
  height: 100%;
}

.tools-section {
  height: 100%;
  overflow-y: auto;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.tool-categories {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 16px;
}

.category-tag {
  text-transform: capitalize;
}

.tool-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tool-item {
  padding: 12px;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
}

.tool-info {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.tool-name {
  font-weight: 500;
  color: #3b82f6;
}

.tool-category {
  font-size: 12px;
  color: #6b7280;
  background: #f3f4f6;
  padding: 2px 6px;
  border-radius: 4px;
}

.tool-description {
  font-size: 13px;
  color: #6b7280;
}
</style>
