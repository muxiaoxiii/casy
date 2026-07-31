# Casy 实现规格 — UI + Copilot + Docsy 集成

## 1. 前端依赖

```json
{
  "dependencies": {
    "vue": "^3.4",
    "vue-router": "^4.3",
    "pinia": "^2.1",
    "element-plus": "^2.7",
    "@element-plus/icons-vue": "^2.3",
    "@tiptap/core": "^3.29",
    "@tiptap/starter-kit": "^3.29",
    "@tiptap/suggestion": "^3.29",
    "@tiptap/extension-placeholder": "^3.29",
    "@tiptap/extension-highlight": "^3.29",
    "@tiptap/extension-underline": "^3.29",
    "@tiptap/vue-3": "^3.29",
    "@tauri-apps/api": "^2.0",
    "@tauri-apps/plugin-dialog": "^2.0",
    "@tauri-apps/plugin-fs": "^2.5",
    "@tauri-apps/plugin-shell": "^2.0",
    "@tauri-apps/plugin-notification": "^2.0"
  }
}
```

## 2. 路由

```javascript
// src/router/index.js
const routes = [
  { path: '/', name: 'home', component: () => import('../modules/home/HomeView.vue') },
  { path: '/cases', name: 'cases', component: () => import('../modules/cases/views/CaseListView.vue') },
  { path: '/cases/:id', name: 'case-detail', component: () => import('../modules/cases/views/CaseDetailView.vue') },
  { path: '/calendar', name: 'calendar', component: () => import('../modules/calendar/views/CalendarView.vue') },
  { path: '/tasks', name: 'tasks', component: () => import('../modules/tasks/views/TasksView.vue') },
  { path: '/inbox', name: 'inbox', component: () => import('../modules/inbox/views/InboxView.vue') },
  { path: '/documents', name: 'documents', component: () => import('../modules/documents/views/DocumentGenView.vue') },
  { path: '/write/:caseId?', name: 'write', component: () => import('../modules/documents/views/WritingView.vue') },
  { path: '/files/:caseId', name: 'files', component: () => import('../modules/files/views/CaseFilesView.vue') },
  { path: '/sync', name: 'sync', component: () => import('../modules/sync/views/SyncStatusView.vue') },
  { path: '/settings', name: 'settings', component: () => import('../modules/settings/SettingsView.vue') },
]
```

## 3. Pinia Stores

### 3.1 Cases Store

```javascript
// src/modules/cases/composables/useCases.js
import { defineStore } from 'pinia'
import { tauriCallSafe } from '../../../core/tauriBridge.js'

export const useCasesStore = defineStore('cases', {
  state: () => ({
    cases: [],
    currentCase: null,
    loading: false,
    filter: { track: null, client: null, court: null, status: null, search: '' },
    page: 1,
    perPage: 50,
    total: 0,
  }),

  getters: {
    activeCases: (state) => state.cases.filter(c => c.caseStatus !== '已完结'),
    groupedByClient: (state) => {
      const groups = {}
      for (const c of state.cases) {
        const key = c.clientName || '未知客户'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },
    groupedByTrack: (state) => {
      const groups = {}
      for (const c of state.cases) {
        const key = c.track || 'other'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },
  },

  actions: {
    async loadCases() {
      this.loading = true
      const result = await tauriCallSafe('list_cases', {
        filter: this.filter,
        page: this.page,
        perPage: this.perPage,
      })
      if (result.ok) {
        this.cases = result.data.items
        this.total = result.data.total
      }
      this.loading = false
    },

    async loadCase(id) {
      const result = await tauriCallSafe('get_case', { id })
      if (result.ok) this.currentCase = result.data
      return result
    },

    async createCase(data) {
      const result = await tauriCallSafe('create_case', { data })
      if (result.ok) await this.loadCases()
      return result
    },

    async updateCase(id, data) {
      const result = await tauriCallSafe('update_case', { id, data })
      if (result.ok) {
        const idx = this.cases.findIndex(c => c.id === id)
        if (idx >= 0) this.cases[idx] = { ...this.cases[idx], ...data }
        if (this.currentCase?.id === id) this.currentCase = { ...this.currentCase, ...data }
      }
      return result
    },

    async deleteCase(id) {
      const result = await tauriCallSafe('delete_case', { id })
      if (result.ok) await this.loadCases()
      return result
    },

    async searchCases(query) {
      const result = await tauriCallSafe('search_cases', { query })
      if (result.ok) this.cases = result.data
      return result
    },
  },
})
```

### 3.2 Inbox Store

```javascript
// src/modules/inbox/composables/useInbox.js
export const useInboxStore = defineStore('inbox', {
  state: () => ({
    items: [],
    loading: false,
    processing: false,
  }),

  getters: {
    pending: (state) => state.items.filter(i => i.status === 'pending'),
    filed: (state) => state.items.filter(i => i.status === 'filed'),
  },

  actions: {
    async loadItems() {
      this.loading = true
      const result = await tauriCallSafe('list_inbox_items', {})
      if (result.ok) this.items = result.data
      this.loading = false
    },

    async addItem(sourceType, data) {
      const result = await tauriCallSafe('add_inbox_item', { sourceType, ...data })
      if (result.ok) {
        await this.processItem(result.data.id)
        await this.loadItems()
      }
      return result
    },

    async processItem(id) {
      this.processing = true
      const result = await tauriCallSafe('process_inbox_item', { id })
      this.processing = false
      return result
    },

    async fileToCase(itemId, caseId, category) {
      const result = await tauriCallSafe('file_inbox_item', { itemId, caseId, category })
      if (result.ok) await this.loadItems()
      return result
    },

    async dismiss(id) {
      const result = await tauriCallSafe('dismiss_inbox_item', { id })
      if (result.ok) await this.loadItems()
      return result
    },
  },
})
```

### 3.3 Settings Store

```javascript
// src/modules/settings/composables/useSettings.js
export const useSettingsStore = defineStore('settings', {
  state: () => ({
    // WebDAV
    webdavUrl: '',
    webdavUsername: '',
    webdavPassword: '',
    webdavAutoSync: true,

    // 飞书
    feishuAppToken: '',
    feishuTableIds: {},
    feishuApiKey: '',

    // AI
    aiMode: 'none',        // 'none' | 'local' | 'remote'
    aiBackend: 'ollama',    // 'ollama' | 'openai' | 'custom'
    aiApiUrl: 'http://localhost:11434',
    aiApiKey: '',
    aiModel: 'qwen2.5:14b',
    aiDailyLimit: 50,
    ocrEngine: 'tesseract', // 'tesseract' | 'vision_llm'

    // IMAP
    imapAccounts: [],

    // 通用
    caseFolderBase: '',
    theme: 'system',
    language: 'zh-CN',
  }),

  actions: {
    async load() {
      const result = await tauriCallSafe('get_settings', {})
      if (result.ok) Object.assign(this, result.data)
    },

    async save() {
      const result = await tauriCallSafe('save_settings', { settings: this.$state })
      return result
    },
  },
})
```

## 4. TipTap Copilot 扩展

### 4.1 案件字段补全

```javascript
// src/modules/documents/composables/caseFieldSuggestion.js
import { Extension } from '@tiptap/core'
import { Suggestion } from '@tiptap/suggestion'

function getSuggestionItems({ query, editor }) {
  // 从当前编辑器的关联案件获取字段
  const caseData = editor.storage.caseData || {}

  const fields = [
    { label: '案号', value: caseData.caseNo || '', icon: '📋' },
    { label: '案件名称', value: caseData.caseName || '', icon: '📋' },
    { label: '客户名称', value: caseData.clientName || '', icon: '👤' },
    { label: '我方地位', value: caseData.ourRole || '', icon: '👤' },
    { label: '对方名称', value: caseData.opponentName || '', icon: '👥' },
    { label: '对方地位', value: caseData.opponentRole || '', icon: '👥' },
    { label: '审理机关', value: caseData.court || '', icon: '🏛️' },
    { label: '案由', value: caseData.causeAction || '', icon: '📝' },
    { label: '专利名称', value: caseData.patentName || '', icon: '📄' },
    { label: '专利申请号', value: caseData.patentAppNo || '', icon: '📄' },
    { label: '内部卷号', value: caseData.internalNo || '', icon: '📁' },
    { label: '今日日期', value: new Date().toLocaleDateString('zh-CN'), icon: '📅' },
    { label: '办案人', value: (caseData.attorneys || []).join('、'), icon: '👤' },
  ]

  return fields
    .filter(f => f.label.includes(query) || f.value.includes(query))
    .map(f => ({
      ...f,
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent(f.value).run()
      },
    }))
}

export const CaseFieldSuggestion = Extension.create({
  name: 'caseFieldSuggestion',
  addOptions() {
    return {
      suggestion: {
        char: '{',
        items: getSuggestionItems,
        render: () => {
          let popup, items
          return {
            onStart(props) {
              popup = document.createElement('div')
              popup.className = 'suggestion-popup'
              document.body.appendChild(popup)
              renderItems(props)
            },
            onUpdate(props) { renderItems(props) },
            onKeyDown(props) {
              if (props.event.key === 'Escape') { popup.remove(); return true }
              return false
            },
            onExit() { popup.remove() },
          }
        },
      },
    }
  },
  addProseMirrorPlugins() {
    return [Suggestion({ editor: this.editor, ...this.options.suggestion })]
  },
})
```

### 4.2 法条补全

```javascript
// src/modules/documents/composables/legalProvisionSuggestion.js

// 本地法条数据库（常用专利法条）
const PATENT_LAW = [
  { article: '第2条', title: '发明创造的定义', law: '专利法' },
  { article: '第22条', title: '授予专利权的条件（新颖性、创造性、实用性）', law: '专利法' },
  { article: '第23条', title: '外观设计的授权条件', law: '专利法' },
  { article: '第25条', title: '不授予专利权的客体', law: '专利法' },
  { article: '第26条', title: '说明书和权利要求书', law: '专利法' },
  { article: '第33条', title: '修改不得超范围', law: '专利法' },
  { article: '第42条', title: '专利权期限', law: '专利法' },
  { article: '第45条', title: '无效宣告请求', law: '专利法' },
  { article: '第46条', title: '无效宣告审查决定', law: '专利法' },
  { article: '第47条', title: '无效宣告的效力', law: '专利法' },
  { article: '第59条', title: '保护范围', law: '专利法' },
  { article: '第64条', title: '权利要求的解释', law: '专利法' },
  { article: '第65条', title: '损害赔偿', law: '专利法' },
  { article: '第71条', title: '诉前保全', law: '专利法' },
  // ... 更多
]

function getSuggestionItems({ query }) {
  return PATENT_LAW
    .filter(l => l.article.includes(query) || l.title.includes(query) || l.law.includes(query))
    .map(l => ({
      label: `《${l.law}》${l.article} ${l.title}`,
      icon: '📜',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent(`《${l.law}》${l.article}`).run()
      },
    }))
}

export const LegalProvisionSuggestion = Extension.create({
  name: 'legalProvisionSuggestion',
  addOptions() {
    return { suggestion: { char: '【', items: getSuggestionItems } }
  },
  addProseMirrorPlugins() {
    return [Suggestion({ editor: this.editor, ...this.options.suggestion })]
  },
})
```

### 4.3 当事人补全

```javascript
// src/modules/documents/composables/partyNameSuggestion.js

function getSuggestionItems({ query }) {
  // 从 cases store 获取所有当事人
  const store = useCasesStore()
  const parties = new Set()
  for (const c of store.cases) {
    if (c.clientName) parties.add(c.clientName)
    if (c.opponentName) parties.add(c.opponentName)
  }

  return Array.from(parties)
    .filter(name => name.includes(query))
    .map(name => ({
      label: name,
      icon: '👤',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent(name).run()
      },
    }))
}

export const PartyNameSuggestion = Extension.create({
  name: 'partyNameSuggestion',
  addOptions() {
    return { suggestion: { char: '@', items: getSuggestionItems } }
  },
  addProseMirrorPlugins() {
    return [Suggestion({ editor: this.editor, ...this.options.suggestion })]
  },
})
```

### 4.4 编辑器组件

```vue
<!-- src/modules/documents/components/LegalEditor.vue -->
<template>
  <div class="legal-editor">
    <editor-content :editor="editor" class="editor-content" />
    <div class="editor-toolbar">
      <button @click="editor.chain().focus().toggleBold().run()" :class="{ active: editor.isActive('bold') }">B</button>
      <button @click="editor.chain().focus().toggleItalic().run()" :class="{ active: editor.isActive('italic') }">I</button>
      <button @click="editor.chain().focus().toggleUnderline().run()" :class="{ active: editor.isActive('underline') }">U</button>
      <span class="separator">|</span>
      <button @click="editor.chain().focus().undo().run()">撤销</button>
      <button @click="editor.chain().focus().redo().run()">重做</button>
      <span class="separator">|</span>
      <button @click="insertField">插入字段 {</button>
      <button @click="insertLaw">插入法条 【</button>
      <button @click="insertParty">插入当事人 @</button>
    </div>
  </div>
</template>

<script setup>
import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Underline from '@tiptap/extension-underline'
import Placeholder from '@tiptap/extension-placeholder'
import { CaseFieldSuggestion } from '../composables/caseFieldSuggestion.js'
import { LegalProvisionSuggestion } from '../composables/legalProvisionSuggestion.js'
import { PartyNameSuggestion } from '../composables/partyNameSuggestion.js'

const props = defineProps({
  modelValue: { type: String, default: '' },
  caseData: { type: Object, default: () => ({}) },
})

const emit = defineEmits(['update:modelValue', 'save'])

const editor = useEditor({
  content: props.modelValue,
  extensions: [
    StarterKit,
    Underline,
    Placeholder.configure({ placeholder: '开始撰写...' }),
    CaseFieldSuggestion,
    LegalProvisionSuggestion,
    PartyNameSuggestion,
  ],
  onUpdate: ({ editor }) => {
    emit('update:modelValue', editor.getHTML())
  },
})

// 将案件数据存入 editor storage 供 suggestion 使用
watch(() => props.caseData, (data) => {
  if (editor.value) editor.value.storage.caseData = data
}, { immediate: true })

function insertField() { editor.value.chain().focus().insertContent('{').run() }
function insertLaw() { editor.value.chain().focus().insertContent('【').run() }
function insertParty() { editor.value.chain().focus().insertContent('@').run() }
</script>
```

## 5. Docsy 集成

### 5.1 IPC 调用桥接

```javascript
// src/modules/documents/composables/useDocsyBridge.js
import { tauriCallSafe } from '../../../core/tauriBridge.js'

export function useDocsyBridge() {
  /// 列出可用的 Docsy 模板
  async function listTemplates() {
    return await tauriCallSafe('list_docsy_templates', {})
  }

  /// 从案件数据生成文书
  async function generateDocument(templatePath, caseData, outputPath) {
    // 将案件字段映射到 Docsy 模板字段
    const values = mapCaseToTemplate(caseData)
    const result = await tauriCallSafe('render_docsy_template', {
      args: {
        templatePath,
        outputPath,
        values,
        structureOverrides: {},
      },
    })
    return result
  }

  /// 批量生成
  async function batchGenerate(templatePath, cases, outputDir) {
    // 导出 Excel → 填写 → 批量生成
    const exportResult = await tauriCallSafe('export_docsy_fields_xlsx', {
      templatePath,
      outputPath: `${outputDir}/batch-fields.xlsx`,
      defaultValues: cases.length > 0 ? mapCaseToTemplate(cases[0]) : {},
    })
    return exportResult
  }

  return { listTemplates, generateDocument, batchGenerate }
}

function mapCaseToTemplate(caseData) {
  return {
    // 按字段名映射
    '法院': caseData.court || '',
    '案号': caseData.caseNo || '',
    '原告': caseData.clientName || '',
    '被告': caseData.opponentName || '',
    '案由': caseData.causeAction || '',
    '日期': new Date().toISOString().split('T')[0],
    '律所名称': '',  // 从设置读取
    '律师': (caseData.attorneys || []).join('、'),
    '诉讼阶段': caseData.caseLevel || '',
    // 按 semantic key 映射
    '当事人': [caseData.clientName, caseData.opponentName].filter(Boolean),
  }
}
```

### 5.2 Docsy Tauri 命令

```rust
// src-tauri/src/commands/docsy_bridge.rs

#[tauri::command]
pub async fn list_docsy_templates() -> Result<Vec<serde_json::Value>, String> {
    run_blocking(|| {
        let templates_dir = docsy_template_dir()?;
        let mut templates = Vec::new();
        for entry in std::fs::read_dir(&templates_dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map(|e| e == "docsytpl").unwrap_or(false) {
                if let Ok(manifest) = read_docsytpl_manifest(&path) {
                    templates.push(serde_json::json!({
                        "path": path.display().to_string(),
                        "name": manifest.template.name,
                        "fieldCount": manifest.fields.len(),
                    }));
                }
            }
        }
        Ok(templates)
    }).await
}

#[tauri::command]
pub async fn render_docsy_template(args: serde_json::Value) -> Result<String, String> {
    run_blocking(move || {
        // 调用 Docsy 的渲染引擎
        // 需要将 Docsy 的 docx_template 模块作为依赖引入
        let args: RenderTemplateArgs = serde_json::from_value(args)
            .map_err(|e| e.to_string())?;
        // ... 调用 render_docx
        Ok(args.output_path)
    }).await
}
```

## 6. 关键 UI 组件

### 6.1 案件列表行颜色

```css
/* 根据期限状态着色 */
.case-row-red { background-color: #fef2f2; }
.case-row-yellow { background-color: #fefce8; }
.case-row-gray { color: #9ca3af; }
```

### 6.2 日历事件颜色

```css
.event-invalidation { background-color: #3b82f6; }  /* 蓝：无效口审 */
.event-court { background-color: #ef4444; }          /* 红：法院开庭 */
.event-appeal { background-color: #eab308; }         /* 黄：二审 */
.event-deadline { background-color: #f97316; }       /* 橙：期限 */
.event-task { background-color: #8b5cf6; }           /* 紫：任务 */
```

### 6.3 收件箱 AI 置信度指示

```css
.confidence-high { color: #22c55e; }    /* > 0.8 */
.confidence-medium { color: #eab308; }  /* 0.5-0.8 */
.confidence-low { color: #ef4444; }     /* < 0.5 */
```
