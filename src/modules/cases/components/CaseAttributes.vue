<script setup>
import { ref, watch } from 'vue'
import { Edit, Check, Close } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'
import { intakeSections, routeOptions, statusGroups, setIntakeRoute } from './caseIntake'
import { normalizeCaseAttorneys } from '../../../core/caseNormalize'
import ThirdPartiesEditor from './ThirdPartiesEditor.vue'
const props = defineProps({ caseData: { type:Object, required:true }, initiallyEditing:Boolean })
const emit = defineEmits(['saved'])
const editing = ref(props.initiallyEditing), saving = ref(false), error = ref(''), form = ref({}), thirdParties = ref([])
function reset() {
  form.value = {...props.caseData, attorneys:normalizeCaseAttorneys(props.caseData.attorneys)}
  try { thirdParties.value = JSON.parse(props.caseData.thirdParties || '[]') } catch { thirdParties.value = [] }
}
watch(() => props.caseData, reset,{immediate:true})
async function save() {
  if (saving.value) return
  error.value = ''
  if (!form.value.caseName?.trim() || thirdParties.value.some(p=>!p.name.trim())) { error.value = '案件名称和第三人名称不能为空'; return }
  saving.value = true
  try {
    const payload = {...form.value, thirdParties:JSON.stringify(thirdParties.value)}
    payload.caseLevel ||= null
    payload.procedureType ||= null
    for (const group of statusGroups) if (!(payload.caseRoute === '三轨并行' || payload.caseRoute?.includes(group.route))) payload[group.key] = null
    const result = await casyContext.cases.update(props.caseData.id,payload)
    if (result.ok) { editing.value = false; emit('saved') } else error.value = result.error
  } finally { saving.value = false }
}
</script>
<template>
  <section class="attributes">
    <div class="heading"><h3>案件属性</h3><el-button v-if="!editing" :icon="Edit" @click="reset(); editing = true">编辑</el-button><div v-else><el-button :icon="Close" :disabled="saving" @click="editing = false; reset()">取消</el-button><el-button type="primary" :icon="Check" :loading="saving" @click="save">保存</el-button></div></div>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <el-form label-position="top" :disabled="saving">
      <el-form-item label="案件程序"><el-select v-if="editing" :model-value="form.caseRoute" @update:model-value="setIntakeRoute(form,$event)"><el-option v-for="[value,label] in routeOptions" :key="value" :value="value" :label="label" /></el-select><div v-else>{{ caseData.caseRoute }}</div></el-form-item>
      <div class="fields"><el-form-item v-for="group in statusGroups.filter(g => form.caseRoute === '三轨并行' || form.caseRoute?.includes(g.route))" :key="group.key" :label="group.label"><el-select v-if="editing" v-model="form[group.key]" clearable><el-option v-for="[value,label] in group.options" :key="value" :value="value" :label="label" /></el-select><span v-else>{{ group.options.find(([v]) => v === form[group.key])?.[1] || '未填写' }}</span></el-form-item></div>
      <section v-for="section in intakeSections" :key="section.key" class="field-section">
        <h4>{{ section.label }}</h4>
        <div class="fields">
          <el-form-item v-for="field in section.fields" :key="field.key" :label="field.label" :class="{wide:field.type === 'textarea'}">
            <template v-if="editing">
              <el-date-picker v-if="['date','datetime'].includes(field.type)" v-model="form[field.key]" :type="field.type" :value-format="field.type === 'datetime' ? 'YYYY-MM-DD HH:mm:ss' : 'YYYY-MM-DD'" />
              <el-select v-else-if="['select','enum','tags'].includes(field.type)" v-model="form[field.key]" filterable :allow-create="field.type !== 'enum'" :multiple="field.type === 'tags'" clearable><el-option v-for="option in field.options" :key="option" :value="option" /></el-select>
              <el-input v-else v-model="form[field.key]" :type="field.type === 'textarea' ? 'textarea' : 'text'" :rows="3" />
            </template>
            <div v-else class="value">{{ Array.isArray(form[field.key]) ? form[field.key].join('、') : form[field.key] || '未填写' }}</div>
          </el-form-item>
        </div>
        <template v-if="section.key === 'parties'">
          <ThirdPartiesEditor v-if="editing" v-model="thirdParties" />
          <div v-else><h4>第三人</h4><p v-if="!thirdParties.length">未填写</p><div v-for="(party,index) in thirdParties" :key="index" class="third-party"><strong>{{ party.name }}</strong><span>{{ [party.role,party.agent,party.firm,party.contact].filter(Boolean).join(' · ') }}</span></div></div>
        </template>
      </section>
    </el-form>
  </section>
</template>
<style scoped>
.heading { display:flex; justify-content:space-between; align-items:center; gap:16px; }
h3 { font-size:17px; } h4 { font-size:14px; margin:0 0 20px; }
.field-section { border-top:1px solid var(--c-border); padding:24px 0; }
.fields { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:0 24px; }
.wide { grid-column:1 / -1; }
.value { white-space:pre-wrap; overflow-wrap:anywhere; min-height:24px; }
.third-party { display:flex; flex-direction:column; gap:8px; margin:12px 0; }
:deep(.el-date-editor) { width:100%; }
@media(max-width:520px) { .fields { grid-template-columns:minmax(0,1fr); } }
</style>
