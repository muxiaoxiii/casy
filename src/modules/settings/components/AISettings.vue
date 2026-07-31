<script setup>
import { ref, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage } from 'element-plus'

const aiMode = ref('noop')
const aiApiUrl = ref('')
const aiApiKey = ref('')
const aiModel = ref('')
const aiDailyLimit = ref(50)
const aiTesting = ref(false)
const aiConnectionStatus = ref(null)
const aiSaving = ref(false)

const ollamaModels = ['qwen2.5:7b', 'qwen2.5:14b', 'llama3.1:8b', 'llama3.1:70b', 'deepseek-coder:7b']
const openaiModels = ['gpt-4o-mini', 'gpt-4o', 'gpt-3.5-turbo', 'deepseek-chat', 'claude-3-haiku-20240307']

async function loadAiConfig() {
  const result = await tauriCallSafe('get_ai_config')
  if (result.ok) {
    aiMode.value = result.data.mode || 'noop'
    aiApiUrl.value = result.data.apiUrl || ''
    aiApiKey.value = result.data.apiKey || ''
    aiModel.value = result.data.model || ''
    aiDailyLimit.value = result.data.dailyLimit ?? 50
  }
}

async function saveAiConfig() {
  aiSaving.value = true
  const result = await tauriCallSafe('configure_ai', {
    mode: aiMode.value,
    apiUrl: aiApiUrl.value || null,
    apiKey: aiApiKey.value || null,
    model: aiModel.value || null,
    dailyLimit: aiDailyLimit.value,
  })
  aiSaving.value = false

  if (result.ok) {
    ElMessage.success('AI 配置已保存')
    aiConnectionStatus.value = null
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function testAiConnection() {
  aiTesting.value = true
  aiConnectionStatus.value = null
  const result = await tauriCallSafe('test_ai_connection')
  aiTesting.value = false

  if (result.ok) {
    aiConnectionStatus.value = 'ok'
    ElMessage.success(result.data)
  } else {
    aiConnectionStatus.value = 'fail'
    ElMessage.error(result.error || '连接失败')
  }
}

onMounted(() => {
  loadAiConfig()
})
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>🤖 AI 后端配置</strong>
          <el-tag v-if="aiMode !== 'noop'" type="success" size="small">已启用</el-tag>
          <el-tag v-else type="info" size="small">规则匹配模式</el-tag>
        </div>
      </template>

      <p class="tip">配置 AI 后端以启用智能文档分类、信息提取和摘要功能。不配置时使用规则匹配。</p>

      <el-form label-width="120px" size="default">
        <el-form-item label="AI 模式">
          <el-radio-group v-model="aiMode">
            <el-radio value="noop">规则匹配（离线）</el-radio>
            <el-radio value="ollama">Ollama（本地）</el-radio>
            <el-radio value="openai">OpenAI 兼容 API</el-radio>
          </el-radio-group>
        </el-form-item>

        <template v-if="aiMode === 'ollama'">
          <el-form-item label="Ollama 地址">
            <el-input v-model="aiApiUrl" placeholder="http://localhost:11434" />
          </el-form-item>
          <el-form-item label="模型">
            <el-select v-model="aiModel" filterable allow-create placeholder="选择或输入模型名称">
              <el-option v-for="m in ollamaModels" :key="m" :label="m" :value="m" />
            </el-select>
          </el-form-item>
        </template>

        <template v-if="aiMode === 'openai'">
          <el-form-item label="API 地址">
            <el-input v-model="aiApiUrl" placeholder="https://api.openai.com/v1" />
          </el-form-item>
          <el-form-item label="API Key">
            <el-input v-model="aiApiKey" placeholder="sk-..." type="password" show-password />
          </el-form-item>
          <el-form-item label="模型">
            <el-select v-model="aiModel" filterable allow-create placeholder="选择或输入模型名称">
              <el-option v-for="m in openaiModels" :key="m" :label="m" :value="m" />
            </el-select>
          </el-form-item>
        </template>

        <el-form-item label="每日调用限额">
          <el-input-number v-model="aiDailyLimit" :min="0" :max="10000" :step="10" />
          <span class="field-hint">设为 0 表示不限制</span>
        </el-form-item>

        <el-form-item>
          <el-button type="primary" :loading="aiSaving" @click="saveAiConfig">保存配置</el-button>
          <el-button
            v-if="aiMode !== 'noop'"
            :loading="aiTesting"
            @click="testAiConnection"
            :type="aiConnectionStatus === 'ok' ? 'success' : aiConnectionStatus === 'fail' ? 'danger' : 'default'"
          >
            {{ aiConnectionStatus === 'ok' ? '✓ 连接正常' : aiConnectionStatus === 'fail' ? '✗ 连接失败' : '测试连接' }}
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<style scoped>
.tab-content {
  padding: 0 16px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.tip {
  color: #909399;
  font-size: 13px;
  margin-bottom: 16px;
}

.field-hint {
  color: #909399;
  font-size: 12px;
  margin-left: 8px;
}
</style>
