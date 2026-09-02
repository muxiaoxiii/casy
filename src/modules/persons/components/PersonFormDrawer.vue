<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { KIND_META, KIND_OPTIONS, type PersonDto, type PersonKind } from '../types'

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    /** 传入则为编辑，否则为新建 */
    person?: PersonDto | null
    /** 新建时预选类型 */
    presetKind?: PersonKind | ''
  }>(),
  { person: null, presetKind: '' }
)

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  /** 保存成功，回传实体 id（新建为新 id） */
  (e: 'saved', id: string): void
}>()

interface PersonForm {
  kind: PersonKind
  name: string
  org: string
  phone: string
  email: string
  preferences: string
  notes: string
}

const blank = (kind: PersonKind): PersonForm => ({
  kind,
  name: '',
  org: '',
  phone: '',
  email: '',
  preferences: '',
  notes: '',
})

const form = reactive<PersonForm>(blank('contact'))
const saving = ref(false)

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    const p = props.person
    if (p) {
      form.kind = p.kind
      form.name = p.name
      form.org = p.org ?? ''
      form.phone = p.phone ?? ''
      form.email = p.email ?? ''
      form.preferences = p.preferences ?? ''
      form.notes = p.notes ?? ''
    } else {
      Object.assign(form, blank(props.presetKind || 'contact'))
    }
  }
)

function close() {
  emit('update:modelValue', false)
}

function nullIfEmpty(s: string): string | null {
  const t = s.trim()
  return t === '' ? null : t
}

async function save() {
  if (!form.name.trim()) {
    ElMessage.warning('请填写名称')
    return
  }
  saving.value = true
  const res = await tauriCallSafe<string>('upsert_person', {
    id: props.person?.id ?? null,
    kind: form.kind,
    name: form.name.trim(),
    org: nullIfEmpty(form.org),
    phone: nullIfEmpty(form.phone),
    email: nullIfEmpty(form.email),
    preferences: nullIfEmpty(form.preferences),
    notes: nullIfEmpty(form.notes),
  })
  saving.value = false
  if (!res.ok || !res.data) {
    ElMessage.error(res.error || '保存失败')
    return
  }
  ElMessage.success(props.person ? '已更新' : '已创建')
  emit('saved', res.data)
  close()
}
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    :title="person ? '编辑实体' : '新建实体'"
    size="440px"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <el-form label-position="top" class="person-form">
      <el-form-item label="类型" required>
        <el-select v-model="form.kind" style="width: 100%">
          <el-option v-for="k in KIND_OPTIONS" :key="k" :value="k" :label="KIND_META[k].label">
            <span class="kind-option">
              <el-icon :color="KIND_META[k].color"><component :is="KIND_META[k].icon" /></el-icon>
              {{ KIND_META[k].label }}
            </span>
          </el-option>
        </el-select>
      </el-form-item>

      <el-form-item label="名称" required>
        <el-input v-model="form.name" placeholder="姓名 / 机构名称" maxlength="50" />
      </el-form-item>

      <el-form-item label="机构 / 单位">
        <el-input v-model="form.org" placeholder="如：某某中级人民法院 / 某某律所" maxlength="100" />
      </el-form-item>

      <div class="form-row">
        <el-form-item label="电话">
          <el-input v-model="form.phone" placeholder="联系电话" maxlength="30" />
        </el-form-item>
        <el-form-item label="邮箱">
          <el-input v-model="form.email" placeholder="电子邮箱" maxlength="100" />
        </el-form-item>
      </div>

      <el-form-item label="偏好 / 习惯">
        <el-input
          v-model="form.preferences"
          type="textarea"
          :rows="4"
          placeholder="如：法官偏好、庭上习惯、沟通注意事项…"
        />
      </el-form-item>

      <el-form-item label="备注">
        <el-input v-model="form.notes" type="textarea" :rows="3" placeholder="内部备注" />
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="close">取消</el-button>
      <el-button type="primary" :loading="saving" @click="save">
        {{ person ? '保存修改' : '创建' }}
      </el-button>
    </template>
  </el-drawer>
</template>

<style scoped>
.kind-option {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.person-form :deep(.el-form-item__label) {
  font-weight: 500;
  color: var(--c-text-regular);
}
</style>
