<script setup>
import { ref, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage } from 'element-plus'
import { Setting, Calendar, Upload, Refresh } from '../../../shared/icons'
import { THEME_OPTIONS, applyThemePreference } from '../../../shared/theme'
import WorkspaceSyncSettings from './WorkspaceSyncSettings.vue'

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

async function openCaseFolder() {
  const result = await casyContext.files.openDefault(settingsStore.caseFolderBase)
  if (!result.ok) ElMessage.error(result.error || '打开目录失败')
}

function handleThemeChange(theme) {
  settingsStore.theme = applyThemePreference(theme)
}

onMounted(() => {
  loadHolidaysSummary()
})
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header>
        <div class="section-title">
          <el-icon><Setting /></el-icon>
          <span>通用设置</span>
        </div>
      </template>

      <el-form label-position="top" size="default">
        <el-form-item label="案件文件夹路径">
          <div class="folder-input">
            <el-input v-model="settingsStore.caseFolderBase" placeholder="默认: ~/Documents/Casy/cases" readonly />
            <el-button :disabled="!settingsStore.caseFolderBase" @click="openCaseFolder">打开</el-button>
          </div>
          <span class="field-hint">案件文件将存储在此目录下</span>
        </el-form-item>

        <el-form-item label="主题风格" class="theme-form-item">
          <div class="theme-grid" role="radiogroup" aria-label="主题风格">
            <button
              v-for="option in THEME_OPTIONS"
              :key="option.value"
              type="button"
              class="theme-option"
              :class="{ active: settingsStore.theme === option.value }"
              role="radio"
              :aria-checked="settingsStore.theme === option.value"
              @click="handleThemeChange(option.value)"
            >
              <span class="theme-swatches" aria-hidden="true">
                <span
                  v-for="swatch in option.swatches"
                  :key="swatch"
                  class="theme-swatch"
                  :style="{ background: swatch }"
                />
              </span>
              <span class="theme-copy">
                <strong>{{ option.label }}</strong>
                <small>{{ option.description }}</small>
              </span>
              <span class="theme-check" aria-hidden="true">✓</span>
            </button>
          </div>
        </el-form-item>

        <el-form-item label="文档主题">
          <el-radio-group v-model="settingsStore.document_theme"><el-radio-button value="legal">法律 / 专利</el-radio-button><el-radio-button value="standard">标准</el-radio-button></el-radio-group>
        </el-form-item>
        <el-form-item label="引用来源">
          <div class="quote-source-settings"><el-input v-for="(_, index) in settingsStore.quote_sources" :key="index" v-model="settingsStore.quote_sources[index]" :aria-label="`第 ${index + 1} 层引用来源`" :placeholder="`来源 ${index + 1}`" maxlength="24" /></div>
        </el-form-item>

        <el-form-item label="语言">
          <el-select v-model="settingsStore.language">
            <el-option label="简体中文" value="zh-CN" />
            <el-option label="English" value="en-US" />
          </el-select>
        </el-form-item>

        <WorkspaceSyncSettings />
        <el-form-item>
          <el-button type="primary" :loading="generalSaving" @click="saveGeneralSettings">保存设置</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 节假日配置 -->
    <el-card style="margin-top: 16px">
      <template #header>
        <div class="section-title">
          <el-icon><Calendar /></el-icon>
          <span>节假日日历</span>
        </div>
      </template>

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
          <el-icon style="margin-right: 4px"><Upload /></el-icon> 导入节假日 JSON
        </el-button>
        <el-button @click="loadHolidaysSummary" :loading="holidaysLoading">
          <el-icon style="margin-right: 4px"><Refresh /></el-icon> 刷新
        </el-button>
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
  padding: 0;
}

.tip {
  color: var(--gray-400);
  font-size: 13px;
  margin-bottom: 16px;
}

.field-hint {
  color: var(--c-text-secondary);
  font-size: 12px;
  margin-left: 8px;
}

.theme-form-item :deep(.el-form-item__content) {
  width: min(100%, 820px);
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  width: 100%;
}

.theme-option {
  min-width: 0;
  min-height: 76px;
  padding: 12px;
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) 20px;
  align-items: center;
  gap: 12px;
  text-align: left;
  font: inherit;
  color: var(--c-text);
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  transition: border-color var(--motion-fast) var(--ease-out), background var(--motion-fast) var(--ease-out), transform var(--motion-fast) var(--ease-out);
}

.theme-option:hover {
  border-color: var(--c-border-strong);
  background: var(--c-bg-hover);
}

.theme-option:active {
  transform: translateY(1px);
}

.theme-option.active {
  border-color: var(--c-primary);
  background: var(--c-primary-light);
  box-shadow: 0 0 0 1px var(--c-primary);
}

.theme-swatches {
  height: 42px;
  display: flex;
  overflow: hidden;
  border-radius: var(--c-radius);
  border: 1px solid color-mix(in srgb, var(--c-border) 80%, transparent);
}

.theme-swatch {
  flex: 1;
}

.theme-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.theme-copy strong {
  color: var(--c-text-heading);
  font-size: 13px;
  line-height: 18px;
}

.theme-copy small {
  color: var(--c-text-secondary);
  font-size: 11.5px;
  line-height: 16px;
}

.theme-check {
  color: var(--c-primary);
  font-weight: 700;
  opacity: 0;
}

.theme-option.active .theme-check {
  opacity: 1;
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
  background: var(--c-bg-subtle);
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
  color: var(--c-text-heading);
}

@media (max-width: 900px) {
  .theme-grid {
    grid-template-columns: 1fr;
  }
}
</style>
