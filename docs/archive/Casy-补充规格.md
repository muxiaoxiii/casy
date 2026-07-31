# Casy 补充规格 — 组件规格 + 映射函数 + 飞书补全 + 防抖策略

> **版本**: v1.0
> **日期**: 2026-07-30
> **说明**: 补充 `Casy-完整规划文档.md` 及五份实现规格文档中的缺口

---

## 一、Vue 组件规格（14 个组件）

### 1.1 CaseFilterBar.vue

**用途**: 案件列表顶部的多条件筛选栏

| 项目 | 内容 |
|------|------|
| **Props** | `modelValue: { track, client, court, status, search, sortBy, groupBy }` |
| **Emits** | `update:modelValue`, `search`, `reset` |
| **数据源** | `casesStore.stats`（按 track/client/court 分组统计）；`casesStore.loadCases()` |
| **关键计算** | `trackOptions` / `clientOptions` / `courtOptions` / `statusOptions` — 从 `stats.byTrack` 等派生 |
| **事件** | 筛选项变化 → emit `update:modelValue` → 父组件调 `loadCases()`；搜索框 `@keyup.enter` 触发 `search` |

```vue
<template>
  <div class="case-filter-bar">
    <el-select v-model="local.track" clearable placeholder="全部轨道" @change="emit">
      <el-option v-for="o in trackOptions" :key="o.value" :label="o.label" :value="o.value" />
    </el-select>
    <el-select v-model="local.client" clearable placeholder="全部客户" @change="emit">
      <el-option v-for="o in clientOptions" :key="o" :label="o" :value="o" />
    </el-select>
    <el-select v-model="local.court" clearable placeholder="全部法院" @change="emit">
      <el-option v-for="o in courtOptions" :key="o" :label="o" :value="o" />
    </el-select>
    <el-select v-model="local.status" clearable placeholder="全部状态" @change="emit">
      <el-option label="进行中" value="进行中" /><el-option label="已完结" value="已完结" />
    </el-select>
    <el-input v-model="local.search" clearable placeholder="搜索案号/名称/客户..." @keyup.enter="$emit('search')" />
    <el-select v-model="local.sortBy" @change="emit">
      <el-option label="立案日期" value="filing_date" /><el-option label="更新时间" value="updated_at" />
      <el-option label="案号" value="case_no" /><el-option label="客户" value="client_name" />
    </el-select>
    <el-select v-model="local.groupBy" @change="emit">
      <el-option label="不分组" value="" /><el-option label="按案由" value="cause_action" />
      <el-option label="按客户" value="client_name" /><el-option label="按审理机关" value="court" />
      <el-option label="按轨道" value="track" />
    </el-select>
    <span class="total">共 {{ total }} 件</span>
  </div>
</template>
```

---

### 1.2 CaseGroupPanel.vue

**用途**: 可折叠的分组面板，含组名 + 计数 badge + 案件行插槽

| 项目 | 内容 |
|------|------|
| **Props** | `title: String`, `count: Number`, `defaultOpen: Boolean (true)` |
| **Emits** | `toggle(open: Boolean)` |
| **数据源** | 父组件传入的分组数据 |
| **关键计算** | `isOpen` — 本地 ref，控制折叠状态 |
| **模板结构** | `el-collapse-transition` + header 行（展开箭头 + title + `el-badge`）+ 默认插槽渲染案件行 |

```vue
<template>
  <div class="case-group-panel">
    <div class="group-header" @click="isOpen = !isOpen">
      <el-icon><ArrowRight v-if="!isOpen" /><ArrowDown v-else /></el-icon>
      <span class="title">{{ title }}</span>
      <el-badge :value="count" type="info" />
    </div>
    <el-collapse-transition>
      <div v-show="isOpen" class="group-body">
        <slot />
      </div>
    </el-collapse-transition>
  </div>
</template>
```

---

### 1.3 CaseInfoPanel.vue

**用途**: 案件详情左栏 — 可编辑字段，按分组折叠

| 项目 | 内容 |
|------|------|
| **Props** | `caseData: Object`（双向绑定或通过 store） |
| **Emits** | `update(field, value)` — 字段级更新，触发自动保存 |
| **数据源** | `casesStore.currentCase`（通过 `loadCase(id)` 加载） |
| **关键计算** | `sections` — 分组定义：`{ basic: [case_name, case_no, internal_no, cause_action], parties: [client_name, our_role, opponent_name, opponent_role, opponent_firm, opponent_agent], court: [court, judge_panel, clerk, attorneys, case_level], patent: [patent_name, patent_app_no, procedure_type], dates: [filing_date, trial_date, verdict_date, ...], result: [case_progress, case_result, notes] }` |
| **模板结构** | 每个 section 一个 `el-collapse-item`；字段类型：文本→`el-input`，日期→`el-date-picker`，选择→`el-select`，多选→`el-select multiple`，备注→`el-input type="textarea"` |

```vue
<template>
  <div class="case-info-panel">
    <el-collapse v-model="activeSections">
      <el-collapse-item v-for="sec in sections" :key="sec.key" :name="sec.key" :title="sec.label">
        <el-form label-width="80px" label-position="left">
          <el-form-item v-for="f in sec.fields" :key="f.key" :label="f.label">
            <component :is="f.component" v-model="form[f.key]" v-bind="f.props"
              @change="$emit('update', f.key, form[f.key])" />
          </el-form-item>
        </el-form>
      </el-collapse-item>
    </el-collapse>
  </div>
</template>
```

---

### 1.4 CaseTimelinePanel.vue

**用途**: 案件详情中栏 — 合并时间线（日志 + 庭审 + 任务）

| 项目 | 内容 |
|------|------|
| **Props** | `caseId: String` |
| **Emits** | `addEvent`, `editEvent(event)`, `deleteEvent(event)` |
| **数据源** | `tauriCallSafe('case_timeline', { caseId })` → `TimelineEvent[]`；或 `timelineStore.loadTimeline(caseId)` |
| **关键计算** | `groupedByMonth` — 按 YYYY-MM 分组的事件 map |
| **模板结构** | 顶部工具栏（`[添加事件 +]` + 类型筛选 + 排序切换）；每月一个分组标题；每条事件：图标 + 日期 + 标题 + 详情展开 + 操作按钮 |

```vue
<template>
  <div class="case-timeline-panel">
    <div class="toolbar">
      <el-button size="small" @click="$emit('addEvent')">+ 添加事件</el-button>
      <el-select v-model="typeFilter" size="small" clearable placeholder="全部类型">
        <el-option v-for="t in eventTypes" :key="t.value" :label="t.label" :value="t.value" />
      </el-select>
    </div>
    <div v-for="(events, month) in groupedByMonth" :key="month" class="month-group">
      <div class="month-header">{{ month }}</div>
      <div v-for="evt in events" :key="evt.id" class="timeline-item">
        <span class="icon">{{ evt.icon }}</span>
        <span class="date">{{ evt.eventDate }}</span>
        <span class="title">{{ evt.title }}</span>
        <div v-if="evt.detail" class="detail">{{ evt.detail }}</div>
      </div>
    </div>
    <el-empty v-if="!events.length" description="还没有事件记录" />
  </div>
</template>
```

---

### 1.5 CaseRelatedPanel.vue

**用途**: 案件详情右栏 — 期限 + 关联案件 + 联系人 + 快捷操作

| 项目 | 内容 |
|------|------|
| **Props** | `caseId: String`, `caseData: Object` |
| **Emits** | `navigateCase(caseId)`, `generateDoc`, `openFolder`, `openFiles` |
| **数据源** | 期限：`tauriCallSafe('deadline_for_case', { caseId })`；关联：`tauriCallSafe('case_relations', { caseId })`；联系人：从 `caseData.judge_panel` / `caseData.clerk` 提取 |
| **关键计算** | `deadlines` — 期限预警列表，按 daysLeft 排序；`relations` — 按 relationType 分组 |
| **模板结构** | 四个区块：`⏰ 期限`（红色/黄色 badge）、`🔗 关联案件`（按类型分组 + 跳转链接）、`👤 联系人`（法官/书记员/对方代理）、`⚡ 快捷操作`（生成文书 / 打开文件夹 / 查看案卷） |

```vue
<template>
  <div class="case-related-panel">
    <div class="section">
      <h4>⏰ 期限</h4>
      <div v-for="d in deadlines" :key="d.ruleName" :class="urgencyClass(d.urgency)">
        {{ d.ruleName }}: {{ d.dueDate }} ({{ d.daysLeft }}天)
      </div>
      <el-empty v-if="!deadlines.length" description="无期限" :image-size="40" />
    </div>
    <div class="section">
      <h4>🔗 关联案件</h4>
      <div v-for="(group, type) in groupedRelations" :key="type">
        <div class="rel-type">{{ relationLabel(type) }}</div>
        <div v-for="r in group" :key="r.id" class="rel-item" @click="$emit('navigateCase', r.targetCase.id)">
          {{ r.targetCase.caseName }} {{ r.targetCase.caseStatus }}
        </div>
      </div>
    </div>
    <div class="section"><h4>👤 联系人</h4> ... </div>
    <div class="section"><h4>⚡ 快捷操作</h4>
      <el-button @click="$emit('generateDoc')">生成文书</el-button>
      <el-button @click="$emit('openFolder')">打开文件夹</el-button>
      <el-button @click="$emit('openFiles')">查看案卷</el-button>
    </div>
  </div>
</template>
```

---

### 1.6 CaseNetworkView.vue

**用途**: 案件关系的列表式可视化（非图形库），按关系类型分层展示

| 项目 | 内容 |
|------|------|
| **Props** | `caseId: String`, `depth: Number (2)` |
| **Emits** | `navigateCase(caseId)` |
| **数据源** | `tauriCallSafe('case_relation_tree', { caseId, depth })` — 递归查 case_relations |
| **关键计算** | `tree` — 树形结构 `{ case, relations: [{ type, children: [...] }] }`；`flatList` — 展平后的节点列表，带缩进层级 |
| **模板结构** | 根节点（当前案件）居中；一级关联以缩进列表展示，每条显示 caseName + relationLabel + status badge；二级关联再缩进；点击任意节点跳转 |

```vue
<template>
  <div class="case-network-view">
    <div class="root-node">{{ root.caseName }}</div>
    <div v-for="node in flatList" :key="node.id" :style="{ paddingLeft: node.level * 24 + 'px' }"
         class="network-node" @click="$emit('navigateCase', node.id)">
      <span class="rel-label">{{ node.relLabel }}</span>
      <span class="case-name">{{ node.caseName }}</span>
      <el-tag size="small">{{ node.status }}</el-tag>
    </div>
  </div>
</template>
```

---

### 1.7 DeadlinePanel.vue

**用途**: 全局期限预警面板，按紧急度排序，点击跳转案件

| 项目 | 内容 |
|------|------|
| **Props** | `limit: Number (20)`, `urgencyFilter: String ('all')` |
| **Emits** | `navigateCase(caseId)` |
| **数据源** | `tauriCallSafe('deadline_warnings', {})` → `DeadlineResult[]`（期限引擎输出） |
| **关键计算** | `sorted` — 按 daysLeft 升序；`filtered` — 按 urgencyFilter 过滤；`grouped` — 按 urgency 分组（Red/Yellow/Green） |
| **模板结构** | 顶部统计（N 条红色 / M 条黄色）；列表项：圆点颜色 + ruleName + caseName + dueDate + "还剩 N 天"；已过期显示"已逾期 N 天"红色 |

```vue
<template>
  <div class="deadline-panel">
    <div class="stats">
      <el-tag type="danger">{{ redCount }} 紧急</el-tag>
      <el-tag type="warning">{{ yellowCount }} 关注</el-tag>
    </div>
    <div v-for="d in filtered" :key="d.caseId + d.ruleName"
         :class="['deadline-item', d.urgency]"
         @click="$emit('navigateCase', d.caseId)">
      <span class="dot" /><span class="rule">{{ d.ruleName }}</span>
      <span class="case">{{ d.caseName }}</span>
      <span class="date">{{ d.dueDate }}</span>
      <span class="days">{{ d.daysLeft > 0 ? `还剩${d.daysLeft}天` : `已逾期${-d.daysLeft}天` }}</span>
    </div>
  </div>
</template>
```

---

### 1.8 TaskDetailPanel.vue

**用途**: 任务编辑抽屉/模态框

| 项目 | 内容 |
|------|------|
| **Props** | `task: Object (null)` — null 为新建模式；`visible: Boolean` |
| **Emits** | `update:visible`, `saved(task)`, `deleted(taskId)` |
| **数据源** | 内部 `form` reactive 对象；保存时调 `tasksStore.createTask(data)` 或 `tauriCallSafe('update_task', { id, data })` |
| **关键计算** | `isNew` — `!props.task?.id`；`caseOptions` — `casesStore.cases` 映射为下拉选项 |
| **模板结构** | `el-drawer` 包裹 `el-form`：任务名称（必填）、关联案件（下拉搜索）、截止日期、优先级（四象限选择器）、描述、指派人；底部：保存 / 删除按钮 |

```vue
<template>
  <el-drawer :model-value="visible" @update:model-value="$emit('update:visible', $event)"
             :title="isNew ? '新建任务' : '编辑任务'" size="400px">
    <el-form :model="form" label-width="80px">
      <el-form-item label="任务名称" required><el-input v-model="form.taskName" /></el-form-item>
      <el-form-item label="关联案件"><el-select v-model="form.caseId" filterable clearable>
        <el-option v-for="c in caseOptions" :key="c.id" :label="c.caseName" :value="c.id" />
      </el-select></el-form-item>
      <el-form-item label="截止日期"><el-date-picker v-model="form.deadline" /></el-form-item>
      <el-form-item label="优先级"><el-select v-model="form.priority">
        <el-option label="重要紧急" value="urgent_important" /><el-option label="重要" value="important" />
        <el-option label="紧急" value="urgent" /><el-option label="普通" value="normal" />
      </el-select></el-form-item>
      <el-form-item label="描述"><el-input v-model="form.description" type="textarea" /></el-form-item>
      <el-form-item label="指派人"><el-input v-model="form.assignee" /></el-form-item>
    </el-form>
    <template #footer>
      <el-button v-if="!isNew" type="danger" @click="handleDelete">删除</el-button>
      <el-button type="primary" @click="handleSave">保存</el-button>
    </template>
  </el-drawer>
</template>
```

---

### 1.9 InboxView.vue

**用途**: 统一收件箱 — 待处理/已归档标签页，含 AI 分类结果

| 项目 | 内容 |
|------|------|
| **Props** | 无（路由级组件） |
| **Emits** | 无 |
| **数据源** | `inboxStore.loadItems()` → `inboxStore.pending` / `inboxStore.filed`；处理：`inboxStore.processItem(id)`；归档：`inboxStore.fileToCase(itemId, caseId, category)` |
| **关键计算** | `pendingItems` / `filedItems` — 从 store getter 获取；`selectedItem` — 当前选中项，右侧展示详情 |
| **模板结构** | 顶部标签页（待处理 N / 已归档 N）+ 添加按钮（文件/笔记/邮件）；左侧列表（卡片式：标题 + AI 分类 badge + 置信度颜色条 + 时间）；右侧详情面板（文本预览 + AI 提取结果 + 案件匹配建议 + 归档操作） |

```vue
<template>
  <div class="inbox-view">
    <div class="inbox-header">
      <el-tabs v-model="activeTab">
        <el-tab-pane label="待处理" name="pending">
          <span slot="label">待处理 <el-badge :value="pendingCount" /></span>
        </el-tab-pane>
        <el-tab-pane label="已归档" name="filed" />
      </el-tabs>
      <el-button-group>
        <el-button @click="addFile">📎 添加文件</el-button>
        <el-button @click="addNote">📝 新建笔记</el-button>
      </el-button-group>
    </div>
    <div class="inbox-body">
      <div class="inbox-list">
        <div v-for="item in currentList" :key="item.id" :class="['inbox-card', { active: selected?.id === item.id }]"
             @click="selected = item">
          <div class="title">{{ item.title || '无标题' }}</div>
          <div class="meta">
            <el-tag size="small" :type="categoryType(item.aiCategory)">{{ item.aiCategory || '未分类' }}</el-tag>
            <span :class="confidenceClass(item.aiConfidence)">{{ (item.aiConfidence * 100).toFixed(0) }}%</span>
            <span class="time">{{ item.createdAt }}</span>
          </div>
        </div>
      </div>
      <div class="inbox-detail" v-if="selected">
        <pre class="content-preview">{{ selected.contentText }}</pre>
        <div class="ai-result" v-if="selected.aiExtracted">
          <h4>AI 提取结果</h4>
          <pre>{{ selected.aiExtracted }}</pre>
        </div>
        <div class="actions">
          <el-select v-model="targetCaseId" filterable placeholder="选择归档案件">
            <el-option v-for="c in cases" :key="c.id" :label="c.caseName" :value="c.id" />
          </el-select>
          <el-select v-model="fileCategory" placeholder="文件分类">
            <el-option v-for="c in categories" :key="c" :label="c" :value="c" />
          </el-select>
          <el-button type="primary" @click="handleFile">归档</el-button>
          <el-button @click="handleDismiss">忽略</el-button>
        </div>
      </div>
    </div>
  </div>
</template>
```

---

### 1.10 SyncStatusView.vue

**用途**: 同步状态页 — WebDAV + 飞书状态，手动同步按钮

| 项目 | 内容 |
|------|------|
| **Props** | 无（路由级组件） |
| **Emits** | 无 |
| **数据源** | `tauriCallSafe('sync_status', {})` → `{ webdav: { lastSync, etag, status }, feishu: { lastSync, status, pendingPushes } }`；手动同步：`tauriCallSafe('manual_sync', { source })` |
| **关键计算** | `webdavStatus` / `feishuStatus` — 响应式状态对象；`isSyncing` — 全局同步中标记 |
| **模板结构** | 两张卡片（WebDAV / 飞书）：同步状态指示灯（绿/黄/红）+ 最后同步时间 + 待推送数量 + [立即同步] 按钮；同步历史列表（最近 10 条，方向箭头 + 时间 + 结果） |

```vue
<template>
  <div class="sync-status-view">
    <el-row :gutter="16">
      <el-col :span="12">
        <el-card>
          <template #header>WebDAV 同步</template>
          <div class="status-line">
            <span :class="['dot', webdavStatus.status]" />
            <span>{{ webdavStatusLabel }}</span>
          </div>
          <div>最后同步: {{ webdavStatus.lastSync || '从未同步' }}</div>
          <el-button :loading="syncing.webdav" @click="manualSync('webdav')">立即同步</el-button>
        </el-card>
      </el-col>
      <el-col :span="12">
        <el-card>
          <template #header>飞书同步</template>
          <div class="status-line">
            <span :class="['dot', feishuStatus.status]" />
            <span>{{ feishuStatusLabel }}</span>
          </div>
          <div>最后同步: {{ feishuStatus.lastSync || '从未同步' }}</div>
          <div>待推送: {{ feishuStatus.pendingPushes }} 条</div>
          <el-button :loading="syncing.feishu" @click="manualSync('feishu')">立即同步</el-button>
        </el-card>
      </el-col>
    </el-row>
    <el-card class="sync-history">
      <template #header>同步历史</template>
      <div v-for="h in history" :key="h.id" class="history-item">
        <span>{{ h.direction === 'push' ? '↑' : '↓' }}</span>
        <span>{{ h.source }}</span>
        <span>{{ h.time }}</span>
        <el-tag :type="h.success ? 'success' : 'danger'" size="small">{{ h.message }}</el-tag>
      </div>
    </el-card>
  </div>
</template>
```

---

### 1.11 ConflictResolver.vue

**用途**: 同步冲突的并排对比解决器

| 项目 | 内容 |
|------|------|
| **Props** | `conflict: { localData, remoteData, conflictFields, tableName, recordId }` |
| **Emits** | `resolve(recordId, choice: 'local' | 'remote' | 'merged', mergedData?)` |
| **数据源** | 父组件传入冲突对象（来自 `sync_map.conflict_fields`） |
| **关键计算** | `diffFields` — 解析 `conflictFields` JSON，得到 `[{ field, localValue, remoteValue, isDifferent }]`；`merged` — 用户逐字段选择后的合并结果 |
| **模板结构** | 标题行（表名 + 记录 ID）；表格三列：字段名 / 本地值 / 远程值，不同字段高亮；每个字段行有单选：保留本地 / 保留远程；底部：[保留本地全部] / [保留远程全部] / [应用合并] |

```vue
<template>
  <el-dialog :model-value="!!conflict" title="解决同步冲突" width="800px">
    <div class="conflict-header">
      <span>表: {{ conflict.tableName }}</span>
      <span>记录: {{ conflict.recordId }}</span>
    </div>
    <el-table :data="diffFields" border>
      <el-table-column prop="field" label="字段" width="160" />
      <el-table-column label="本地值">
        <template #default="{ row }">
          <div :class="{ different: row.isDifferent }">{{ row.localValue }}</div>
        </template>
      </el-table-column>
      <el-table-column label="远程值">
        <template #default="{ row }">
          <div :class="{ different: row.isDifferent }">{{ row.remoteValue }}</div>
        </template>
      </el-table-column>
      <el-table-column label="选择" width="200">
        <template #default="{ row }">
          <el-radio-group v-model="row.choice" size="small">
            <el-radio-button label="local">本地</el-radio-button>
            <el-radio-button label="remote">远程</el-radio-button>
          </el-radio-group>
        </template>
      </el-table-column>
    </el-table>
    <template #footer>
      <el-button @click="resolveAll('local')">保留本地全部</el-button>
      <el-button @click="resolveAll('remote')">保留远程全部</el-button>
      <el-button type="primary" @click="applyMerged">应用合并</el-button>
    </template>
  </el-dialog>
</template>
```

---

### 1.12 TemplateBrowser.vue

**用途**: 文书工坊的模板列表浏览

| 项目 | 内容 |
|------|------|
| **Props** | `templates: Array<{ path, name, fieldCount }>` |
| **Emits** | `select(template)` |
| **数据源** | `useDocsyBridge().listTemplates()` → `tauriCallSafe('list_docsy_templates', {})` |
| **关键计算** | `searchText` — 本地搜索过滤；`filtered` — `templates.filter(t => t.name.includes(searchText))` |
| **模板结构** | 搜索框 + 卡片网格：每张卡片显示模板名称 + 字段数量 badge + 点击选中高亮；选中后 emit `select` |

```vue
<template>
  <div class="template-browser">
    <el-input v-model="searchText" placeholder="搜索模板..." clearable prefix-icon="Search" />
    <div class="template-grid">
      <div v-for="t in filtered" :key="t.path" :class="['template-card', { active: selected?.path === t.path }]"
           @click="selected = t; $emit('select', t)">
        <div class="name">{{ t.name }}</div>
        <div class="meta">{{ t.fieldCount }} 个字段</div>
      </div>
    </div>
  </div>
</template>
```

---

### 1.13 DocumentGenView.vue

**用途**: 文书生成页 — 模板选择 + 字段预览 + 生成按钮

| 项目 | 内容 |
|------|------|
| **Props** | 无（路由级组件） |
| **Emits** | 无 |
| **数据源** | 模板：`useDocsyBridge().listTemplates()`；案件：`casesStore.cases`；生成：`useDocsyBridge().generateDocument(templatePath, caseData, outputPath)` |
| **关键计算** | `selectedTemplate` — 当前选中模板；`selectedCase` — 当前选中案件；`previewValues` — `mapCaseToTemplate(selectedCase)` 的结果；`fieldList` — 模板字段列表（从模板 manifest 解析） |
| **模板结构** | 左侧：`TemplateBrowser` 组件；右侧上部：案件选择下拉 + 字段预览表格（字段名 → 映射值）；右侧下部：[选择输出路径] + [生成文书] 按钮；批量模式切换：单个/批量（导出 Excel → 填写 → 批量生成） |

```vue
<template>
  <div class="document-gen-view">
    <el-row :gutter="16">
      <el-col :span="8">
        <TemplateBrowser :templates="templates" @select="selectedTemplate = $event" />
      </el-col>
      <el-col :span="16">
        <el-card>
          <el-select v-model="selectedCaseId" filterable placeholder="选择案件">
            <el-option v-for="c in cases" :key="c.id" :label="c.caseName" :value="c.id" />
          </el-select>
          <el-table v-if="previewValues" :data="fieldRows" border size="small">
            <el-table-column prop="field" label="模板字段" />
            <el-table-column prop="value" label="映射值" />
          </el-table>
          <el-divider />
          <el-button type="primary" :disabled="!selectedTemplate || !selectedCaseId" @click="handleGenerate">
            生成文书
          </el-button>
          <el-button @click="handleBatch">批量生成</el-button>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>
```

---

### 1.14 WritingView.vue

**用途**: TipTap 编辑器 + 案件数据侧栏

| 项目 | 内容 |
|------|------|
| **Props** | 路由参数 `caseId?: String` |
| **Emits** | 无 |
| **数据源** | 编辑器内容：`draftsStore`（草稿 CRUD）；案件数据：`casesStore.loadCase(caseId)` 传给编辑器的 `caseData`；保存：`tauriCallSafe('save_draft', { ... })` |
| **关键计算** | `caseData` — 当前关联案件数据，传给 `LegalEditor` 的 `caseData` prop 供 suggestion 使用；`autoSaveTimer` — 2 秒防抖自动保存 |
| **模板结构** | 左侧 `LegalEditor`（TipTap 编辑器 + 工具栏）；右侧案件信息侧栏（可折叠）：当前案件字段摘要 + 快捷插入按钮（{字段 / 【法条 / @当事人）；底部状态栏：字数统计 + 最后保存时间 + 导出 Word 按钮 |

```vue
<template>
  <div class="writing-view">
    <div class="editor-main">
      <LegalEditor v-model="content" :case-data="caseData" @update:model-value="scheduleSave" />
    </div>
    <div class="editor-sidebar" v-if="caseData">
      <h4>案件信息</h4>
      <div class="field-summary">
        <div><strong>案号:</strong> {{ caseData.caseNo }}</div>
        <div><strong>客户:</strong> {{ caseData.clientName }}</div>
        <div><strong>对方:</strong> {{ caseData.opponentName }}</div>
        <div><strong>审理机关:</strong> {{ caseData.court }}</div>
      </div>
      <el-divider />
      <el-button size="small" @click="insertField">{ 插入字段</el-button>
      <el-button size="small" @click="insertLaw">【 插入法条</el-button>
      <el-button size="small" @click="insertParty">@ 插入当事人</el-button>
    </div>
    <div class="editor-statusbar">
      <span>字数: {{ wordCount }}</span>
      <span>最后保存: {{ lastSaved }}</span>
      <el-button size="small" @click="exportWord">导出 Word</el-button>
    </div>
  </div>
</template>
```

---

## 二、mapCaseToTemplate 完整映射函数

将 Casy 案件数据转换为 Docsy 模板 values 对象。Docsy 字段类型：text / date / select / party_list / reference / checkbox / radio_group / checkbox_group / delete_text。

### 2.1 映射函数

```javascript
// src/modules/documents/composables/mapCaseToTemplate.js

/**
 * 将 Casy 案件数据映射为 Docsy 模板 values
 * @param {Object} caseData - 案件对象（来自 casesStore.currentCase）
 * @param {Object} settings - 设置（律所名称等）
 * @param {Object} templateManifest - 模板 manifest（含字段定义）
 * @returns {Object} Docsy values 对象，key 为模板字段名
 */
export function mapCaseToTemplate(caseData, settings = {}, templateManifest = null) {
  if (!caseData) return {}

  const values = {}

  // ---- 文本字段 → 简单字符串 ----
  values['法院'] = caseData.court || ''
  values['案号'] = caseData.caseNo || ''
  values['案件名称'] = caseData.caseName || ''
  values['案由'] = caseData.causeAction || ''
  values['内部卷号'] = caseData.internalNo || ''
  values['专利名称'] = caseData.patentName || ''
  values['专利申请号'] = caseData.patentAppNo || ''
  values['诉讼阶段'] = caseData.caseLevel || ''
  values['案件进展'] = caseData.caseProgress || ''
  values['案件结果'] = caseData.caseResult || ''
  values['备注'] = caseData.notes || ''
  values['律所名称'] = settings.firmName || ''
  values['律师'] = (caseData.attorneys || []).join('、')

  // ---- 日期字段 → YYYY-MM-DD 字符串 ----
  const dateFields = {
    '立案日期': 'filingDate',
    '收到起诉状日期': 'complaintReceivedDate',
    '开庭日期': 'trialDate',
    '二审日期': 'trial2Date',
    '三审日期': 'trial3Date',
    '判决日期': 'verdictDate',
    '中止日期': 'stayDate',
    '救济期限': 'reliefDeadline',
    '请求人首次无效日期': 'petitionerFirstInvalid',
    '请求人补充意见期限': 'petitionerSuppDeadline',
    '请求人提交日期': 'petitionerSubmitDate',
    '请求人收到日期': 'petitionerReceivedDate',
    '请求人答复期限': 'petitionerReplyDeadline',
    '专利权人收到日期': 'patenteeReceivedDate',
    '专利权人陈述期限': 'patenteeStatementDeadline',
    '专利权人收到补充日期': 'patenteeReceivedSuppDate',
    '专利权人补充期限': 'patenteeSuppDeadline',
    '专利权人提交补充日期': 'patenteeSubmitSuppDate',
  }
  for (const [tplField, caseKey] of Object.entries(dateFields)) {
    values[tplField] = caseData[caseKey] ? formatDateStr(caseData[caseKey]) : ''
  }

  // 今日日期（不从 caseData 取值，直接用当前日期）
  values['日期'] = formatDate(new Date())
  values['今日日期'] = formatDate(new Date())

  // ---- party_list 字段 → [{name, suffix}] 数组 ----
  // 我方当事人
  const ourParties = []
  if (caseData.clientName) {
    ourParties.push({
      name: caseData.clientName,
      suffix: caseData.ourRole || '请求人',
    })
  }
  values['我方当事人'] = ourParties

  // 对方当事人
  const opponentParties = []
  if (caseData.opponentName) {
    opponentParties.push({
      name: caseData.opponentName,
      suffix: caseData.opponentRole || '被请求人',
    })
  }
  // 对方代理可能有多人
  if (caseData.opponentAgent) {
    opponentParties.push({
      name: caseData.opponentAgent,
      suffix: '代理人',
    })
  }
  values['对方当事人'] = opponentParties

  // 合并当事人列表（供模板中"当事人"字段使用）
  values['当事人'] = [...ourParties, ...opponentParties]

  // ---- reference 字段 → 解析后的值 ----
  // reference 字段在 Docsy 中表示从字典/列表中引用的值
  // 当用户设置了 structureOverride 时，替换模板原文前缀
  values['审理机关'] = caseData.court || ''   // reference → 直接用名称
  values['审级'] = caseData.caseLevel || ''   // reference → 直接用选项文本
  values['对方代理律所'] = caseData.opponentFirm || ''

  // ---- checkbox/radio 字段 → boolean 或选中项 ----
  // 诉讼程序类型
  values['普通程序'] = caseData.procedureType === '普通'
  values['简易程序'] = caseData.procedureType === '简易'

  // 判决类型（radio_group）
  values['判决类型'] = caseData.verdictType || ''  // 如 "判决" / "裁定" / "决定"

  // 裁判结果（radio_group）
  values['胜诉'] = caseData.caseResult === '胜诉'
  values['败诉'] = caseData.caseResult === '败诉'
  values['部分胜诉'] = caseData.caseResult === '部分胜诉'

  // ---- 多个 mark_refs 处理 ----
  // Docsy 允许同一字段在模板不同位置出现多次（mark_ref）
  // 每个 mark_ref 有唯一 position 标识
  // 渲染时 Docsy 会按 position 逐一替换，值相同
  // Casy 只需提供一次值，Docsy 渲染器自动处理多处替换
  // 如果需要不同位置填不同值（如日期格式不同），则：
  // values['立案日期_长格式'] = formatDateLong(caseData.filingDate)
  // values['立案日期_短格式'] = formatDateShort(caseData.filingDate)

  // 清理空值（避免模板中出现 undefined）
  for (const key of Object.keys(values)) {
    if (values[key] === undefined || values[key] === null) {
      values[key] = typeof values[key] === 'boolean' ? false : ''
    }
  }

  return values
}

// ---- 辅助函数 ----

function formatDate(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function formatDateStr(s) {
  // 已经是 YYYY-MM-DD 格式则直接返回
  if (/^\d{4}-\d{2}-\d{2}/.test(s)) return s.slice(0, 10)
  // 飞书时间戳（毫秒）
  if (/^\d{13}$/.test(s)) {
    return formatDate(new Date(Number(s)))
  }
  return s
}

function formatDateLong(s) {
  // 2024年6月15日
  const d = formatDateStr(s)
  const [y, m, day] = d.split('-')
  return `${y}年${parseInt(m)}月${parseInt(day)}日`
}
```

### 2.2 多 mark_refs 处理策略

Docsy 模板中同一字段名可在不同位置出现多次（如"案号"在标题和正文中各出现一次）。渲染逻辑：

1. Docsy 渲染器遍历所有 `<w:t>` 节点，找到占位符后用 values 中的值替换
2. 同一 key 出现多次时，每次替换用相同的值
3. 如果模板中有 `{{案号:标题}}` 和 `{{案号:正文}}` 两个不同 position 标记的同一字段，Docsy 会分别查找 `values['案号']`
4. Casy 侧只需提供一个 `values['案号']` 即可，不需要特殊处理

如果确实需要不同位置用不同格式（如长日期 vs 短日期），则使用不同的字段名：

```javascript
// 在模板中使用 {{立案日期_长}} 和 {{立案日期_短}}
values['立案日期_长'] = '2024年6月15日'
values['立案日期_短'] = '2024-06-15'
```

---

## 三、飞书认证流程

### 3.1 获取 tenant_access_token

飞书自建应用使用 App ID + App Secret 获取 tenant_access_token，有效期 2 小时。

```rust
// src-tauri/src/sync/feishu_auth.rs

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const TOKEN_URL: &str = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";

#[derive(Debug, Serialize, Deserialize)]
struct TokenRequest {
    app_id: String,
    app_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    code: i64,
    msg: String,
    tenant_access_token: String,
    expire: u64,  // 秒，通常 7200
}

/// 飞书令牌管理器
pub struct FeishuAuth {
    app_id: String,
    app_secret: String,
    token: Option<String>,
    expires_at: Option<Instant>,
    client: reqwest::blocking::Client,
}

impl FeishuAuth {
    /// 从 OS keychain 读取 App ID 和 App Secret，创建实例
    pub fn from_keychain() -> Result<Self> {
        let keyring = keyring::Entry::new("Casy", "feishu")?;
        let secret_json = keyring.get_password()
            .map_err(|_| anyhow::anyhow!("飞书凭据未配置，请在设置页填写"))?;
        let secret: FeishuSecret = serde_json::from_str(&secret_json)?;

        Ok(Self {
            app_id: secret.app_id,
            app_secret: secret.app_secret,
            token: None,
            expires_at: None,
            client: reqwest::blocking::Client::new(),
        })
    }

    /// 将 App ID / App Secret 存入 OS keychain
    pub fn save_to_keychain(app_id: &str, app_secret: &str) -> Result<()> {
        let keyring = keyring::Entry::new("Casy", "feishu")?;
        let secret = FeishuSecret {
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
        };
        keyring.set_password(&serde_json::to_string(&secret)?)?;
        Ok(())
    }

    /// 获取有效的 tenant_access_token（自动刷新）
    pub fn get_token(&mut self) -> Result<&str> {
        // 检查是否需要刷新：没有 token，或已过期（提前 5 分钟刷新）
        let needs_refresh = match (&self.token, &self.expires_at) {
            (None, _) => true,
            (_, Some(exp)) => exp.elapsed() > Duration::from_secs(5 * 60), // 提前 5 分钟
            _ => true,
        };

        if needs_refresh {
            self.refresh_token()?;
        }

        Ok(self.token.as_ref().unwrap())
    }

    /// 刷新 token
    fn refresh_token(&mut self) -> Result<()> {
        let resp = self.client.post(TOKEN_URL)
            .json(&TokenRequest {
                app_id: self.app_id.clone(),
                app_secret: self.app_secret.clone(),
            })
            .send()?;

        let status = resp.status();
        let body: TokenResponse = resp.json()?;

        if body.code != 0 || status != 200 {
            return Err(anyhow::anyhow!(
                "飞书认证失败 (code={}, msg={})", body.code, body.msg
            ));
        }

        self.token = Some(body.tenant_access_token);
        // expire 字段是秒数，提前 5 分钟过期
        let ttl = Duration::from_secs(body.expire.saturating_sub(300));
        self.expires_at = Some(Instant::now() + ttl);

        log::info!("飞书 token 已刷新，有效期 {}秒", body.expire);
        Ok(())
    }

    /// 构造 Authorization header 值
    pub fn auth_header(&mut self) -> Result<String> {
        let token = self.get_token()?;
        Ok(format!("Bearer {}", token))
    }
}

#[derive(Serialize, Deserialize)]
struct FeishuSecret {
    app_id: String,
    app_secret: String,
}
```

### 3.2 错误处理

```rust
/// 统一的飞书 API 调用封装
pub fn feishu_api_call<T: for<'de> Deserialize<'de>>(
    auth: &mut FeishuAuth,
    method: reqwest::Method,
    url: &str,
    body: Option<serde_json::Value>,
) -> Result<FeishuApiResponse<T>> {
    let token = auth.get_token()?;
    let mut req = auth.client.request(method, url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json; charset=utf-8");

    if let Some(b) = body {
        req = req.json(&b);
    }

    let resp = req.send()?;

    // 处理认证失败（token 过期后首次调用可能触发 99991663）
    if resp.status() == 401 {
        auth.token = None;  // 强制下次刷新
        return Err(anyhow::anyhow!("飞书认证已过期，请重试"));
    }

    let api_resp: FeishuApiResponse<T> = resp.json()?;
    if api_resp.code != 0 {
        // 常见错误码处理
        match api_resp.code {
            99991663 => {
                auth.token = None;
                return Err(anyhow::anyhow!("飞书 token 无效，已自动刷新，请重试"));
            }
            99991668 => {
                return Err(anyhow::anyhow!("飞书 API 限流，请稍后重试"));
            }
            _ => {
                return Err(anyhow::anyhow!("飞书 API 错误: {} (code={})", api_resp.msg, api_resp.code));
            }
        }
    }

    Ok(api_resp)
}
```

### 3.3 交互流程

```
用户首次使用飞书同步:
1. 前端 → tauriCallSafe('save_feishu_credentials', { appId, appSecret })
2. 后端 → FeishuAuth::save_to_keychain() 存入 OS keychain
3. 后端 → FeishuAuth::from_keychain() 验证凭据有效性
4. 成功 → 返回 ok；失败 → 返回错误信息

后续每次 API 调用:
1. FeishuAuth::get_token() 检查缓存
2. 未过期 → 直接使用缓存的 token
3. 已过期或提前 5 分钟 → 自动 POST /auth/v3/tenant_access_token/internal 刷新
4. 刷新失败 → 返回错误，前端提示重新配置凭据
```

---

## 四、飞书导入补充 — hearings / tasks / officials / case_relations

### 4.1 hearings 表导入（15 字段）

```rust
// src-tauri/src/migrate/feishu_hearings.rs

/// 飞书 hearings 表字段映射
/// 飞书字段名 → Casy 字段名 → 类型
const HEARING_FIELDS: &[(&str, &str, &str)] = &[
    ("庭审记录号", "hearing_record", "text"),       // 必填，用作幂等 key
    ("案件", "case_id", "duplex_link"),              // 关联到 cases 表
    ("庭审名称", "hearing_name", "text"),
    ("庭审日期", "hearing_date", "datetime"),
    ("地点", "venue", "text"),
    ("出席人员", "attendees", "text"),
    ("法官/合议组", "judges", "text"),               // JSON array
    ("审理机关", "court", "text"),
    ("审级", "case_level", "text"),
    ("联系方式", "contact_info", "text"),
    ("实际状态", "actual_status", "select"),          // 已开 / 未开
    ("关联文件", "files_json", "attachment"),
    ("备注", "notes", "text"),
    ("创建时间", "created_at", "datetime"),
    ("更新时间", "updated_at", "datetime"),
];

pub fn import_hearings(conn: &Connection, dump: &Value) -> Result<ImportReport> {
    let records = dump.pointer("/tables/hearings/records")
        .and_then(|v| v.as_array())
        .unwrap_or(&vec![]);
    let mut report = ImportReport::default();

    for record in records {
        let feishu_id = record["record_id"].as_str().unwrap_or_default();
        let fields = &record["fields"];

        // 解析关联案件（DuplexLink 字段）
        let case_id = extract_duplex_link_first(fields, "案件")
            .unwrap_or_default();

        let hearing = Hearing {
            id: feishu_id.to_string(),  // 用飞书 record_id 作幂等
            case_id,
            hearing_record: extract_text(&fields["庭审记录号"]),
            hearing_name: Some(extract_text(&fields["庭审名称"])),
            hearing_date: extract_datetime(&fields["庭审日期"]),
            venue: Some(extract_text(&fields["地点"])),
            attendees: Some(extract_text(&fields["出席人员"])),
            judges: Some(extract_json_array_text(&fields["法官/合议组"])),
            court: Some(extract_text(&fields["审理机关"])),
            case_level: Some(extract_single_select(&fields["审级"])),
            contact_info: Some(extract_text(&fields["联系方式"])),
            actual_status: Some(extract_single_select(&fields["实际状态"])
                .unwrap_or_default()),
            files_json: Some(extract_attachments_json(&fields["关联文件"])),
            created_at: Some(extract_datetime(&fields["创建时间"])
                .unwrap_or_else(|| now_local())),
        };

        // 幂等插入
        conn.execute(
            "INSERT OR REPLACE INTO hearings
             (id, case_id, hearing_record, hearing_name, hearing_date, venue,
              attendees, judges, court, case_level, contact_info, actual_status,
              files_json, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            rusqlite::params![
                hearing.id, hearing.case_id, hearing.hearing_record,
                hearing.hearing_name, hearing.hearing_date, hearing.venue,
                hearing.attendees, hearing.judges, hearing.court,
                hearing.case_level, hearing.contact_info, hearing.actual_status,
                hearing.files_json, hearing.created_at,
            ],
        )?;
        report.hearings += 1;
    }

    Ok(report)
}
```

### 4.2 tasks 表导入（11 字段）

```rust
/// 飞书 tasks 表字段映射
const TASK_FIELDS: &[(&str, &str, &str)] = &[
    ("任务名称", "task_name", "text"),
    ("案件", "case_id", "duplex_link"),
    ("描述", "description", "text"),
    ("创建日期", "created_date", "datetime"),
    ("截止日期", "deadline", "datetime"),
    ("优先级", "priority", "select"),                // 紧急重要/重要/紧急/普通
    ("是否完成", "completed", "checkbox"),
    ("指派人", "assignee", "text"),
    ("完成说明", "finish_note", "text"),
    ("来源日志", "source_log_id", "duplex_link"),
    ("创建时间", "created_at", "datetime"),
];

pub fn import_tasks(conn: &Connection, dump: &Value) -> Result<ImportReport> {
    let records = dump.pointer("/tables/tasks/records")
        .and_then(|v| v.as_array())
        .unwrap_or(&vec![]);
    let mut report = ImportReport::default();

    for record in records {
        let feishu_id = record["record_id"].as_str().unwrap_or_default();
        let fields = &record["fields"];

        let case_id = extract_duplex_link_first(fields, "案件").unwrap_or_default();
        let source_log = extract_duplex_link_first(fields, "来源日志");

        // 优先级映射：飞书选项 → Casy 四象限
        let priority = match extract_single_select(&fields["优先级"]).as_deref() {
            Some("紧急重要") => "urgent_important",
            Some("重要") => "important",
            Some("紧急") => "urgent",
            _ => "normal",
        };

        // 完成状态：飞书 checkbox → 0/1
        let completed = extract_checkbox(&fields["是否完成"]) as i32;

        let task = Task {
            id: feishu_id.to_string(),
            case_id: if case_id.is_empty() { None } else { Some(case_id) },
            task_name: extract_text(&fields["任务名称"]),
            description: Some(extract_text(&fields["描述"])),
            created_date: extract_datetime(&fields["创建日期"])
                .unwrap_or_else(|| today()),
            deadline: extract_datetime(&fields["截止日期"]),
            priority: Some(priority.to_string()),
            completed,
            assignee: Some(extract_text(&fields["指派人"])),
            finish_note: Some(extract_text(&fields["完成说明"])),
            source_log_id: source_log,
            created_at: Some(extract_datetime(&fields["创建时间"])
                .unwrap_or_else(|| now_local())),
        };

        conn.execute(
            "INSERT OR REPLACE INTO tasks
             (id, case_id, task_name, description, created_date, deadline,
              priority, completed, assignee, finish_note, source_log_id, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                task.id, task.case_id, task.task_name, task.description,
                task.created_date, task.deadline, task.priority, task.completed,
                task.assignee, task.finish_note, task.source_log_id, task.created_at,
            ],
        )?;
        report.tasks += 1;
    }

    Ok(report)
}
```

### 4.3 officials 表导入（7 字段）

```rust
/// 飞书 officials 表字段映射
const OFFICIAL_FIELDS: &[(&str, &str, &str)] = &[
    ("姓名", "name", "text"),
    ("角色", "role", "select"),                      // 法官/法官助理/书记员/法院
    ("所属法院", "court", "text"),
    ("联系方式", "contact_detail", "text"),
    ("联系文本", "contact_text", "text"),
    ("联系记录", "contact_record", "text"),
    ("创建时间", "created_at", "datetime"),
];

pub fn import_officials(conn: &Connection, dump: &Value) -> Result<ImportReport> {
    let records = dump.pointer("/tables/officials/records")
        .and_then(|v| v.as_array())
        .unwrap_or(&vec![]);
    let mut report = ImportReport::default();

    for record in records {
        let feishu_id = record["record_id"].as_str().unwrap_or_default();
        let fields = &record["fields"];

        let role = extract_single_select(&fields["角色"])
            .unwrap_or_else(|| "法院".to_string());

        // 验证 role 合法性
        if !["法官", "法官助理", "书记员", "法院"].contains(&role.as_str()) {
            report.errors.push(format!("官方人员 {} 角色无效: {}", feishu_id, role));
            continue;
        }

        let official = Official {
            id: feishu_id.to_string(),
            name: Some(extract_text(&fields["姓名"])),
            role,
            court: extract_text(&fields["所属法院"]),
            contact_detail: extract_text(&fields["联系方式"]),
            contact_text: Some(extract_text(&fields["联系文本"])),
            contact_record: Some(extract_text(&fields["联系记录"])),
            created_at: Some(extract_datetime(&fields["创建时间"])
                .unwrap_or_else(|| now_local())),
        };

        conn.execute(
            "INSERT OR REPLACE INTO officials
             (id, name, role, court, contact_detail, contact_text, contact_record, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                official.id, official.name, official.role, official.court,
                official.contact_detail, official.contact_text,
                official.contact_record, official.created_at,
            ],
        )?;
        report.officials += 1;
    }

    Ok(report)
}
```

### 4.4 case_relations 导入（DuplexLink 字段）

飞书多维表格中的"关联"类型字段（DuplexLink）存储了记录间的双向关联。从 cases 表的关联字段中提取关系。

```rust
/// 从 cases 表的 DuplexLink 字段中提取案件关系
pub fn import_case_relations(conn: &Connection, dump: &Value) -> Result<ImportReport> {
    let records = dump.pointer("/tables/cases/records")
        .and_then(|v| v.as_array())
        .unwrap_or(&vec![]);
    let mut report = ImportReport::default();

    for record in records {
        let source_id = record["record_id"].as_str().unwrap_or_default();
        let fields = &record["fields"];

        // 同一专利案件（DuplexLink 字段"关联案件"）
        if let Some(related_ids) = extract_duplex_link_all(fields, "关联案件") {
            for target_id in &related_ids {
                if source_id != target_id {
                    insert_relation_if_absent(conn, source_id, target_id, "same_patent", None)?;
                    report.relations += 1;
                }
            }
        }

        // 同一客户案件（从 client_name 自动推断）
        // 这部分在 import_all 完成后调用 auto_detect_relations()

        // 上诉关系（从 case_level "二审"/"再审" + 关联案件推断）
        let level = extract_single_select(&fields["审级"]);
        if level.as_deref() == Some("二审") || level.as_deref() == Some("再审") {
            if let Some(related_ids) = extract_duplex_link_all(fields, "关联案件") {
                for target_id in &related_ids {
                    insert_relation_if_absent(conn, source_id, target_id, "appeal_of", None)?;
                    report.relations += 1;
                }
            }
        }
    }

    Ok(report)
}

/// 自动检测：相同 patent_app_no 的案件互相建立 same_patent 关系
pub fn auto_detect_relations(conn: &Connection) -> Result<i64> {
    let mut count = 0i64;

    // 按专利号分组
    let mut stmt = conn.prepare(
        "SELECT id, patent_app_no FROM cases WHERE patent_app_no IS NOT NULL AND patent_app_no != ''"
    )?;
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, patent_no) = row?;
        groups.entry(patent_no).or_default().push(id);
    }

    // 同一专利号的案件互相建立关系
    for (_patent_no, ids) in &groups {
        if ids.len() < 2 { continue; }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                insert_relation_if_absent(conn, &ids[i], &ids[j], "same_patent", None)?;
                count += 1;
            }
        }
    }

    // 按客户分组（同客户不同案件）
    let mut stmt = conn.prepare(
        "SELECT id, client_name FROM cases WHERE client_name IS NOT NULL AND client_name != ''"
    )?;
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, client) = row?;
        groups.entry(client).or_default().push(id);
    }
    for (_client, ids) in &groups {
        if ids.len() < 2 { continue; }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                insert_relation_if_absent(conn, &ids[i], &ids[j], "same_party", None)?;
                count += 1;
            }
        }
    }

    Ok(count)
}

fn insert_relation_if_absent(
    conn: &Connection,
    source: &str,
    target: &str,
    rel_type: &str,
    label: Option<&str>,
) -> Result<()> {
    // 双向插入（UNIQUE 约束防止重复）
    for (s, t) in [(source, target), (target, source)] {
        conn.execute(
            "INSERT OR IGNORE INTO case_relations (id, source_case_id, target_case_id, relation_type, label)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![new_id(), s, t, rel_type, label],
        )?;
    }
    Ok(())
}
```

### 4.5 DuplexLink 字段提取辅助函数

```rust
/// 提取 DuplexLink 字段的第一条记录 ID
fn extract_duplex_link_first(fields: &Value, key: &str) -> Option<String> {
    let val = fields.get(key)?;
    match val {
        // 飞书 DuplexLink 格式: [{"record_id": "xxx", "text": "..."}]
        Value::Array(arr) => arr.first()
            .and_then(|item| item.get("record_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        // 兼容直接是字符串的情况
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// 提取 DuplexLink 字段的所有记录 ID
fn extract_duplex_link_all(fields: &Value, key: &str) -> Option<Vec<String>> {
    let val = fields.get(key)?;
    match val {
        Value::Array(arr) => {
            let ids: Vec<String> = arr.iter()
                .filter_map(|item| {
                    item.get("record_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            if ids.is_empty() { None } else { Some(ids) }
        }
        _ => None,
    }
}
```

---

## 五、自动保存 vs 同步冲突预防策略

### 5.1 防抖层级设计

```
用户按键 → auto-save (2s) → schedule_push (5s) → WebDAV/飞书 PUSH
                                                        │
                                                        └→ VACUUM INTO 仅手动/启动
```

三层防抖，逐层递进：

| 层级 | 触发 | 延迟 | 目标 |
|------|------|------|------|
| L1 自动保存 | 用户停止输入 | 2 秒 | 写入本地 SQLite |
| L2 同步推送 | L1 保存完成 | 5 秒 | 推送到远程 |
| L3 安全拷贝 | 手动同步或启动 | 无延迟 | VACUUM INTO + 上传 |

### 5.2 Rust 端实现

```rust
// src-tauri/src/sync/debounce.rs

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct DebounceSync {
    /// 上次自动保存的时间
    last_save_at: Arc<Mutex<Option<Instant>>>,
    /// 上次推送的时间
    last_push_at: Arc<Mutex<Option<Instant>>>,
    /// 是否有待推送的修改
    has_pending_changes: Arc<AtomicBool>,
    /// 推送定时器的 generation（用于取消旧的延迟推送）
    push_generation: Arc<AtomicU64>,
}

impl DebounceSync {
    pub fn new() -> Self {
        Self {
            last_save_at: Arc::new(Mutex::new(None)),
            last_push_at: Arc::new(Mutex::new(None)),
            has_pending_changes: Arc::new(AtomicBool::new(false)),
            push_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// L1: 自动保存完成后调用
    pub async fn on_save(&self) {
        let mut last_save = self.last_save_at.lock().await;
        *last_save = Some(Instant::now());
        self.has_pending_changes.store(true, Ordering::SeqCst);

        // 计划 5 秒后推送
        let gen = self.push_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let push_gen = self.push_generation.clone();
        let last_save_at = self.last_save_at.clone();
        let last_push_at = self.last_push_at.clone();
        let has_pending = self.has_pending_changes.clone();

        tokio::spawn(async move {
            // 等待 5 秒
            tokio::time::sleep(Duration::from_secs(5)).await;

            // 检查是否被更新的 generation 取消
            if push_gen.load(Ordering::SeqCst) != gen {
                return; // 有更新的 save 发生，本次推送取消
            }

            // 检查 5 秒内是否还有新的 save
            let last_save_time = *last_save_at.lock().await;
            if let Some(t) = last_save_time {
                if t.elapsed() < Duration::from_secs(5) {
                    return; // 5 秒内有新 save，等下一轮
                }
            }

            // 执行推送
            if has_pending.load(Ordering::SeqCst) {
                log::info!("自动推送触发");
                // sync_coordinator.push() —— 实际推送逻辑
                let mut last_push = last_push_at.lock().await;
                *last_push = Some(Instant::now());
                has_pending.store(false, Ordering::SeqCst);
            }
        });
    }

    /// L3: 仅在手动同步或启动时执行 VACUUM INTO
    pub async fn should_vacuum(&self) -> bool {
        // 只在以下情况执行：
        // 1. 用户手动点击"立即同步"
        // 2. 应用启动时的首次同步
        // 不在每次自动推送时执行
        false // 由调用方显式传入
    }
}
```

### 5.3 前端自动保存集成

```javascript
// src/modules/cases/composables/useAutoSave.js

export function useAutoSave(caseId) {
  const store = useCasesStore()
  let saveTimer = null
  let pushTimer = null

  function scheduleSave(field, value) {
    // L1: 2 秒防抖自动保存
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(async () => {
      const result = await store.updateCase(caseId, { [field]: value })
      if (result.ok) {
        // L2: 通知后端有新保存，后端管理 5 秒推送防抖
        await tauriCallSafe('notify_save_done', {})
      }
    }, 2000)
  }

  // 组件卸载时清除定时器
  onUnmounted(() => {
    if (saveTimer) clearTimeout(saveTimer)
    if (pushTimer) clearTimeout(pushTimer)
  })

  return { scheduleSave }
}
```

### 5.4 WebDAV 上传策略

WebDAV 与飞书不同，每次上传是全量数据库文件（<1MB），因此不走自动推送，仅在以下时机上传：

| 时机 | 是否上传 | 是否 VACUUM INTO |
|------|---------|-----------------|
| 自动保存（2秒防抖后） | 否 | 否 |
| 飞书自动推送（5秒防抖后） | 否 | 否 |
| 用户手动点击"立即同步" | 是 | 是 |
| 应用启动（检测到远程变化） | 是（仅 PULL） | 否 |
| 应用关闭 | 是 | 是 |

```rust
/// WebDAV 同步策略：仅手动或关闭时上传
pub fn webdav_sync_strategy() -> SyncStrategy {
    SyncStrategy {
        auto_push: false,         // 不自动推送
        push_on_manual: true,     // 手动同步时上传
        push_on_close: true,      // 关闭时上传
        vacuum_on_push: true,     // 上传前 VACUUM INTO
        pull_on_startup: true,    // 启动时检查远程
    }
}

/// 飞书同步策略：5 秒防抖自动推送
pub fn feishu_sync_strategy() -> SyncStrategy {
    SyncStrategy {
        auto_push: true,          // 自动推送
        auto_push_delay: 5,       // 5 秒防抖
        push_on_manual: true,
        push_on_close: true,
        vacuum_on_push: false,    // 飞书不需要 VACUUM
        pull_on_startup: true,
        pull_interval: 15 * 60,   // 15 分钟定时拉取
    }
}
```

### 5.5 冲突预防总结

```
场景：用户快速编辑案件 → 自动保存 → 同时另一设备修改了同一案件

预防机制：
1. 自动保存是 PATCH 语义（只更新提供的字段），不覆盖未提供的字段
2. 飞书 PUSH 使用 updated_at 时间戳裁决：本地更新时间 > 远程 → 推送；否则跳过
3. WebDAV 使用 ETag 检测：PUSH 前 HEAD 检查 ETag，变了则触发冲突解决
4. 冲突发生时：弹出 ConflictResolver 组件，用户逐字段选择

极端场景：用户在设备 A 编辑字段 X，在设备 B 编辑字段 Y
→ PATCH 语义下不会冲突，因为每次只推送修改过的字段
→ 飞书合并后两端都包含最新的 X 和 Y
```

---

## 六、附录：飞书通用提取辅助函数

这些函数在 hearings/tasks/officials 导入中共享：

```rust
/// 提取文本字段
fn extract_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(obj) => obj.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

/// 提取单选字段
fn extract_single_select(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) => arr.first().and_then(|v| v.as_str()).map(|s| s.to_string()),
        Value::Object(obj) => obj.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    }
}

/// 提取日期时间字段（飞书毫秒时间戳或字符串）
fn extract_datetime(value: &Value) -> Option<String> {
    match value {
        Value::Number(n) => {
            let ms = n.as_i64()?;
            let dt = chrono::NaiveDateTime::from_timestamp_millis(ms)?;
            Some(dt.format("%Y-%m-%d").to_string())
        }
        Value::String(s) => {
            // 已经是日期字符串
            if s.len() >= 10 { Some(s[..10].to_string()) } else { Some(s.clone()) }
        }
        _ => None,
    }
}

/// 提取 checkbox 字段（布尔值）
fn extract_checkbox(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(s) => s == "true" || s == "1" || s == "是",
        _ => false,
    }
}

/// 提取 JSON 数组文本（如法官列表 ["张三","李四"] → "["张三","李四"]"）
fn extract_json_array_text(value: &Value) -> String {
    match value {
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
        }
        Value::String(s) => {
            // 可能已经是 JSON 字符串
            if s.starts_with('[') { s.clone() }
            else { serde_json::to_string(&[s]).unwrap_or_else(|_| "[]".to_string()) }
        }
        _ => "[]".to_string(),
    }
}

/// 提取附件字段的 JSON 表示
fn extract_attachments_json(value: &Value) -> String {
    match value {
        Value::Array(arr) => {
            let files: Vec<serde_json::Value> = arr.iter().map(|item| {
                serde_json::json!({
                    "name": item.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "url": item.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    "size": item.get("size").and_then(|v| v.as_i64()).unwrap_or(0),
                })
            }).collect();
            serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string())
        }
        _ => "[]".to_string(),
    }
}
```
