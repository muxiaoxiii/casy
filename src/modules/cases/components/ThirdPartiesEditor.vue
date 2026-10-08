<script setup>
import { Plus, Delete } from '../../../shared/icons'
import { roleOptions } from './caseIntake'
defineProps({ modelValue: { type: Array, default: () => [] } })
const emit = defineEmits(['update:modelValue'])
</script>
<template>
  <section class="third-parties">
    <div class="ui-row ui-row--between party-heading" style="gap:12px"><h3>第三人</h3><el-button :icon="Plus" @click="emit('update:modelValue', [...modelValue, { name: '', role: '第三人', agent: '', firm: '', contact: '' }])">添加第三人</el-button></div>
    <div v-for="(party, index) in modelValue" :key="index" class="ui-grid party-row">
      <el-form-item :label="'第三人 ' + (index + 1) + ' 名称'" required><el-input v-model="party.name" /></el-form-item>
      <el-form-item label="诉讼地位"><el-select v-model="party.role" filterable allow-create><el-option v-for="role in roleOptions" :key="role" :value="role" /></el-select></el-form-item>
      <el-form-item label="代理人"><el-input v-model="party.agent" /></el-form-item>
      <el-form-item label="代理律所"><el-input v-model="party.firm" /></el-form-item>
      <el-form-item label="联系方式"><el-input v-model="party.contact" /></el-form-item>
      <el-button :icon="Delete" aria-label="删除第三人" title="删除第三人" @click="emit('update:modelValue', modelValue.filter((_, i) => i !== index))" />
    </div>
  </section>
</template>
<style scoped>
.party-heading { margin-bottom: 16px; }
h3 { font-size: 15px; margin: 0; }
.party-row { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0 16px; border-top: 1px solid var(--c-border); padding-top: 16px; }
.party-row > .el-button { justify-self: end; align-self: center; }

</style>
