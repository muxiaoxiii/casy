<template>
  <div class="template-browser">
    <!-- 搜索栏 -->
    <div class="search-bar">
      <el-input
        v-model="searchText"
        placeholder="搜索模板..."
        clearable
        prefix-icon="Search"
        size="small"
      />
    </div>

    <!-- 模板列表 -->
    <div class="template-list" v-loading="loading">
      <el-alert v-if="error" :title="error" type="error" :closable="false"><el-button text @click="loadTemplates">重试</el-button></el-alert>
      <!-- 按分类分组显示 -->
      <div
        v-for="(templates, category) in filteredByCategory"
        :key="category"
        class="template-category"
      >
        <div class="category-header" role="button" tabindex="0" :aria-expanded="!!expandedCategories[category]" @keydown.enter="toggleCategory(category)" @keydown.space.prevent="toggleCategory(category)" @click="toggleCategory(category)">
          <el-icon>
            <ArrowRight v-if="!expandedCategories[category]" />
            <ArrowDown v-else />
          </el-icon>
          <span class="category-name">{{ category }}</span>
          <el-badge :value="templates.length" type="info" />
        </div>

        <el-collapse-transition>
          <div v-show="expandedCategories[category]" class="category-items">
            <div
              v-for="tpl in templates"
              :key="tpl.id"
              :class="['template-card', { active: selected?.id === tpl.id }]"
              role="button" tabindex="0" :aria-label="`使用模板 ${tpl.name}`" @keydown.enter="selectTemplate(tpl)" @keydown.space.prevent="selectTemplate(tpl)" @click="selectTemplate(tpl)"
            >
              <div class="template-icon">📄</div>
              <div class="template-info">
                <div class="template-name">{{ tpl.name }}</div>
                <div class="template-meta">
                  <span class="field-count">{{ tpl.fieldCount }} 个字段</span>
                  <span class="template-desc" v-if="tpl.description">
                    {{ tpl.description }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </el-collapse-transition>
      </div>

      <!-- 空状态 -->
      <el-empty
        v-if="!loading && !error && Object.keys(filteredByCategory).length === 0"
        description="暂无模板"
        :image-size="60"
      >
        <template #description>
          <p>暂无模板</p>
          <p class="empty-hint">
            请将 .docx 模板文件放入<br />
            ~/Documents/Casy/templates/ 目录
          </p>
        </template>
      </el-empty>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, reactive, watch } from 'vue'
import { useDocsyBridge } from '../composables/useDocsyBridge.js'

const props = defineProps({
  modelValue: {
    type: Object,
    default: null,
  },
})

const emit = defineEmits(['update:modelValue', 'select'])

const { templates, loading, error, loadTemplates, searchTemplates, templatesByCategory } =
  useDocsyBridge()

const searchText = ref('')
const selected = ref(props.modelValue)

watch(() => props.modelValue, value => {
  selected.value = value
})

// 分类展开状态
const expandedCategories = reactive({})

// 过滤后的模板（按分类分组）
const filteredByCategory = computed(() => {
  const filtered = searchTemplates(searchText.value)
  const groups = {}

  for (const tpl of filtered) {
    const cat = tpl.category || '其他'
    if (!groups[cat]) {
      groups[cat] = []
    }
    groups[cat].push(tpl)
  }

  return groups
})

// 切换分类展开状态
function toggleCategory(category) {
  expandedCategories[category] = !expandedCategories[category]
}

// 选择模板
function selectTemplate(tpl) {
  selected.value = tpl
  emit('update:modelValue', tpl)
  emit('select', tpl)
}

// 初始化：加载模板并展开第一个分类
onMounted(async () => {
  await loadTemplates()

  // 默认展开第一个分类
  const cats = Object.keys(templatesByCategory.value)
  if (cats.length > 0) {
    expandedCategories[cats[0]] = true
  }
})
</script>

<style scoped>
.template-browser {
  display: flex;
  flex-direction: column;
  height: 100%;
  border-right: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
}

.search-bar {
  padding: 12px;
  border-bottom: 1px solid var(--c-border);
}

.template-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}

.template-category {
  margin-bottom: 4px;
}

.category-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  cursor: pointer;
  user-select: none;
  transition: background var(--motion-fast);
}

.category-header:hover {
  background: var(--c-primary-light);
}

.category-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text-secondary);
}

.category-items {
  padding: 0 8px;
}

.template-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  margin: 2px 0;
  border-radius: 6px;
  cursor: pointer;
  transition: all var(--motion-fast) var(--ease-out);
  border: 1px solid transparent;
}

.template-card:hover {
  background: var(--c-primary-light);
  border-color: var(--c-primary-light);
}

.template-card.active {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
  box-shadow: 0 0 0 1px var(--c-primary) inset;
}

.template-icon {
  font-size: 24px;
  flex-shrink: 0;
}

.template-info {
  flex: 1;
  min-width: 0;
}

.template-name {
  font-size: 14px;
  color: var(--c-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.template-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  font-size: 12px;
  color: var(--gray-400);
}

.field-count {
  padding: 1px 6px;
  background: var(--bg-success-weak);
  color: var(--c-success);
  border-radius: 3px;
  font-size: 11px;
}

.template-desc {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.empty-hint {
  font-size: 12px;
  color: var(--gray-400);
  text-align: center;
  line-height: 1.6;
}
</style>
