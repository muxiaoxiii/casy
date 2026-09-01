<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import {
  Document, Edit, Finished, List, Grid,
  Reading, Collection, MagicStick, Minus,
  Opportunity, User, ChatSquare
} from '@element-plus/icons-vue'

interface CommandItem {
  id: string
  title: string
  subtitle: string
  category: 'basic' | 'legal' | 'ai'
  icon: any
  action: () => void
  keywords: string[]
}

const props = defineProps<{
  editor: any
  visible: boolean
  position: { x: number; y: number }
  caseData?: Record<string, any>
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'open-ai', promptType?: string): void
  (e: 'open-law'): void
  (e: 'open-knowledge'): void
}>()

const search = ref('')
const selectedIndex = ref(0)
const menuRef = ref<HTMLElement | null>(null)

const commands = computed<CommandItem[]>(() => [
  // ── 基础格式 ──
  {
    id: 'paragraph',
    title: '正文段落',
    subtitle: '纯文本内容，支持 Markdown 语法',
    category: 'basic',
    icon: Document,
    keywords: ['p', 'text', 'paragraph', 'zhengwen', 'duanluo', '文本', '段落'],
    action: () => {
      props.editor.chain().focus().setParagraph().run()
    },
  },
  {
    id: 'h1',
    title: '一级标题 (H1)',
    subtitle: '大章节标题 (# )',
    category: 'basic',
    icon: Edit,
    keywords: ['h1', 'heading1', 'yijibiaoti', '标题1', '大标题'],
    action: () => {
      props.editor.chain().focus().toggleHeading({ level: 1 }).run()
    },
  },
  {
    id: 'h2',
    title: '二级标题 (H2)',
    subtitle: '小节与核心论点 (## )',
    category: 'basic',
    icon: Edit,
    keywords: ['h2', 'heading2', 'erjibiaoti', '标题2', '小节'],
    action: () => {
      props.editor.chain().focus().toggleHeading({ level: 2 }).run()
    },
  },
  {
    id: 'h3',
    title: '三级标题 (H3)',
    subtitle: '段落要点与子论据 (### )',
    category: 'basic',
    icon: Edit,
    keywords: ['h3', 'heading3', 'sanjibiaoti', '标题3'],
    action: () => {
      props.editor.chain().focus().toggleHeading({ level: 3 }).run()
    },
  },
  {
    id: 'bullet-list',
    title: '无序列表',
    subtitle: '项目清单与证据并列 (- )',
    category: 'basic',
    icon: List,
    keywords: ['ul', 'bullet', 'list', 'wuxu', 'liebiao', '清单'],
    action: () => {
      props.editor.chain().focus().toggleBulletList().run()
    },
  },
  {
    id: 'ordered-list',
    title: '有序列表',
    subtitle: '按顺序阐述事实与理由 (1. )',
    category: 'basic',
    icon: List,
    keywords: ['ol', 'ordered', 'num', 'youxu', '数字列表'],
    action: () => {
      props.editor.chain().focus().toggleOrderedList().run()
    },
  },
  {
    id: 'task-list',
    title: '待办清单',
    subtitle: '带勾选框的任务跟踪列表 ([] )',
    category: 'basic',
    icon: Finished,
    keywords: ['todo', 'task', 'daiban', 'checklist', '勾选'],
    action: () => {
      props.editor.chain().focus().toggleTaskList().run()
    },
  },
  {
    id: 'blockquote',
    title: '引用块',
    subtitle: '高亮法理依据或案由摘录 (> )',
    category: 'basic',
    icon: ChatSquare,
    keywords: ['quote', 'blockquote', 'yinyong', '引用', '摘录'],
    action: () => {
      props.editor.chain().focus().toggleBlockquote().run()
    },
  },
  {
    id: 'callout',
    title: '重要提示框 (Callout)',
    subtitle: '醒目的背景卡片，用于诉讼预警或法庭须知',
    category: 'basic',
    icon: Opportunity,
    keywords: ['callout', 'tishi', 'box', 'card', '卡片', '警告', '提示'],
    action: () => {
      props.editor.chain().focus().insertContent(`
        <blockquote class="callout-box">
          <p>💡 <strong>重要法庭提示：</strong>在此输入需要重点注意的诉讼策略或合议庭倾向...</p>
        </blockquote>
      `).run()
    },
  },
  {
    id: 'divider',
    title: '分割线',
    subtitle: '视觉划分不同文书版块 (---)',
    category: 'basic',
    icon: Minus,
    keywords: ['hr', 'divider', 'line', 'fengexian', '分割线', '分界'],
    action: () => {
      props.editor.chain().focus().setHorizontalRule().run()
    },
  },
  {
    id: 'table',
    title: '表格 (3x3)',
    subtitle: '用于事实比对、质证清单或损失赔偿计算',
    category: 'basic',
    icon: Grid,
    keywords: ['table', 'grid', 'biaoge', '表格', '质证表'],
    action: () => {
      props.editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()
    },
  },

  // ── 法律业务专属 ──
  {
    id: 'case-field',
    title: '案件动态字段',
    subtitle: '自动填充：案号、委托人、审理法院等 ({ )',
    category: 'legal',
    icon: Document,
    keywords: ['field', 'case', 'ziduan', 'anhao', '案号', '法院', '标的'],
    action: () => {
      props.editor.chain().focus().insertContent('{').run()
    },
  },
  {
    id: 'legal-provision',
    title: '法条快速引用',
    subtitle: '检索民法典、民诉法、专利法等法条 (【 )',
    category: 'legal',
    icon: Reading,
    keywords: ['law', 'statute', 'fatiao', '法条', '法律', '规定'],
    action: () => {
      emit('open-law')
    },
  },
  {
    id: 'party-name',
    title: '当事人提及',
    subtitle: '快速插入原告、被告或第三人姓名 (@ )',
    category: 'legal',
    icon: User,
    keywords: ['party', 'client', 'dangshiren', '当事人', '原告', '被告'],
    action: () => {
      props.editor.chain().focus().insertContent('@').run()
    },
  },
  {
    id: 'knowledge-vault',
    title: '智库知识双链',
    subtitle: '搜索并插入知识库沉淀的胜诉要点与法理备忘',
    category: 'legal',
    icon: Collection,
    keywords: ['vault', 'knowledge', 'zhishi', '智库', '知识库', '双链'],
    action: () => {
      emit('open-knowledge')
    },
  },

  // ── AI 智能助手 ──
  {
    id: 'ai-draft',
    title: 'AI 辅助起草',
    subtitle: '根据案情与诉求，智能生成完整文书框架',
    category: 'ai',
    icon: MagicStick,
    keywords: ['ai', 'copilot', 'qicao', '起草', '撰写', '智能'],
    action: () => {
      emit('open-ai', 'draft')
    },
  },
  {
    id: 'ai-polish',
    title: 'AI 润色与法言法语转换',
    subtitle: '将口语化表达升华为规范严谨的司法文书术语',
    category: 'ai',
    icon: MagicStick,
    keywords: ['ai', 'polish', 'runse', '润色', '法言法语', '优化'],
    action: () => {
      emit('open-ai', 'polish')
    },
  },
])

const filteredCommands = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return commands.value
  return commands.value.filter(cmd =>
    cmd.title.toLowerCase().includes(query) ||
    cmd.subtitle.toLowerCase().includes(query) ||
    cmd.keywords.some(k => k.toLowerCase().includes(query))
  )
})

const groupedCommands = computed(() => {
  const groups: Record<string, CommandItem[]> = {
    basic: [],
    legal: [],
    ai: [],
  }
  for (const cmd of filteredCommands.value) {
    if (groups[cmd.category]) {
      groups[cmd.category].push(cmd)
    }
  }
  return groups
})

function executeCommand(cmd: CommandItem) {
  cmd.action()
  emit('close')
}

function handleKeydown(e: KeyboardEvent) {
  if (!props.visible) return

  if (e.key === 'ArrowDown') {
    e.preventDefault()
    selectedIndex.value = (selectedIndex.value + 1) % filteredCommands.value.length
    scrollSelectedIntoView()
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selectedIndex.value =
      (selectedIndex.value - 1 + filteredCommands.value.length) % filteredCommands.value.length
    scrollSelectedIntoView()
  } else if (e.key === 'Enter') {
    e.preventDefault()
    const target = filteredCommands.value[selectedIndex.value]
    if (target) {
      executeCommand(target)
    }
  } else if (e.key === 'Escape') {
    e.preventDefault()
    emit('close')
  }
}

function scrollSelectedIntoView() {
  const el = menuRef.value?.querySelector('.command-item.selected')
  if (el) {
    el.scrollIntoView({ block: 'nearest' })
  }
}

watch(() => props.visible, (val) => {
  if (val) {
    search.value = ''
    selectedIndex.value = 0
    window.addEventListener('keydown', handleKeydown, true)
  } else {
    window.removeEventListener('keydown', handleKeydown, true)
  }
})

const menuStyle = computed(() => {
  const maxW = typeof window !== 'undefined' ? window.innerWidth - 340 : 800
  return {
    left: Math.min(props.position.x, maxW) + 'px',
    top: props.position.y + 24 + 'px',
  }
})
</script>

<template>
  <div
    v-if="visible"
    ref="menuRef"
    class="notion-slash-menu"
    :style="menuStyle"
    @click.stop
  >
    <div class="slash-search-bar">
      <el-icon :size="14" color="var(--slate-gray-light)"><Edit /></el-icon>
      <input
        v-model="search"
        placeholder="搜索块类型或输入关键字 (如 标题, 表格, 法条)..."
        class="slash-input"
        autofocus
      />
    </div>

    <div class="slash-scroll-list">
      <!-- 基础块 -->
      <div v-if="groupedCommands.basic.length" class="command-group">
        <div class="group-label">基础排版块</div>
        <div
          v-for="cmd in groupedCommands.basic"
          :key="cmd.id"
          class="command-item"
          :class="{ selected: filteredCommands[selectedIndex]?.id === cmd.id }"
          @click="executeCommand(cmd)"
          @mouseenter="selectedIndex = filteredCommands.findIndex(c => c.id === cmd.id)"
        >
          <div class="cmd-icon-box">
            <el-icon><component :is="cmd.icon" /></el-icon>
          </div>
          <div class="cmd-text-box">
            <span class="cmd-title">{{ cmd.title }}</span>
            <span class="cmd-sub">{{ cmd.subtitle }}</span>
          </div>
        </div>
      </div>

      <!-- 法律专属块 -->
      <div v-if="groupedCommands.legal.length" class="command-group">
        <div class="group-label">法律与案卷专属</div>
        <div
          v-for="cmd in groupedCommands.legal"
          :key="cmd.id"
          class="command-item"
          :class="{ selected: filteredCommands[selectedIndex]?.id === cmd.id }"
          @click="executeCommand(cmd)"
          @mouseenter="selectedIndex = filteredCommands.findIndex(c => c.id === cmd.id)"
        >
          <div class="cmd-icon-box legal">
            <el-icon><component :is="cmd.icon" /></el-icon>
          </div>
          <div class="cmd-text-box">
            <span class="cmd-title">{{ cmd.title }}</span>
            <span class="cmd-sub">{{ cmd.subtitle }}</span>
          </div>
        </div>
      </div>

      <!-- AI 智能助手 -->
      <div v-if="groupedCommands.ai.length" class="command-group">
        <div class="group-label">AI 智伴赋能</div>
        <div
          v-for="cmd in groupedCommands.ai"
          :key="cmd.id"
          class="command-item"
          :class="{ selected: filteredCommands[selectedIndex]?.id === cmd.id }"
          @click="executeCommand(cmd)"
          @mouseenter="selectedIndex = filteredCommands.findIndex(c => c.id === cmd.id)"
        >
          <div class="cmd-icon-box ai">
            <el-icon><component :is="cmd.icon" /></el-icon>
          </div>
          <div class="cmd-text-box">
            <span class="cmd-title">{{ cmd.title }}</span>
            <span class="cmd-sub">{{ cmd.subtitle }}</span>
          </div>
        </div>
      </div>

      <!-- 无匹配结果 -->
      <div v-if="filteredCommands.length === 0" class="empty-hint">
        未找到匹配的块指令，按 ESC 返回
      </div>
    </div>
  </div>
</template>

<style scoped>
.notion-slash-menu {
  position: fixed;
  width: 320px;
  max-height: 380px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.15), 0 2px 6px rgba(0, 0, 0, 0.05);
  display: flex;
  flex-direction: column;
  z-index: 9999;
  overflow: hidden;
  animation: slash-pop 0.15s cubic-bezier(0.2, 0.8, 0.2, 1);
}

@keyframes slash-pop {
  from {
    opacity: 0;
    transform: scale(0.96) translateY(-4px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.slash-search-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--c-border-light);
  background: var(--c-bg-subtle);
}

.slash-input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 12.5px;
  color: var(--c-text);
}

.slash-scroll-list {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.command-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.group-label {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 6px 8px 2px;
}

.command-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px;
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  transition: all var(--motion-fast);
}

.command-item:hover,
.command-item.selected {
  background: var(--c-bg-hover);
}

.command-item.selected {
  background: var(--c-bg-selected);
}

.cmd-icon-box {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  display: grid;
  place-items: center;
  font-size: 14px;
  color: var(--c-text);
  flex-shrink: 0;
}

.cmd-icon-box.legal {
  background: var(--c-primary-light);
  color: var(--c-primary);
  border-color: transparent;
}

.cmd-icon-box.ai {
  background: var(--bg-risk-weak);
  color: var(--status-risk);
  border-color: transparent;
}

.cmd-text-box {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 1px;
}

.cmd-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.cmd-sub {
  font-size: 11px;
  color: var(--slate-gray-light);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty-hint {
  padding: 24px 16px;
  text-align: center;
  font-size: 12px;
  color: var(--slate-gray-light);
}
</style>
