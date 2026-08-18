<script setup>
/**
 * ReferenceSelect - 远程搜索下拉组件
 *
 * 支持案件/客户/法官/知识条目的模糊搜索，基于 el-select + filterable + remote
 *
 * Props:
 *   modelValue: 当前选中的值（单选 String / 多选 Array）
 *   type: 搜索类型 ('case' | 'client' | 'judge' | 'knowledge')
 *   placeholder: 占位文本
 *   disabled: 是否禁用
 *   clearable: 是否可清除
 *   multiple: 是否多选
 *   labelField: 多选时用于展示已选项的字段（内部使用）
 *
 * Events:
 *   update:modelValue: 值变化时触发
 *   select: 选中时触发，返回完整对象（多选时返回数组）
 */
import { ref, watch } from 'vue'
import { tauriCallSafe } from '../../core/tauriBridge'

const props = defineProps({
  modelValue: {
    type: [String, Array],
    default: null,
  },
  type: {
    type: String,
    default: 'case',
    validator: (val) => ['case', 'client', 'judge', 'knowledge'].includes(val),
  },
  placeholder: {
    type: String,
    default: '搜索...',
  },
  disabled: {
    type: Boolean,
    default: false,
  },
  clearable: {
    type: Boolean,
    default: true,
  },
  multiple: {
    type: Boolean,
    default: false,
  },
})

const emit = defineEmits(['update:modelValue', 'select'])

const options = ref([])
const loading = ref(false)

// 已选中项的标签映射（多选模式下用于显示已选标签）
const selectedLabels = ref({})

// 搜索防抖
let searchTimer = null

async function remoteSearch(query) {
  if (!query || query.length < 1) {
    options.value = []
    return
  }

  if (searchTimer) clearTimeout(searchTimer)

  return new Promise((resolve) => {
    searchTimer = setTimeout(async () => {
      loading.value = true
      try {
        let results = []

        if (props.type === 'case') {
          const result = await tauriCallSafe('search_cases', { query })
          if (result.ok) {
            results = (result.data || []).map((c) => ({
              value: c.id,
              label: `${c.caseName} (${c.caseNo || '无案号'})`,
              sublabel: `${c.clientName} vs ${c.opponentName}`,
              tag: c.caseStatus || null,
              tagType: c.caseStatus === '已完结' ? 'info' : 'success',
              raw: c,
            }))
          }
        } else if (props.type === 'client') {
          const result = await tauriCallSafe('search_cases', { query })
          if (result.ok) {
            const clientMap = new Map()
            for (const c of result.data || []) {
              if (c.clientName && !clientMap.has(c.clientName)) {
                clientMap.set(c.clientName, {
                  value: c.clientName,
                  label: c.clientName,
                  sublabel: `相关案件: ${c.caseName}`,
                  raw: { clientName: c.clientName, clientId: c.clientId },
                })
              }
            }
            results = Array.from(clientMap.values())
          }
        } else if (props.type === 'judge') {
          const result = await tauriCallSafe('search_cases', { query })
          if (result.ok) {
            const judgeMap = new Map()
            for (const c of result.data || []) {
              if (c.judgePanel && !judgeMap.has(c.judgePanel)) {
                judgeMap.set(c.judgePanel, {
                  value: c.judgePanel,
                  label: c.judgePanel,
                  sublabel: c.court || '',
                  raw: { judgePanel: c.judgePanel, court: c.court },
                })
              }
            }
            results = Array.from(judgeMap.values())
          }
        } else if (props.type === 'knowledge') {
          const result = await tauriCallSafe('hybrid_search_knowledge', { query, limit: 20 })
          if (result.ok) {
            results = (result.data || []).map((item) => ({
              value: item.id,
              label: item.title || '未命名条目',
              sublabel: item.source || item.category || '',
              tag: item.category || null,
              tagType: 'warning',
              raw: item,
            }))
          }
        }

        options.value = results
      } catch (e) {
        console.warn('搜索失败:', e)
        options.value = []
      } finally {
        loading.value = false
      }
      resolve()
    }, 300)
  })
}

function onChange(val) {
  emit('update:modelValue', val)

  if (props.multiple && Array.isArray(val)) {
    // 多选模式：收集所有选中项的 raw 对象
    const selectedItems = val
      .map((v) => {
        const opt = options.value.find((o) => o.value === v)
        if (opt) return opt.raw
        // 已选但不在当前 options 中的，从缓存取
        return selectedLabels.value[v] || null
      })
      .filter(Boolean)
    emit('select', selectedItems)
  } else {
    // 单选模式
    const selected = options.value.find((opt) => opt.value === val)
    if (selected) {
      emit('select', selected.raw)
    }
  }
}

function onClear() {
  emit('update:modelValue', props.multiple ? [] : null)
  selectedLabels.value = {}
  options.value = []
}

// 远程搜索时缓存已选中项的标签
watch(options, (newOpts) => {
  for (const opt of newOpts) {
    selectedLabels.value[opt.value] = opt
  }
})
</script>

<template>
  <el-select
    :model-value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    :clearable="clearable"
    :multiple="multiple"
    filterable
    remote
    reserve-keyword
    :remote-method="remoteSearch"
    :loading="loading"
    style="width: 100%"
    @change="onChange"
    @clear="onClear"
  >
    <el-option
      v-for="opt in options"
      :key="opt.value"
      :label="opt.label"
      :value="opt.value"
    >
      <div class="option-content">
        <div class="option-main">
          <span class="option-label">{{ opt.label }}</span>
          <el-tag
            v-if="opt.tag"
            :type="opt.tagType || 'info'"
            size="small"
            effect="plain"
            class="option-tag"
          >
            {{ opt.tag }}
          </el-tag>
        </div>
        <span v-if="opt.sublabel" class="option-sublabel">{{ opt.sublabel }}</span>
      </div>
    </el-option>
  </el-select>
</template>

<style scoped>
.option-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 2px 0;
}

.option-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.option-label {
  font-size: 13px;
  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.option-tag {
  flex-shrink: 0;
  transform: scale(0.85);
  transform-origin: left center;
}

.option-sublabel {
  font-size: 11px;
  color: #909399;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
