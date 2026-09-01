<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { Search, Loading, Close, Document, MagicStick } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

const props = defineProps<{
  modelValue: boolean
  caseId: string
  fileIds: string[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const query = ref('')
const isSearching = ref(false)
const searchLogs = ref<{ timestamp: string; message: string; status: string }[]>([])
const finalResult = ref<any>(null)
const logContainerRef = ref<HTMLElement | null>(null)

let unlisten: UnlistenFn | null = null

onMounted(async () => {
  unlisten = await listen('reasoning_progress', (event: any) => {
    const payload = event.payload
    
    // 构造终端风格时间戳
    const now = new Date()
    const ts = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}`
    
    searchLogs.value.push({
      timestamp: ts,
      message: payload.message,
      status: payload.status
    })

    // 自动滚动到底部
    nextTick(() => {
      if (logContainerRef.value) {
        logContainerRef.value.scrollTop = logContainerRef.value.scrollHeight
      }
    })
  })
})

onUnmounted(() => {
  if (unlisten) {
    unlisten()
  }
})

function closePanel() {
  emit('update:modelValue', false)
}

async function doSearch() {
  if (!query.value.trim()) return
  if (!props.fileIds || props.fileIds.length === 0) {
    ElMessage.warning('当前案件暂无已建立索引的文件')
    return
  }

  isSearching.value = true
  searchLogs.value = []
  finalResult.value = null

  try {
    const res: string = await invoke('reasoning_search', {
      query: query.value,
      scope: props.fileIds
    })
    
    // res 应当为 JSON 字符串
    const parsed = JSON.parse(res)
    if (parsed.status === 'success' && parsed.data.length > 0) {
      finalResult.value = parsed.data[0]
    } else {
      throw new Error('未获取到有效回答')
    }
  } catch (error: any) {
    ElMessage.error(error.toString())
    searchLogs.value.push({
      timestamp: new Date().toLocaleTimeString(),
      message: `Error: ${error.toString()}`,
      status: 'error'
    })
  } finally {
    isSearching.value = false
  }
}
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    @update:model-value="closePanel"
    direction="rtl"
    size="520px"
    class="reasoning-drawer"
    :with-header="false"
  >
    <div class="drawer-content">
      <!-- 头部 -->
      <div class="drawer-header">
        <div class="header-title">
          <el-icon class="title-icon"><MagicStick /></el-icon>
          <h2>深度推理检索 (Reasoning RAG)</h2>
        </div>
        <el-button link @click="closePanel" class="close-btn">
          <el-icon :size="20"><Close /></el-icon>
        </el-button>
      </div>

      <!-- 搜索框 -->
      <div class="search-box">
        <el-input
          v-model="query"
          placeholder="请输入您要检索的复杂案情问题..."
          @keyup.enter="doSearch"
          :disabled="isSearching"
          clearable
        >
          <template #append>
            <el-button @click="doSearch" :disabled="isSearching" type="primary">
              <el-icon v-if="isSearching" class="is-loading"><Loading /></el-icon>
              <el-icon v-else><Search /></el-icon>
              {{ isSearching ? '正在推理...' : '开始探索' }}
            </el-button>
          </template>
        </el-input>
      </div>

      <!-- 终端日志区域 -->
      <div class="terminal-log" ref="logContainerRef" v-show="searchLogs.length > 0">
        <div class="terminal-header">
          <span class="dot red"></span>
          <span class="dot yellow"></span>
          <span class="dot green"></span>
          <span class="terminal-title">Agent Thought Process</span>
        </div>
        <div class="terminal-body">
          <div v-for="(log, idx) in searchLogs" :key="idx" class="log-line" :class="log.status">
            <span class="log-ts">[{{ log.timestamp }}]</span>
            <span class="log-msg">{{ log.message }}</span>
          </div>
          <div v-if="isSearching" class="log-line typing">
            <span class="log-ts">[{{ new Date().toLocaleTimeString() }}]</span>
            <span class="log-msg cursor-blink">...</span>
          </div>
        </div>
      </div>

      <!-- 最终结果展示 -->
      <div class="result-box" v-if="finalResult">
        <div class="result-header">
          <h3>推理结论</h3>
        </div>
        <div class="result-body markdown-body">
          {{ finalResult.answer }}
        </div>
        <div class="result-citations" v-if="finalResult.references && finalResult.references.length > 0">
          <h4>来源溯源</h4>
          <div class="citation-tags">
            <el-tag v-for="(ref, i) in finalResult.references" :key="i" size="small" type="info" class="cite-tag">
              <el-icon><Document /></el-icon> {{ ref }}
            </el-tag>
          </div>
        </div>
      </div>
      
      <!-- 空状态 -->
      <div class="empty-state" v-if="!isSearching && searchLogs.length === 0 && !finalResult">
        <el-icon class="empty-icon"><MagicStick /></el-icon>
        <p>基于最新 PageIndex 架构，无需全量读取。<br/>AI Agent 将按需翻阅卷宗，为您进行深度逻辑推理。</p>
      </div>
    </div>
  </el-drawer>
</template>

<style scoped>
.reasoning-drawer :deep(.el-drawer__body) {
  padding: 0;
  background: var(--c-surface);
}

.drawer-content {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 24px;
}

.drawer-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.header-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.title-icon {
  font-size: 22px;
  color: var(--c-primary);
}

.header-title h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--c-text);
}

.close-btn {
  color: var(--c-text-secondary);
}
.close-btn:hover {
  color: var(--c-text);
}

.search-box {
  margin-bottom: 24px;
}
.search-box :deep(.el-input-group__append) {
  background-color: var(--c-primary);
  color: white;
  border-color: var(--c-primary);
  padding: 0 16px;
}
.search-box :deep(.el-button) {
  border-radius: 0 4px 4px 0;
  display: flex;
  gap: 6px;
  font-weight: 600;
}

.terminal-log {
  background: #1e1e1e;
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  margin-bottom: 24px;
  flex-shrink: 0;
  max-height: 250px;
}

.terminal-header {
  background: #2d2d2d;
  padding: 8px 12px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}
.dot.red { background: #ff5f56; }
.dot.yellow { background: #ffbd2e; }
.dot.green { background: #27c93f; }

.terminal-title {
  margin-left: 8px;
  color: #888;
  font-size: 11px;
  font-family: var(--font-mono);
  letter-spacing: 0.5px;
}

.terminal-body {
  padding: 12px 16px;
  overflow-y: auto;
  flex: 1;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
}

.log-line {
  margin-bottom: 6px;
  word-break: break-all;
}

.log-ts {
  color: #666;
  margin-right: 8px;
}

.log-msg {
  color: #d4d4d4;
}

.log-line.start .log-msg { color: #569cd6; }
.log-line.reading .log-msg { color: #ce9178; }
.log-line.extracting .log-msg { color: #dcdcaa; }
.log-line.extracted .log-msg { color: #b5cea8; }
.log-line.success .log-msg { color: #4ec9b0; font-weight: bold; }
.log-line.error .log-msg { color: #f14c4c; }

.cursor-blink {
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  50% { opacity: 0; }
}

.result-box {
  flex: 1;
  background: var(--c-bg-muted);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  padding: 20px;
  overflow-y: auto;
}

.result-header h3 {
  margin: 0 0 16px 0;
  font-size: 16px;
  color: var(--c-text);
  font-weight: 600;
}

.result-body {
  color: var(--c-text-regular);
  font-size: 14px;
  line-height: 1.7;
  white-space: pre-wrap;
  margin-bottom: 24px;
}

.result-citations h4 {
  margin: 0 0 12px 0;
  font-size: 13px;
  color: var(--c-text-secondary);
}

.citation-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.cite-tag {
  display: flex;
  align-items: center;
  gap: 4px;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--c-text-secondary);
  text-align: center;
  gap: 16px;
}
.empty-icon {
  font-size: 48px;
  opacity: 0.5;
}
.empty-state p {
  font-size: 13px;
  line-height: 1.8;
  max-width: 80%;
}
</style>
