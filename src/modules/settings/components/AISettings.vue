<script setup>
import { onMounted } from 'vue'
import { useAiSettingsStore } from '../../../stores/aiSettings'
import { AI_PROMPTS } from '../../../core/prompts'

const aiStore = useAiSettingsStore()

onMounted(() => {
  aiStore.load()
})
</script>

<template>
  <div class="ai-settings">
    <h4>AI 大模型配置 (Copilot & 推荐系统)</h4>
    <p style="font-size: 12px; color: #666; margin-bottom: 16px;">
      设置您的大模型服务商。推荐使用 DeepSeek 或 OpenAI。系统会自动利用该配置来驱动收件箱的意图识别、早报总结以及文书的 AI 辅助撰写。
    </p>
    
    <el-form label-position="top" size="default">
      <el-form-item label="服务商 (Provider)">
        <el-select v-model="aiStore.provider" style="width: 100%">
          <el-option label="DeepSeek" value="deepseek" />
          <el-option label="OpenAI" value="openai" />
          <el-option label="Local (Ollama)" value="local" />
        </el-select>
      </el-form-item>

      <el-form-item label="API Base URL">
        <el-input v-model="aiStore.baseUrl" placeholder="https://api.deepseek.com/v1" />
      </el-form-item>

      <el-form-item label="API Key" v-if="aiStore.provider !== 'local'">
        <el-input v-model="aiStore.apiKey" type="password" show-password placeholder="sk-..." />
      </el-form-item>

      <el-form-item label="Model">
        <el-input v-model="aiStore.model" placeholder="deepseek-chat" />
      </el-form-item>

      <el-form-item label="System Prompt (系统人设设定)">
        <el-input 
          v-model="aiStore.systemPrompt" 
          type="textarea" 
          :rows="5"
        />
        <div style="margin-top: 8px; font-size: 12px; color: #999;">
          若需重置系统人设，可点击 <a href="javascript:void(0)" @click="aiStore.systemPrompt = AI_PROMPTS.SYSTEM_DEFAULT">恢复默认</a>。
        </div>
      </el-form-item>
    </el-form>
  </div>
</template>

<style scoped>
.ai-settings {
  padding: 8px 0;
  max-width: 600px;
}
h4 {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--c-text-heading);
}
</style>
