<script setup>
import { ref, computed, onMounted, nextTick } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { aiToolCaller } from '../../../core/ai/tool-caller'
import { ElMessage } from 'element-plus'
import {
  ChatDotRound,
  Position,
  Loading,
  Tools,
  CircleCheck,
  CircleClose,
} from '@element-plus/icons-vue'

// ============================================================
// 状态
// ============================================================
const messages = ref([])
const inputMessage = ref('')
const loading = ref(false)
const showToolCalls = ref(false)

// 可用模型
const availableModels = ref([])
const selectedProvider = ref('ollama')
const selectedModel = ref('qwen2.5:14b')

// ============================================================
// 计算属性
// ============================================================
const toolDefinitions = computed(() => {
  return casyContext.getToolDefinitions()
})

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  loadModels()
  addSystemMessage()
})

// ============================================================
// 函数
// ============================================================
function loadModels() {
  const providers = casyContext.getProviders()
  availableModels.value = providers.flatMap(p => 
    p.models.map(m => ({
      provider: p.name,
      model: m.id,
      label: `${p.name}/${m.name}`,
    }))
  )
}

function addSystemMessage() {
  messages.value.push({
    role: 'system',
    content: `我是 Casy AI 助手，可以帮你查询案件、管理任务、搜索知识库。

我可以调用以下工具：
${toolDefinitions.value.map(t => `- ${t.name}: ${t.description}`).join('\n')}

请告诉我你需要什么帮助。`,
    timestamp: new Date(),
  })
}

async function sendMessage() {
  const content = inputMessage.value.trim()
  if (!content || loading.value) return
  
  // 添加用户消息
  messages.value.push({
    role: 'user',
    content,
    timestamp: new Date(),
  })
  
  inputMessage.value = ''
  loading.value = true
  
  try {
    // 设置模型
    aiToolCaller.setModel(selectedProvider.value, selectedModel.value)
    
    // 构建消息历史
    const history = messages.value
      .filter(m => m.role !== 'system')
      .map(m => ({
        role: m.role,
        content: m.content,
      }))
    
    // 调用 AI（带工具调用）
    const result = await aiToolCaller.chatWithTools(
      [{ role: 'system', content: '你是 Casy AI 助手，帮助律师管理案件、任务和知识库。' }, ...history],
      { autoConfirm: false }
    )
    
    // 添加 AI 响应
    messages.value.push({
      role: 'assistant',
      content: result.content,
      toolCalls: result.toolCalls,
      toolResults: result.toolResults,
      timestamp: new Date(),
    })
    
    // 如果有工具调用结果，添加工具调用详情
    if (result.toolCalls.length > 0) {
      messages.value.push({
        role: 'system',
        content: `执行了 ${result.toolCalls.length} 个工具调用`,
        toolCallDetails: result.toolCalls.map((tc, i) => ({
          name: tc.name,
          params: tc.params,
          result: result.toolResults[i],
        })),
        timestamp: new Date(),
      })
    }
  } catch (error) {
    console.error('AI error:', error)
    messages.value.push({
      role: 'assistant',
      content: `抱歉，发生了错误：${error.message}`,
      timestamp: new Date(),
    })
  } finally {
    loading.value = false
    scrollToBottom()
  }
}

function scrollToBottom() {
  nextTick(() => {
    const container = document.querySelector('.chat-messages')
    if (container) {
      container.scrollTop = container.scrollHeight
    }
  })
}

function formatTime(date) {
  if (!date) return ''
  return new Date(date).toLocaleTimeString('zh-CN', { 
    hour: '2-digit', 
    minute: '2-digit' 
  })
}

function clearChat() {
  messages.value = []
  addSystemMessage()
}
</script>

<template>
  <div class="ai-chat-panel">
    <!-- 顶部工具栏 -->
    <div class="chat-toolbar">
      <div class="model-selector">
        <el-select 
          v-model="selectedProvider" 
          size="small" 
          style="width: 100px"
          placeholder="提供商"
        >
          <el-option label="Ollama" value="ollama" />
          <el-option label="OpenAI" value="openai" />
          <el-option label="DeepSeek" value="deepseek" />
        </el-select>
        
        <el-select 
          v-model="selectedModel" 
          size="small" 
          style="width: 150px"
          placeholder="模型"
        >
          <el-option 
            v-for="m in availableModels" 
            :key="m.model"
            :label="m.label" 
            :value="m.model" 
          />
        </el-select>
      </div>
      
      <div class="toolbar-actions">
        <el-switch
          v-model="showToolCalls"
          size="small"
          active-text="显示工具调用"
          inactive-text=""
        />
        <el-button size="small" @click="clearChat">清空</el-button>
      </div>
    </div>
    
    <!-- 消息列表 -->
    <div class="chat-messages">
      <div 
        v-for="(msg, index) in messages" 
        :key="index"
        :class="['message', `message-${msg.role}`]"
      >
        <!-- 系统消息 -->
        <div v-if="msg.role === 'system'" class="system-message">
          <div class="system-content" v-html="msg.content"></div>
          
          <!-- 工具调用详情 -->
          <div v-if="msg.toolCallDetails && showToolCalls" class="tool-call-details">
            <div 
              v-for="(tc, i) in msg.toolCallDetails" 
              :key="i"
              class="tool-call-item"
            >
              <div class="tool-call-header">
                <el-icon><Tools /></el-icon>
                <span class="tool-name">{{ tc.name }}</span>
              </div>
              <div class="tool-call-params">
                <pre>{{ JSON.stringify(tc.params, null, 2) }}</pre>
              </div>
              <div class="tool-call-result">
                <el-icon v-if="tc.result?.ok !== false" color="#67C23A"><CircleCheck /></el-icon>
                <el-icon v-else color="#F56C6C"><CircleClose /></el-icon>
                <span>{{ tc.result?.ok !== false ? '成功' : '失败' }}</span>
              </div>
            </div>
          </div>
        </div>
        
        <!-- 用户消息 -->
        <div v-else-if="msg.role === 'user'" class="user-message">
          <div class="message-content">{{ msg.content }}</div>
          <div class="message-time">{{ formatTime(msg.timestamp) }}</div>
        </div>
        
        <!-- AI 消息 -->
        <div v-else-if="msg.role === 'assistant'" class="assistant-message">
          <div class="message-content">{{ msg.content }}</div>
          
          <!-- 工具调用摘要 -->
          <div v-if="msg.toolCalls?.length > 0 && showToolCalls" class="tool-calls-summary">
            <el-tag size="small" type="info">
              调用了 {{ msg.toolCalls.length }} 个工具
            </el-tag>
          </div>
          
          <div class="message-time">{{ formatTime(msg.timestamp) }}</div>
        </div>
      </div>
      
      <!-- 加载中 -->
      <div v-if="loading" class="message message-loading">
        <el-icon class="loading-icon"><Loading /></el-icon>
        <span>思考中...</span>
      </div>
    </div>
    
    <!-- 输入区域 -->
    <div class="chat-input">
      <el-input
        v-model="inputMessage"
        type="textarea"
        :rows="2"
        placeholder="输入消息，例如：查询所有进行中的案件"
        @keydown.enter.ctrl="sendMessage"
        @keydown.enter.meta="sendMessage"
      />
      <el-button 
        type="primary" 
        :icon="Position"
        :loading="loading"
        @click="sendMessage"
      >
        发送
      </el-button>
    </div>
  </div>
</template>

<style scoped>
.ai-chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #fff;
  border-radius: 8px;
  overflow: hidden;
}

.chat-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #e5e7eb;
  background: #f9fafb;
}

.model-selector {
  display: flex;
  gap: 8px;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.message {
  margin-bottom: 16px;
}

.system-message {
  background: #f3f4f6;
  border-radius: 8px;
  padding: 12px;
  font-size: 13px;
  color: #6b7280;
  white-space: pre-wrap;
}

.user-message {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.user-message .message-content {
  background: #3b82f6;
  color: white;
  border-radius: 8px 8px 0 8px;
  padding: 8px 12px;
  max-width: 80%;
}

.assistant-message {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
}

.assistant-message .message-content {
  background: #f3f4f6;
  border-radius: 8px 8px 8px 0;
  padding: 8px 12px;
  max-width: 80%;
  white-space: pre-wrap;
}

.message-time {
  font-size: 11px;
  color: #9ca3af;
  margin-top: 4px;
}

.tool-calls-summary {
  margin-top: 8px;
}

.tool-call-details {
  margin-top: 12px;
  border-top: 1px solid #e5e7eb;
  padding-top: 12px;
}

.tool-call-item {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
  padding: 8px;
  margin-bottom: 8px;
}

.tool-call-header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-weight: 500;
  margin-bottom: 4px;
}

.tool-name {
  color: #3b82f6;
}

.tool-call-params {
  background: #f9fafb;
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 12px;
  overflow-x: auto;
}

.tool-call-params pre {
  margin: 0;
  font-family: monospace;
}

.tool-call-result {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  font-size: 12px;
}

.message-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #6b7280;
}

.loading-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.chat-input {
  display: flex;
  gap: 8px;
  padding: 12px;
  border-top: 1px solid #e5e7eb;
}

.chat-input .el-textarea {
  flex: 1;
}
</style>
