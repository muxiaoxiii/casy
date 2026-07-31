<script setup>
import ReferenceSelect from '../../../shared/components/ReferenceSelect.vue'

const props = defineProps({
  form: { type: Object, required: true },
})

const emit = defineEmits(['update'])

const trackOptions = [
  { value: 'patent_invalidation', label: '专利无效' },
  { value: 'admin_litigation', label: '行政诉讼' },
  { value: 'civil_tort', label: '民事侵权' },
  { value: 'other', label: '其他' },
]

const levelOptions = ['一审', '二审', '再审', '结案']
const statusOptions = ['待判决', '待开庭', '待口审', '待无效决定', '待补充意见', '中止', '胜诉', '败诉', '结案', '对方撤案', '撤诉']

function onInput() {
  emit('update')
}

function onClientSelect(client) {
  if (client?.clientName) {
    props.form.clientName = client.clientName
    emit('update')
  }
}
</script>

<template>
  <div class="case-info-panel">
    <el-card>
      <template #header><strong>案件信息</strong></template>
      <el-form label-width="80px" size="small">
        <el-form-item label="案件名称">
          <el-input v-model="form.caseName" @input="onInput" />
        </el-form-item>
        <el-form-item label="案号">
          <el-input v-model="form.caseNo" @input="onInput" placeholder="如：(2024)京73行初1号" />
        </el-form-item>
        <el-form-item label="内部卷号">
          <el-input v-model="form.internalNo" @input="onInput" />
        </el-form-item>
        <el-form-item label="轨道">
          <el-select v-model="form.track" @change="onInput" style="width: 100%">
            <el-option v-for="opt in trackOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="案由">
          <el-input v-model="form.causeAction" @input="onInput" />
        </el-form-item>
        <el-form-item label="审级">
          <el-select v-model="form.caseLevel" clearable @change="onInput" style="width: 100%">
            <el-option v-for="opt in levelOptions" :key="opt" :label="opt" :value="opt" />
          </el-select>
        </el-form-item>
        <el-form-item label="案件进展">
          <el-select v-model="form.caseProgress" clearable filterable allow-create @change="onInput" style="width: 100%">
            <el-option v-for="opt in statusOptions" :key="opt" :label="opt" :value="opt" />
          </el-select>
        </el-form-item>
        <el-form-item label="案件结果">
          <el-select v-model="form.caseResult" clearable @change="onInput" style="width: 100%">
            <el-option label="胜诉" value="胜诉" />
            <el-option label="败诉" value="败诉" />
            <el-option label="结案" value="结案" />
            <el-option label="对方撤案" value="对方撤案" />
            <el-option label="撤诉" value="撤诉" />
          </el-select>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card style="margin-top: 12px">
      <template #header><strong>当事人</strong></template>
      <el-form label-width="80px" size="small">
        <el-form-item label="客户">
          <ReferenceSelect
            v-model="form.clientName"
            type="client"
            placeholder="搜索客户名称"
            @select="onClientSelect"
            @update:model-value="onInput"
          />
        </el-form-item>
        <el-form-item label="我方地位">
          <el-input v-model="form.ourRole" @input="onInput" placeholder="如：专利权人" />
        </el-form-item>
        <el-form-item label="对方">
          <el-input v-model="form.opponentName" @input="onInput" />
        </el-form-item>
        <el-form-item label="对方地位">
          <el-input v-model="form.opponentRole" @input="onInput" placeholder="如：请求人" />
        </el-form-item>
        <el-form-item label="对方代理">
          <el-input v-model="form.opponentFirm" @input="onInput" />
        </el-form-item>
      </el-form>
    </el-card>

    <el-card style="margin-top: 12px">
      <template #header><strong>审理</strong></template>
      <el-form label-width="80px" size="small">
        <el-form-item label="法院">
          <el-input v-model="form.court" @input="onInput" />
        </el-form-item>
        <el-form-item label="办案人">
          <el-input v-model="form.attorneys" @input="onInput" placeholder="多人用逗号分隔" />
        </el-form-item>
        <el-form-item label="书记员">
          <el-input v-model="form.clerk" @input="onInput" />
        </el-form-item>
      </el-form>
    </el-card>

    <el-card style="margin-top: 12px">
      <template #header><strong>专利</strong></template>
      <el-form label-width="80px" size="small">
        <el-form-item label="专利名称">
          <el-input v-model="form.patentName" @input="onInput" />
        </el-form-item>
        <el-form-item label="申请号">
          <el-input v-model="form.patentAppNo" @input="onInput" />
        </el-form-item>
      </el-form>
    </el-card>

    <el-card style="margin-top: 12px">
      <template #header><strong>备注</strong></template>
      <el-input v-model="form.notes" type="textarea" :rows="3" @input="onInput" />
    </el-card>
  </div>
</template>

<style scoped>
.case-info-panel {
  /* 继承父级布局 */
}
</style>
