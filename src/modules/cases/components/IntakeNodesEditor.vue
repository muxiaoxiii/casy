<script setup>
import { ref } from 'vue'
import { Plus, Delete } from '../../../shared/icons'
defineProps({ form: { type: Object, required: true } })
const tab = ref('hearings')
const groups = [
  { key: 'hearings', label: '庭审', fields: [
    ['hearingName','庭审名称','text',true],['hearingDate','开庭时间','datetime',true],['court','审理机关'],
    ['venue','开庭地点'],['attendees','出庭人员'],['judges','审判人员'],['caseLevel','审级','select',['一审','二审','再审']],
    ['contactInfo','联系方式'],['actualStatus','实际开庭情况','select',['已开','未开']],
  ], defaults: { hearingName: '开庭 / 口审', actualStatus: '未开' } },
  { key: 'logs', label: '办案日志', fields: [
    ['eventSummary','事件概述','text',true],['eventName','事件名称'],['eventDate','发生时间','datetime',true],
    ['eventType','类型','select',[['record','记录'],['submitted','交文'],['received','收文'],['task','任务']]],
    ['content','操作内容','textarea'],
  ], defaults: { eventType: 'record' } },
  { key: 'tasks', label: '任务', fields: [
    ['taskName','任务名称','text',true],['description','任务详细描述','textarea'],['deadline','截止日期','date'],
    ['assignee','任务执行人'],['priority','优先级','select',[['normal','普通'],['important','重要'],['urgent','紧急'],['urgent_important','重要且紧急']]],
    ['completed','完成状态','checkbox'],['finishNote','完结记录','textarea'],
  ], defaults: { priority: 'normal', completed: false } },
  { key: 'officials', label: '官方联系人', fields: [
    ['name','姓名','text',true],['role','身份','select',['法官','法官助理','书记员','法院']],['court','所属机关'],
    ['contactDetail','具体联系方式'],['contactText','联系方式备注'],['contactRecord','联系记录','textarea'],
  ], defaults: { role: '法官' } },
]
function add(form, group) { form[group.key].push({ ...Object.fromEntries(group.fields.map(([key]) => [key,''])), ...group.defaults }) }
</script>
<template>
  <el-tabs v-model="tab">
    <el-tab-pane v-for="group in groups" :key="group.key" :name="group.key" :label="group.label + (form[group.key].length ? ' (' + form[group.key].length + ')' : '')">
      <el-button :icon="Plus" @click="add(form, group)">添加{{ group.label }}</el-button>
      <div v-for="(row,index) in form[group.key]" :key="index" class="node-row">
        <div class="node-heading"><strong>{{ group.label }} {{ index + 1 }}</strong><el-button :icon="Delete" :aria-label="'删除' + group.label" :title="'删除' + group.label" @click="form[group.key].splice(index,1)" /></div>
        <div class="node-fields">
          <el-form-item v-for="[key,label,type,options] in group.fields" :key="key" :label="label" :required="options === true" :class="{ wide: type === 'textarea' }">
            <el-date-picker v-if="type === 'date' || type === 'datetime'" v-model="row[key]" :type="type" :value-format="type === 'date' ? 'YYYY-MM-DD' : 'YYYY-MM-DD HH:mm:ss'" />
            <el-select v-else-if="type === 'select'" v-model="row[key]" clearable><el-option v-for="option in options" :key="Array.isArray(option) ? option[0] : option" :value="Array.isArray(option) ? option[0] : option" :label="Array.isArray(option) ? option[1] : option" /></el-select>
            <el-checkbox v-else-if="type === 'checkbox'" v-model="row[key]">已完成</el-checkbox>
            <el-input v-else v-model="row[key]" :type="type === 'textarea' ? 'textarea' : 'text'" :rows="3" />
          </el-form-item>
        </div>
      </div>
    </el-tab-pane>
  </el-tabs>
</template>
<style scoped>
.node-row { padding-top: 20px; margin-top: 20px; border-top: 1px solid var(--c-border); }
.node-heading { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.node-fields { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 0 16px; }
.wide { grid-column: 1 / -1; }
:deep(.el-date-editor) { width: 100%; }
@media(max-width: 480px) { .node-fields { grid-template-columns: minmax(0,1fr); } }
</style>
