<script setup lang="ts">
/**
 * ViewSchemePicker —— 视图方案选择器（C7）
 *
 * 把"当前这套搜索/排序/筛选/列配置"存下来、命名、随时切回、可导出分享。
 * 底层复用既有的 saved_filters（`list_saved_filters` / `save_filter` /
 * `delete_filter`）——机制一直都在，此前只有案件一处在用。
 *
 * 约定：
 * - `moduleKey` 是方案的作用域（tasks / files / cases…），同一模块内方案互斥；
 * - `state` 是调用方当前的完整视图状态，保存时整份写入；
 * - 应用方案由调用方在 `@apply` 里把 state 写回自己的 ref——组件不持有视图状态。
 */
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../core/plugin/context'
import type { IpcJsonObject } from '../../types/ipc'
import { ElMessage } from 'element-plus'
import { FolderOpened, Download, Upload } from '../icons'

const props = withDefaults(defineProps<{
  /** 方案作用域：tasks / files / cases … */
  moduleKey: string
  /** 当前视图状态（保存时整份写入） */
  state: Record<string, unknown>
  /** 按钮尺寸 */
  size?: 'small' | 'default'
}>(), { size: 'small' })

const emit = defineEmits<{
  (e: 'apply', state: Record<string, unknown>): void
}>()

interface Scheme {
  id: string
  name: string
  state: Record<string, unknown>
}

/** saved_filters 的 filter 列是 IpcJsonObject；视图状态由字符串/数字/布尔组成 */
const toIpcState = (state: Record<string, unknown>): IpcJsonObject => JSON.parse(JSON.stringify(state)) as IpcJsonObject

const schemes = ref<Scheme[]>([])
const saving = ref(false)
const newName = ref('')

const hasState = computed(() => Object.keys(props.state || {}).some((key) => props.state[key] !== '' && props.state[key] != null))

async function load() {
  const result = await casyContext.settings.savedFilters(props.moduleKey)
  if (result.ok && Array.isArray(result.data)) {
    schemes.value = result.data.map((raw) => ({
      id: String(raw.id ?? ''),
      name: String(raw.name ?? ''),
      state: (raw.filter && typeof raw.filter === 'object' ? raw.filter : {}) as Record<string, unknown>,
    }))
  }
}

async function save() {
  const name = newName.value.trim()
  if (!name) { ElMessage.warning('给这套视图起个名字'); return }
  if (saving.value) return
  saving.value = true
  const result = await casyContext.settings.saveFilter({
    name,
    module: props.moduleKey,
    entityType: props.moduleKey,
    filter: toIpcState(props.state),
  })
  saving.value = false
  if (!result.ok) { ElMessage.error(result.error || '保存方案失败'); return }
  newName.value = ''
  ElMessage.success('方案已保存')
  await load()
}

async function remove(scheme: Scheme) {
  const result = await casyContext.settings.deleteFilter(scheme.id)
  if (!result.ok) { ElMessage.error(result.error || '删除失败'); return }
  await load()
}

function apply(scheme: Scheme) {
  emit('apply', scheme.state)
}

/** 导出为 JSON 文件（C7：方案可分享/迁移） */
function exportSchemes() {
  const payload = JSON.stringify({ module: props.moduleKey, schemes: schemes.value }, null, 2)
  const blob = new Blob([payload], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `casy-view-schemes-${props.moduleKey}.json`
  anchor.click()
  URL.revokeObjectURL(url)
  ElMessage.success(`已导出 ${schemes.value.length} 套方案`)
}

/** 从 JSON 导入（逐套保存，失败的跳过并说明） */
async function importSchemes() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = 'application/json'
  input.onchange = async () => {
    const file = input.files?.[0]
    if (!file) return
    try {
      const parsed = JSON.parse(await file.text()) as { schemes?: Array<{ name?: string; state?: Record<string, unknown> }> }
      const list = Array.isArray(parsed.schemes) ? parsed.schemes : []
      let ok = 0
      for (const item of list) {
        const name = String(item.name || '').trim()
        if (!name || !item.state) continue
        const result = await casyContext.settings.saveFilter({
          name, module: props.moduleKey, entityType: props.moduleKey, filter: toIpcState(item.state),
        })
        if (result.ok) ok += 1
      }
      ElMessage.success(`已导入 ${ok} 套方案${ok === list.length ? '' : `（共 ${list.length} 套）`}`)
      await load()
    } catch (error) {
      ElMessage.error(`导入失败：${error instanceof Error ? error.message : String(error)}`)
    }
  }
  input.click()
}

onMounted(load)
</script>

<template>
  <el-dropdown trigger="click" placement="bottom-end" @command="(command: string) => command === '__export__' ? exportSchemes() : command === '__import__' ? importSchemes() : null">
    <el-button :size="size" :disabled="!schemes.length && !hasState">
      <el-icon :size="13"><FolderOpened /></el-icon>
      视图方案<span v-if="schemes.length"> · {{ schemes.length }}</span>
    </el-button>
    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item v-for="scheme in schemes" :key="scheme.id" divided>
          <span class="scheme-row">
            <el-button text size="small" class="scheme-apply" @click.stop="apply(scheme)">{{ scheme.name }}</el-button>
            <el-button text size="small" class="scheme-del" @click.stop="remove(scheme)">删除</el-button>
          </span>
        </el-dropdown-item>
        <li class="scheme-save">
          <el-input v-model="newName" size="small" placeholder="保存当前视图为…" @keyup.enter="save" />
          <el-button size="small" type="primary" :loading="saving" @click="save">保存</el-button>
        </li>
        <el-dropdown-item command="__export__" :disabled="!schemes.length">
          <el-icon><Download /></el-icon> 导出方案
        </el-dropdown-item>
        <el-dropdown-item command="__import__">
          <el-icon><Upload /></el-icon> 导入方案
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>
</template>

<style scoped>
.scheme-row { display: inline-flex; align-items: center; gap: 4px; width: 100%; }
.scheme-apply { flex: 1; min-width: 0; justify-content: flex-start; }
.scheme-del { color: var(--c-text-secondary); }
.scheme-save { display: flex; align-items: center; gap: 6px; padding: 6px 10px; }
</style>
