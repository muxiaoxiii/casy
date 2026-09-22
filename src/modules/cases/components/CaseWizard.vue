<script setup>
import { ref, reactive, computed, watch } from 'vue'
import { Close, Check, Back, Right, Plus, Delete, MagicStick } from '../../../shared/icons'
import { ElMessage, ElMessageBox } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import { intakeSections, routeOptions, statusGroups, newIntake, setIntakeRoute, intakePayload, parseIntakeText } from './caseIntake'
import ThirdPartiesEditor from './ThirdPartiesEditor.vue'
import IntakeNodesEditor from './IntakeNodesEditor.vue'

const props = defineProps({ modelValue: Boolean, submit: { type: Function, required: true }, initialCase: Object, title: String })
const emit = defineEmits(['update:modelValue'])
const formData = reactive(newIntake())
const saving = ref(false)
const error = ref('')
const activeStep = ref('basic')
const sourceText = ref('')
const baseline = ref('')
const suggestions = ref([])
const relatedOptions = ref([])
const searching = ref(false)
const relatedId = ref('')
const relationType = ref('cross_reference')
const sections = [...intakeSections, { key:'nodes', label:'办案节点' }, { key:'relations', label:'关联案件' }]
const currentIndex = computed(() => sections.findIndex(s => s.key === activeStep.value))
const section = computed(() => sections[currentIndex.value])
const relationTypes = [['cross_reference','关联案件'],['same_patent','同一专利'],['same_party','共同当事人'],['appeal_of','本案为关联案的二审 / 再审']]
let searchVersion = 0

watch(() => props.modelValue, async visible => {
  if (!visible) return
  Object.assign(formData, newIntake(), JSON.parse(JSON.stringify(props.initialCase || {})))
  baseline.value = JSON.stringify(formData)
  activeStep.value = 'basic'
  sourceText.value = ''
  error.value = ''
  relatedId.value = ''
  relatedOptions.value = []
  relationType.value = 'cross_reference'
  searchVersion += 1
  searching.value = false
  const result = await casyContext.cases.list({ perPage: 200 })
  if (result.ok) suggestions.value = result.data.items
}, { immediate: true })
function suggest(key, query, callback) {
  const values = [...new Set(suggestions.value.map(c => c[key]).filter(v => typeof v === 'string' && v))]
  callback(values.filter(v => v.toLowerCase().includes(query.toLowerCase())).slice(0,15).map(value => ({value})))
}
async function searchRelated(query) {
  const version = ++searchVersion
  searching.value = true
  try {
    const result = query.trim() ? await casyContext.cases.search(query.trim()) : await casyContext.cases.list({perPage:50})
    if (version === searchVersion) relatedOptions.value = result.ok ? (Array.isArray(result.data) ? result.data : result.data.items) : []
  } finally { if (version === searchVersion) searching.value = false }
}
function addRelated() {
  const selected = relatedOptions.value.find(c => c.id === relatedId.value)
  if (!selected) return
  if (formData.relatedCases.some(r => r.caseId === selected.id && r.relationType === relationType.value)) return
  formData.relatedCases.push({ caseId:selected.id, relationType:relationType.value, label:selected.caseName })
  relatedId.value = ''
}
async function close(done) {
  if (saving.value) return
  if (JSON.stringify(formData) !== baseline.value) {
    try { await ElMessageBox.confirm('关闭后，本次尚未保存的案件信息将被丢弃。','放弃录入？',{confirmButtonText:'放弃',cancelButtonText:'继续录入',type:'warning'}) } catch { return }
  }
  emit('update:modelValue',false)
  if (typeof done === 'function') done()
}
function fillFromText() {
  const parsed = parseIntakeText(sourceText.value)
  for (const [key,value] of Object.entries(parsed.fields)) {
    if (!formData[key] || formData[key] === newIntake()[key] || (Array.isArray(formData[key]) && !formData[key].length)) formData[key] = key === 'attorneys' && typeof value === 'string' ? value.split(/[、,，;；]/).map(v=>v.trim()).filter(Boolean) : value
  }
  if (parsed.unmatched.length) formData.notes = [formData.notes,parsed.unmatched.join('\n')].filter(Boolean).join('\n')
  ElMessage.success(`已识别 ${Object.keys(parsed.fields).length} 项`)
}
async function save() {
  if (saving.value) return
  error.value = ''
  let payload
  try { payload = intakePayload(formData) } catch (e) { error.value = e.message; return }
  saving.value = true
  try {
    const result = await props.submit(payload)
    if (!result?.ok) { error.value = result?.error || '保存失败，请重试'; return }
    baseline.value = JSON.stringify(formData)
    emit('update:modelValue',false)
  } catch (e) { error.value = e.message || String(e) }
  finally { saving.value = false }
}
</script>
<template>
  <el-drawer :model-value="modelValue" :before-close="close" :close-on-press-escape="!saving" :close-on-click-modal="false" size="min(860px, 100vw)" class="case-wizard-drawer" :with-header="false" destroy-on-close>
    <div class="intake">
      <header><div><h2>{{ title || (initialCase ? '新建关联案件' : '新建案件') }}</h2><span class="case-name">{{ formData.caseName || '未命名案件' }}</span></div><el-button :icon="Close" :disabled="saving" aria-label="关闭" title="关闭" @click="close" /></header>
      <nav aria-label="案件录入步骤"><button v-for="item in sections" :key="item.key" type="button" :aria-current="activeStep === item.key ? 'step' : undefined" :class="{active:activeStep === item.key}" @click="activeStep = item.key">{{ item.label }}</button></nav>
      <div class="intake-body">
        <el-form label-position="top" :disabled="saving" @submit.prevent="save">
          <template v-if="activeStep === 'basic'">
            <el-collapse><el-collapse-item title="从案件资料提取" name="paste"><el-input v-model="sourceText" type="textarea" :rows="4" placeholder="案件名称：&#10;案号：&#10;客户名称：&#10;第三人：" /><el-button :icon="MagicStick" :disabled="!sourceText.trim()" class="extract" @click="fillFromText">提取信息</el-button></el-collapse-item></el-collapse>
            <el-form-item label="案件程序" class="route-field"><el-select :model-value="formData.caseRoute" @update:model-value="setIntakeRoute(formData, $event)"><el-option v-for="[value,label] in routeOptions" :key="value" :label="label" :value="value" /></el-select></el-form-item>
            <div class="fields"><el-form-item v-for="group in statusGroups.filter(g => formData.caseRoute === '三轨并行' || formData.caseRoute?.includes(g.route))" :key="group.key" :label="group.label"><el-select v-model="formData[group.key]"><el-option v-for="[value,label] in group.options" :key="value" :value="value" :label="label" /></el-select></el-form-item></div>
          </template>
          <div v-if="section.fields" class="fields">
            <el-form-item v-for="field in section.fields" :key="field.key" :label="field.label" :required="field.key === 'caseName'" :class="{wide:field.type === 'textarea' || field.key === 'caseName'}">
              <el-date-picker v-if="['date','datetime'].includes(field.type)" v-model="formData[field.key]" :type="field.type" :value-format="field.type === 'datetime' ? 'YYYY-MM-DD HH:mm:ss' : 'YYYY-MM-DD'" clearable />
              <el-select v-else-if="['select','enum','tags'].includes(field.type)" v-model="formData[field.key]" filterable :allow-create="field.type !== 'enum'" :multiple="field.type === 'tags'" default-first-option clearable><el-option v-for="option in field.options" :key="option" :value="option" /></el-select>
              <el-autocomplete v-else-if="field.type === 'suggest'" v-model="formData[field.key]" :fetch-suggestions="(q, cb) => suggest(field.key,q,cb)" clearable />
              <el-input v-else v-model="formData[field.key]" :type="field.type === 'textarea' ? 'textarea' : 'text'" :inputmode="field.type === 'amount' ? 'decimal' : 'text'" :rows="3" />
            </el-form-item>
          </div>
          <ThirdPartiesEditor v-if="activeStep === 'parties'" v-model="formData.thirdParties" />
          <IntakeNodesEditor v-if="activeStep === 'nodes'" :form="formData" />
          <template v-if="activeStep === 'relations'">
            <el-form-item label="关联案件"><el-select v-model="relatedId" filterable remote :remote-method="searchRelated" :loading="searching" @visible-change="$event && searchRelated('')" placeholder="案件名称或案号"><el-option v-for="item in relatedOptions" :key="item.id" :value="item.id" :label="[item.caseName,item.caseNo].filter(Boolean).join(' · ')" /></el-select></el-form-item>
            <el-form-item label="关联类型"><el-select v-model="relationType"><el-option v-for="[value,label] in relationTypes" :key="value" :value="value" :label="label" /></el-select></el-form-item>
            <el-button :icon="Plus" :disabled="!relatedId" @click="addRelated">添加关联</el-button>
            <div v-for="(relation,index) in formData.relatedCases" :key="index" class="relation-row"><div><strong>{{ relation.label }}</strong><p>{{ relationTypes.find(r => r[0] === relation.relationType)?.[1] }}</p></div><el-button :icon="Delete" aria-label="移除关联" title="移除关联" @click="formData.relatedCases.splice(index,1)" /></div>
          </template>
        </el-form>
      </div>
      <footer>
        <el-alert v-if="error" :title="error" type="error" :closable="false" show-icon />
        <div class="footer-actions"><el-button :disabled="saving" @click="close">取消</el-button><span class="spacer" /><el-button v-if="currentIndex > 0" :icon="Back" :disabled="saving" @click="activeStep = sections[currentIndex - 1].key">上一项</el-button><el-button v-if="currentIndex < sections.length - 1" :icon="Right" :disabled="saving" @click="activeStep = sections[currentIndex + 1].key">下一项</el-button><el-button type="primary" :icon="Check" :loading="saving" @click="save">创建案件</el-button></div>
      </footer>
    </div>
  </el-drawer>
</template>
<style scoped>
.intake { height: 100%; display: flex; flex-direction: column; color: var(--c-text); background: var(--c-bg); }
header { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 22px 24px 16px; border-bottom: 1px solid var(--c-border); }
header > div { min-width: 0; }
h2 { margin: 0 0 6px; font-size: 20px; }
.case-name { display: block; color: var(--c-text-secondary); font-size: 13px; overflow-wrap: anywhere; }
nav { display: flex; flex-wrap: wrap; gap: 4px; padding: 12px 20px; border-bottom: 1px solid var(--c-border); }
nav button { background: transparent; border: 0; border-bottom: 2px solid transparent; padding: 10px 8px; color: var(--c-text-secondary); cursor: pointer; font: inherit; font-size: 13px; }
nav button.active { color: var(--el-color-primary); border-bottom-color: var(--el-color-primary); font-weight: 600; }
.intake-body { flex: 1; min-height: 0; overflow-y: auto; padding: 20px 24px; scroll-padding-block: 24px; }
.intake-body :deep(input), .intake-body :deep(textarea) { scroll-margin-block: 24px; }
.fields { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 0 20px; }
.wide { grid-column: 1 / -1; }
:deep(.el-date-editor), :deep(.el-autocomplete), :deep(.el-select) { width: 100%; min-width: 0; }
.route-field { margin-top: 20px; }
.extract { margin-top: 12px; }
footer { border-top: 1px solid var(--c-border); padding: 14px 24px; background: var(--c-bg-soft); }
.footer-actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.footer-actions .el-button + .el-button { margin-left: 0; }
footer .el-alert { margin-bottom: 12px; }
.spacer { flex: 1; }
.relation-row { display: flex; justify-content: space-between; gap: 16px; align-items: center; border-top: 1px solid var(--c-border); margin-top: 16px; padding-top: 16px; overflow-wrap: anywhere; }
.relation-row p { color: var(--c-text-secondary); margin: 6px 0 0; font-size: 12px; }
@media(max-width: 520px) { .fields { grid-template-columns: minmax(0,1fr); } header,.intake-body,footer { padding-left: 16px; padding-right: 16px; } nav { padding: 8px; } .spacer { display: none; } .footer-actions { justify-content: flex-end; } }
</style>
<style>
.case-wizard-drawer > .el-drawer__body { padding: 0; overflow: hidden; }
</style>
