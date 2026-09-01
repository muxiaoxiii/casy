<script setup>
import { ref, reactive } from 'vue'
import {
  Document,
  User,
  Location,
  Money,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

const props = defineProps({
  modelValue: Boolean
})

const emit = defineEmits(['update:modelValue', 'create'])

const saving = ref(false)
const activeStep = ref('basic')

const formData = reactive({
  caseName: '',
  caseNo: '',
  track: 'patent_invalidation',
  caseLevel: '',
  clientName: '',
  ourRole: '原告',
  opponentName: '',
  opponentRole: '被告',
  court: '',
  judgePanel: '',
  filingDate: '',
  caseAmount: ''
})

const trackOptions = [
  { value: 'patent_invalidation', label: '专利无效宣告程序' },
  { value: 'civil_tort', label: '民事诉讼程序 (侵权/合同)' },
  { value: 'admin_litigation', label: '行政诉讼程序' },
  { value: 'arbitration', label: '商事仲裁程序' },
  { value: 'other', label: '非诉业务 / 常年顾问 / 其他' },
]

function handleClose() {
  emit('update:modelValue', false)
}

async function handleSave() {
  if (!formData.caseName) {
    ElMessage.warning('请输入案件名称')
    return
  }
  saving.value = true
  
  // 模拟保存延迟
  await new Promise(resolve => setTimeout(resolve, 300))
  
  emit('create', { ...formData })
  saving.value = false
  handleClose()
  
  // 清空表单
  Object.keys(formData).forEach(key => {
    formData[key] = key === 'track' ? 'patent_invalidation' : ''
  })
  activeStep.value = 'basic'
}
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    @update:model-value="$emit('update:modelValue', $event)"
    size="680px"
    class="case-wizard-drawer"
    :with-header="false"
    destroy-on-close
  >
    <div class="wizard-container">
      <div class="wizard-header">
        <h2>创建新案件</h2>
        <p class="subtitle">录入案件核心结构信息，后续可通过 AI 自动解析并补全细节</p>
        <button class="close-btn" @click="handleClose">×</button>
      </div>

      <div class="wizard-layout">
        <!-- 左侧导航 -->
        <div class="wizard-nav">
          <div class="nav-item" :class="{ active: activeStep === 'basic' }" @click="activeStep = 'basic'">
            <el-icon><Document /></el-icon> 基础信息
          </div>
          <div class="nav-item" :class="{ active: activeStep === 'parties' }" @click="activeStep = 'parties'">
            <el-icon><User /></el-icon> 当事各方
          </div>
          <div class="nav-item" :class="{ active: activeStep === 'court' }" @click="activeStep = 'court'">
            <el-icon><Location /></el-icon> 管辖机构
          </div>
          <div class="nav-item" :class="{ active: activeStep === 'finance' }" @click="activeStep = 'finance'">
            <el-icon><Money /></el-icon> 费用与排期
          </div>
        </div>

        <!-- 右侧表单区 -->
        <div class="wizard-content">
          <el-form label-position="top" size="large">
            
            <transition name="el-fade-in-linear" mode="out-in">
              <div v-if="activeStep === 'basic'" key="basic">
                <div class="section-title">基础信息</div>
                <el-form-item label="案件名称 (必填)">
                  <el-input v-model="formData.caseName" placeholder="例如：腾讯诉老干妈合同纠纷案" />
                </el-form-item>
                <el-form-item label="案号">
                  <el-input v-model="formData.caseNo" placeholder="例如：(2023) 粤03民初 1234 号" />
                </el-form-item>
                <el-form-item label="程序类型">
                  <el-select v-model="formData.track" style="width: 100%">
                    <el-option
                      v-for="item in trackOptions"
                      :key="item.value"
                      :label="item.label"
                      :value="item.value"
                    />
                  </el-select>
                </el-form-item>
                <el-form-item label="审理级别">
                  <el-input v-model="formData.caseLevel" placeholder="例如：一审 / 二审 / 仲裁 / 执行" />
                </el-form-item>
              </div>

              <div v-else-if="activeStep === 'parties'" key="parties">
                <div class="section-title">我方当事人</div>
                <el-row :gutter="12">
                  <el-col :span="14">
                    <el-form-item label="名称">
                      <el-input v-model="formData.clientName" placeholder="客户姓名或公司名" />
                    </el-form-item>
                  </el-col>
                  <el-col :span="10">
                    <el-form-item label="诉讼地位">
                      <el-input v-model="formData.ourRole" placeholder="如：原告/上诉人" />
                    </el-form-item>
                  </el-col>
                </el-row>
                
                <el-divider border-style="dashed" />
                
                <div class="section-title">对方当事人</div>
                <el-row :gutter="12">
                  <el-col :span="14">
                    <el-form-item label="名称">
                      <el-input v-model="formData.opponentName" placeholder="对方姓名或公司名" />
                    </el-form-item>
                  </el-col>
                  <el-col :span="10">
                    <el-form-item label="诉讼地位">
                      <el-input v-model="formData.opponentRole" placeholder="如：被告/被上诉人" />
                    </el-form-item>
                  </el-col>
                </el-row>
              </div>

              <div v-else-if="activeStep === 'court'" key="court">
                <div class="section-title">管辖机构</div>
                <el-form-item label="受理法院/仲裁委">
                  <el-input v-model="formData.court" placeholder="例如：深圳市南山区人民法院" />
                </el-form-item>
                <el-form-item label="审判长/合议庭">
                  <el-input v-model="formData.judgePanel" placeholder="法官姓名或团队" />
                </el-form-item>
              </div>

              <div v-else-if="activeStep === 'finance'" key="finance">
                <div class="section-title">费用与排期</div>
                <el-form-item label="标的额 (元)">
                  <el-input v-model="formData.caseAmount" placeholder="请输入数字" type="number" />
                </el-form-item>
                <el-form-item label="立案日期">
                  <el-date-picker v-model="formData.filingDate" type="date" placeholder="选择日期" style="width: 100%" value-format="YYYY-MM-DD" />
                </el-form-item>
                <el-alert title="其余节点排期将在案件详情页时间线中管理" type="info" show-icon :closable="false" />
              </div>
            </transition>

          </el-form>
        </div>
      </div>

      <div class="wizard-footer">
        <el-button @click="handleClose">取消</el-button>
        <el-button type="primary" :loading="saving" @click="handleSave">创建案件</el-button>
      </div>
    </div>
  </el-drawer>
</template>

<style scoped>
.wizard-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--c-bg);
}

.wizard-header {
  position: relative;
  padding: 24px 32px 20px;
  background: var(--c-bg-soft);
  border-bottom: 1px solid var(--c-border);
}
.wizard-header h2 {
  margin: 0 0 4px;
  font-size: 20px;
  font-weight: 600;
  color: var(--c-text);
}
.wizard-header .subtitle {
  margin: 0;
  font-size: 13px;
  color: var(--c-text-secondary);
}
.close-btn {
  position: absolute;
  top: 24px;
  right: 24px;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  border: none;
  background: var(--c-bg-mute);
  color: var(--c-text-secondary);
  font-size: 20px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}
.close-btn:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.wizard-layout {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.wizard-nav {
  width: 180px;
  background: var(--c-bg-mute);
  padding: 24px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-right: 1px solid var(--c-border);
}
.nav-item {
  padding: 12px 14px;
  border-radius: 8px;
  font-size: 14.5px;
  color: var(--c-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 10px;
  transition: all 0.2s;
}
.nav-item:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}
.nav-item.active {
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 500;
}

.wizard-content {
  flex: 1;
  padding: 32px 40px;
  overflow-y: auto;
}
.section-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--c-text);
  margin-bottom: 20px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.section-title::before {
  content: '';
  width: 4px;
  height: 14px;
  background: var(--c-primary);
  border-radius: 2px;
}

.wizard-footer {
  padding: 16px 32px;
  background: var(--c-bg-soft);
  border-top: 1px solid var(--c-border);
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

:deep(.el-drawer__body) {
  padding: 0 !important;
}

:deep(.el-form-item__label) {
  font-weight: 500;
  color: var(--c-text);
  margin-bottom: 6px !important;
}
:deep(.el-input__wrapper) {
  box-shadow: 0 0 0 1px var(--c-border) inset;
  background: var(--c-bg);
  border-radius: 8px;
}
:deep(.el-input__wrapper.is-focus) {
  box-shadow: 0 0 0 1px var(--c-primary) inset;
}
</style>
