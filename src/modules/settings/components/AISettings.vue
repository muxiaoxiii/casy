<script setup>
import { computed } from 'vue'
import { useSettingsStore } from '../../../stores/settings'

const store = useSettingsStore()

const modeOptions = [
  { value: 'none', label: '规则匹配（无需 AI）' },
  { value: 'ollama', label: 'Ollama 本地模型' },
  { value: 'openai', label: 'OpenAI API' },
]
</script>

<template>
  <div class="ai-settings">
    <h4>AI 设置</h4>
    <el-form label-width="100px" size="small">
      <el-form-item label="AI 模式">
        <el-select v-model="store.ai_mode" style="width: 100%">
          <el-option v-for="opt in modeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
      </el-form-item>

      <el-form-item v-if="store.ai_mode !== 'none'" label="后端">
        <el-select v-model="store.ai_backend" style="width: 100%">
          <el-option label="Ollama" value="ollama" />
          <el-option label="OpenAI" value="openai" />
        </el-select>
      </el-form-item>

      <el-form-item v-if="store.ai_mode !== 'none'" label="API 地址">
        <el-input v-model="store.ai_api_url" placeholder="http://localhost:11434" />
      </el-form-item>

      <el-form-item v-if="store.ai_backend === 'openai'" label="API Key">
        <el-input v-model="store.ai_api_key" type="password" show-password />
      </el-form-item>

      <el-form-item v-if="store.ai_mode !== 'none'" label="模型">
        <el-input v-model="store.ai_model" placeholder="qwen2.5:14b" />
      </el-form-item>

      <el-form-item label="每日限制">
        <el-input-number v-model="store.ai_daily_limit" :min="0" :max="1000" />
      </el-form-item>
    </el-form>
  </div>
</template>

<style scoped>
.ai-settings {
  padding: 8px 0;
}
h4 {
  margin: 0 0 16px;
  font-size: 14px;
  font-weight: 600;
}
</style>
