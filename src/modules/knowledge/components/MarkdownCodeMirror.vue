<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { basicSetup } from 'codemirror'
import { Compartment, EditorState } from '@codemirror/state'
import { EditorView, keymap, placeholder as cmPlaceholder } from '@codemirror/view'
import { markdown } from '@codemirror/lang-markdown'
import { autocompletion } from '@codemirror/autocomplete'
import { indentWithTab } from '@codemirror/commands'

const props = defineProps({
  modelValue: { type: String, default: '' },
  noteTitles: { type: Array, default: () => [] },
  placeholder: { type: String, default: '# 标题\n\n使用 Markdown 开始写作…' },
})

const emit = defineEmits(['update:modelValue', 'save', 'blur'])
const host = ref(null)
let view = null
let applyingExternal = false
let themeObserver = null
const darkCompartment = new Compartment()

function wikiLinkCompletion(context) {
  const match = context.matchBefore(/\[\[[^\]]*/)
  if (!match) return null
  const query = match.text.slice(2).trim().toLowerCase()
  const options = props.noteTitles
    // 标题含方括号时 Wiki 语法无法解析（与后端 parse_wiki_titles 一致），不出现在补全里
    .filter(item => item?.title && !item.title.includes('[') && !item.title.includes(']'))
    .filter(item => !query || item.title.toLowerCase().includes(query))
    .slice(0, 40)
    .map(item => ({
      label: item.title,
      detail: item.categoryLabel || '知识笔记',
      type: 'text',
      apply: `${item.title}]]`,
    }))
  return { from: match.from + 2, options, validFor: /^[^\]]*$/ }
}

// 应用主题以 <html data-theme="..."> 为准（见 src/shared/theme.ts）
function isDarkTheme() {
  return ['dark', 'solarized-dark'].includes(document.documentElement.dataset.theme || '')
}

const casyTheme = EditorView.theme({
  '&': { height: '100%', backgroundColor: 'transparent', color: 'var(--c-text)' },
  '.cm-scroller': { fontFamily: "'SFMono-Regular', Consolas, 'Liberation Mono', monospace", lineHeight: '1.75' },
  '.cm-content': { padding: '24px 30px', caretColor: 'var(--c-primary)' },
  '.cm-line': { padding: '0' },
  '.cm-gutters': { backgroundColor: 'var(--c-bg-subtle)', color: 'var(--c-text-secondary)', border: '0', paddingRight: '4px' },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'color-mix(in srgb, var(--c-primary) 7%, transparent)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'color-mix(in srgb, var(--c-primary) 20%, transparent) !important' },
  '.cm-cursor': { borderLeftColor: 'var(--c-primary)' },
  '.cm-tooltip': { backgroundColor: 'var(--c-bg-card)', color: 'var(--c-text)', border: '1px solid var(--c-border)', boxShadow: 'var(--shadow-md)' },
  '.cm-tooltip-autocomplete ul li[aria-selected]': { backgroundColor: 'var(--c-primary)', color: '#fff' },
  '.cm-panels': { backgroundColor: 'var(--c-bg-card)', color: 'var(--c-text)' },
  '.cm-searchMatch': { backgroundColor: 'rgba(250, 204, 21, .28)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'rgba(249, 115, 22, .34)' },
})
// 注：dark 标志不在这里静态指定，统一由 darkCompartment 按 data-theme 动态供给，
// 否则主题从暗切亮时静态 true 会残留（Facet 组合为 or）。

function buildState(doc) {
  return EditorState.create({
    doc,
    extensions: [
      basicSetup,
      markdown(),
      EditorView.lineWrapping,
      casyTheme,
      darkCompartment.of(EditorView.darkTheme.of(isDarkTheme())),
      cmPlaceholder(props.placeholder),
      autocompletion({ override: [wikiLinkCompletion], activateOnTyping: true }),
      keymap.of([
        indentWithTab,
        { key: 'Mod-s', preventDefault: true, run: () => { emit('save'); return true } },
      ]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged && !applyingExternal) emit('update:modelValue', update.state.doc.toString())
      }),
      EditorView.domEventHandlers({ blur: () => { emit('blur'); return false } }),
    ],
  })
}

onMounted(() => {
  view = new EditorView({ state: buildState(props.modelValue), parent: host.value })
  // 主题切换时同步 CodeMirror 的 dark 标志（CSS 变量部分自动跟随，无需处理）
  themeObserver = new MutationObserver(() => {
    view?.dispatch({ effects: darkCompartment.reconfigure(EditorView.darkTheme.of(isDarkTheme())) })
  })
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

watch(() => props.modelValue, (value) => {
  if (!view || value === view.state.doc.toString()) return
  applyingExternal = true
  // 切换笔记：整体重建 state，撤销历史随之重置，
  // 避免 Cmd+Z 把上一篇笔记的内容"撤"进当前笔记。
  view.setState(buildState(value || ''))
  applyingExternal = false
})

onBeforeUnmount(() => {
  themeObserver?.disconnect()
  view?.destroy()
})

defineExpose({
  focus: () => view?.focus(),
  flushAndGetMarkdown: () => view?.state.doc.toString() ?? props.modelValue,
})
</script>

<template><div ref="host" class="codemirror-host" /></template>

<style scoped>
.codemirror-host{height:100%;min-height:0;overflow:hidden;font-size:14px}.codemirror-host :deep(.cm-editor){height:100%}.codemirror-host :deep(.cm-scroller){overflow:auto}.codemirror-host :deep(.cm-focused){outline:none}
</style>
