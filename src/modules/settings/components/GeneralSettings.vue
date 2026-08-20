<script setup>
import { ref, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage } from 'element-plus'

const settingsStore = useSettingsStore()

const generalSaving = ref(false)

// === 节假日配置 ===
const holidaysLoading = ref(false)
const holidaysSummary = ref(null)
const holidaysImporting = ref(false)

async function loadHolidaysSummary() {
  holidaysLoading.value = true
  const result = await casyContext.settings.holidaysSummary()
  holidaysLoading.value = false
  if (result.ok) {
    holidaysSummary.value = result.data
  }
}

async function importHolidaysJson() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({
    multiple: false,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  })
  if (!selected) return

  holidaysImporting.value = true
  const result = await casyContext.settings.importHolidaysJson(selected)
  holidaysImporting.value = false

  if (result.ok) {
    ElMessage.success(`导入成功：${result.data.holidays_count} 个节假日`)
    await loadHolidaysSummary()
  } else {
    ElMessage.error(result.error || '导入失败')
  }
}

async function saveGeneralSettings() {
  generalSaving.value = true
  const result = await settingsStore.save()
  generalSaving.value = false
  if (result.ok) {
    ElMessage.success('通用设置已保存')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function selectCaseFolder() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ directory: true })
  if (selected) {
    settingsStore.caseFolderBase = selected
  }
}

onMounted(() => {
  loadHolidaysSummary()
})
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header><strong>⚙️ 通用设置</strong></template>

      <el-form label-width="140px" size="default">
        <el-form-item label="案件文件夹路径">
          <div class="folder-input">
            <el-input v-model="settingsStore.caseFolderBase" placeholder="默认: ~/Documents/Casy/cases" readonly />
            <el-button @click="selectCaseFolder">选择</el-button>
          </div>
          <span class="field-hint">案件文件将存储在此目录下</span>
        </el-form-item>

        <el-form-item label="主题">
          <el-radio-group v-model="settingsStore.theme">
            <el-radio value="system">跟随系统</el-radio>
            <el-radio value="light">浅色</el-radio>
            <el-radio value="dark">深色</el-radio>
          </el-radio-group>
        </el-form-item>

        <el-form-item label="语言">
          <el-select v-model="settingsStore.language">
            <el-option label="简体中文" value="zh-CN" />
            <el-option label="English" value="en-US" />
          </el-select>
        </el-form-item>

        <el-form-item>
          <el-button type="primary" :loading="generalSaving" @click="saveGeneralSettings">保存设置</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 节假日配置 -->
    <el-card style="margin-top: 16px">
      <template #header><strong>📅 节假日日历</strong></template>

      <p class="tip">管理中国法定节假日数据，用于期限引擎的工作日顺延计算。支持从 JSON 文件导入自定义节假日数据。</p>

      <div v-if="holidaysSummary" class="holidays-summary">
        <el-descriptions :column="3" border size="small">
          <el-descriptions-item label="节假日天数">{{ holidaysSummary.holidaysCount }}</el-descriptions-item>
          <el-descriptions-item label="调休工作日">{{ holidaysSummary.workdaysCount }}</el-descriptions-item>
          <el-descriptions-item label="覆盖年份">{{ holidaysSummary.yearRange }}</el-descriptions-item>
        </el-descriptions>
      </div>

      <div style="margin-top: 12px; display: flex; gap: 8px;">
        <el-button type="primary" :loading="holidaysImporting" @click="importHolidaysJson">
          📥 导入节假日 JSON
        </el-button>
        <el-button @click="loadHolidaysSummary" :loading="holidaysLoading">刷新</el-button>
      </div>

      <div class="holidays-json-format">
        <h4>JSON 格式说明</h4>
        <pre class="json-example">{
  "holidays": ["2026-01-01", "2026-01-02", "2026-01-03"],
  "workdays": ["2026-01-04"]
}</pre>
        <p class="tip">holidays 为法定假日，workdays 为调休上班日。日期格式 YYYY-MM-DD。</p>
      </div>
    </el-card>
  </div>
</template>

<style scoped>
.tab-content {
  padding: 0 16px;
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

.folder-input {
  display: flex;
  gap: 8px;
  width: 100%;
}

.folder-input .el-input {
  flex: 1;
}

.holidays-summary {
  margin-bottom: 12px;
}

.holidays-json-format {
  margin-top: 16px;
  padding: 12px;
  background: #f5f7fa;
  border-radius: 6px;
}

.json-example {
  background: #282c34;
  color: #abb2bf;
  padding: 12px;
  border-radius: 4px;
  font-size: 12px;
  overflow-x: auto;
  margin: 8px 0;
}

h4 {
  margin: 12px 0 8px;
  font-size: 14px;
  color: #606266;
}
</style>
