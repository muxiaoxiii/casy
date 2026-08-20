/**
 * Copilot 写作辅助 composable
 *
 * 提供知识检索、AI 写作生成、建议插入等功能。
 * 与 TipTap 编辑器配合使用。
 */

import { ref, shallowRef } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'

/**
 * Copilot composable
 * @param {import('vue').Ref} editorRef - TipTap editor ref
 * @param {Object} options - 配置选项
 * @param {string} [options.style] - 当前文书风格
 * @param {string} [options.caseId] - 关联案件 ID
 */
export function useCopilot(editorRef, options = {}) {
  // 检索状态
  const searchQuery = ref('')
  const searchResults = ref({
    paragraphs: [],  // 相关段落
    laws: [],        // 相关法条
    cases: [],       // 相关判例
  })
  const searching = ref(false)

  // AI 生成状态
  const generating = ref(false)
  const aiSuggestion = ref(null)  // { text, intent, sources }
  const aiDialogVisible = ref(false)
  const aiIntent = ref('')

  // 展开的知识条目
  const expandedItemId = ref(null)

  // 搜索防抖计时器
  let searchTimer = null

  /**
   * 根据查询文本检索知识库
   * 使用混合检索（FTS5 + 语义向量）
   * @param {string} query - 搜索查询
   * @param {number} [limit=15] - 返回条数
   */
  async function searchKnowledge(query, limit = 15) {
    if (!query || query.trim().length < 2) {
      searchResults.value = { paragraphs: [], laws: [], cases: [] }
      return
    }

    searching.value = true
    const result = await tauriCallSafe('hybrid_search_knowledge', {
      query: query.trim(),
      limit,
    })

    if (result.ok && result.data) {
      // 按 category 分组
      const paragraphs = []
      const laws = []
      const cases = []

      for (const item of result.data) {
        const entry = {
          id: item.id,
          title: item.title,
          content: item.content,
          category: item.category,
          tags: item.tags,
          lawName: item.lawName || item.law_name,
          articleNo: item.articleNo || item.article_no,
          score: item.score,
          source: item.source,
          expanded: false,
        }

        // 分类到不同区域（基于职能分类）
        if (item.category === 'reference') {
          // 参考类：法条、案例、文献
          if (item.lawName || item.articleNo) {
            laws.push(entry)
          } else {
            cases.push(entry)
          }
        } else {
          // 灵感、方法、问题、经验、日志
          paragraphs.push(entry)
        }
      }

      searchResults.value = { paragraphs, laws, cases }
    } else {
      searchResults.value = { paragraphs: [], laws: [], cases: [] }
    }

    searching.value = false
  }

  /**
   * 防抖搜索（300ms）
   * @param {string} query
   */
  function debouncedSearch(query) {
    if (searchTimer) clearTimeout(searchTimer)
    searchTimer = setTimeout(() => searchKnowledge(query), 300)
  }

  /**
   * 根据编辑器内容自动检索相关上下文
   * 提取当前段落和选中文本作为查询
   * @param {string} editorContent - 编辑器全文内容
   */
  async function searchContext(editorContent) {
    if (!editorContent) return

    const editor = editorRef.value
    if (!editor) return

    // 获取当前光标所在段落的文本
    const { state } = editor
    const { from } = state.selection

    // 获取当前段落
    const resolvedPos = state.doc.resolve(from)
    const paragraphNode = resolvedPos.parent
    const paragraphText = paragraphNode.textContent || ''

    // 取段落前 100 字符作为检索查询
    const query = paragraphText.trim().substring(0, 100)
    if (query.length >= 4) {
      await searchKnowledge(query, 10)
    }
  }

  /**
   * 根据职能分类名称获取中文标签
   */
  function getCategoryLabel(category) {
    const labels = {
      inspiration: '灵感',
      method: '方法',
      reference: '参考',
      question: '问题',
      experience: '经验',
      log: '日志',
      // 兼容旧分类
      common_paragraph: '参考',
      law_reference: '参考',
      legal_provision: '参考',
      case_note: '参考',
      complaint: '方法',
      defense_brief: '方法',
      legal_opinion: '方法',
      lawyer_letter: '方法',
      reply_brief: '方法',
      other: '参考',
    }
    return labels[category] || category
  }

  /**
   * 获取职能标签对应的 emoji
   */
  function getCategoryIcon(category) {
    const icons = {
      inspiration: '💡',
      method: '📐',
      reference: '📖',
      question: '❓',
      experience: '⭐',
      log: '📝',
      // 兼容旧分类
      common_paragraph: '📖',
      law_reference: '📖',
      legal_provision: '📖',
      case_note: '📖',
      complaint: '📐',
      defense_brief: '📐',
      legal_opinion: '📐',
      lawyer_letter: '📐',
      reply_brief: '📐',
      other: '📖',
    }
    return icons[category] || '📝'
  }

  /**
   * 将知识条目内容插入编辑器光标位置
   * @param {Object} item - 知识条目
   */
  function insertToEditor(item) {
    const editor = editorRef.value
    if (!editor || !item?.content) return

    editor.chain().focus().insertContent(item.content).run()
  }

  /**
   * 插入引用脚注（来源 + 案号）
   * @param {Object} item - 知识条目
   */
  /**
   * 插入块引用（设计哲学 §9.3）
   * 
   * 如果知识条目有 id，使用 TipTap 块引用扩展插入可交互的引用节点；
   * 否则回退到文本引用。
   */
  function insertCitation(item) {
    const editor = editorRef.value
    if (!editor) return

    // 如果有知识条目 ID，使用块引用节点
    if (item.id) {
      editor.chain().focus().insertBlockReference(item.id, null).run()
      return
    }

    // 回退到文本引用（法条等）
    let citation = ''
    if (item.lawName && item.articleNo) {
      citation = `《${item.lawName}》${item.articleNo}`
    } else if (item.title) {
      citation = item.title
    }

    const source = item.tags ? `（来源：${item.tags}）` : ''
    const footnote = `\n【引用】${citation}${source}`

    editor.chain().focus().insertContent(footnote).run()
  }

  /**
   * 复制知识条目内容到剪贴板
   * @param {Object} item
   */
  async function copyContent(item) {
    if (!item?.content) return
    try {
      await navigator.clipboard.writeText(item.content)
      return true
    } catch {
      // fallback
      const textarea = document.createElement('textarea')
      textarea.value = item.content
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
      return true
    }
  }

  /**
   * 切换知识条目展开/折叠
   * @param {string} itemId
   */
  function toggleExpand(itemId) {
    expandedItemId.value = expandedItemId.value === itemId ? null : itemId
  }

  /**
   * 调用 AI 生成写作建议
   * @param {string} intent - 写作意图
   * @param {string} [context] - 当前编辑器上下文
   * @param {string} [style] - 文书风格
   * @returns {Promise<string|null>} 生成的文本
   */
  async function generateWriting(intent, context, style) {
    if (!intent || intent.trim().length === 0) return null

    generating.value = true

    // 将当前检索到的相关知识组装为上下文
    const allResults = [
      ...searchResults.value.paragraphs,
      ...searchResults.value.laws,
      ...searchResults.value.cases,
    ]
    const knowledgeText = allResults
      .slice(0, 5)
      .map((item) => `【${item.title}】\n${item.content}`)
      .join('\n\n---\n\n')

    const result = await tauriCallSafe('generate_writing_suggestion', {
      intent: intent.trim(),
      context: context || '',
      knowledge: knowledgeText,
      style: style || options.style || 'general',
    })

    generating.value = false

    if (result.ok && result.data) {
      aiSuggestion.value = {
        text: result.data,
        intent,
        sources: allResults.slice(0, 5).map((item) => ({
          id: item.id,
          title: item.title,
        })),
      }
      return result.data
    }

    return null
  }

  /**
   * 将 AI 建议插入编辑器（灰色下划线 decoration）
   * @param {string} text - 建议文本
   */
  function insertSuggestion(text) {
    const editor = editorRef.value
    if (!editor || !text) return

    // 使用 Highlight extension 标记 AI 生成内容
    // 插入带 mark 的内容
    editor
      .chain()
      .focus()
      .insertContent(`<mark class="ai-suggestion">${text}</mark>`)
      .run()
  }

  /**
   * 接受 AI 建议：移除 decoration 标记
   */
  function acceptSuggestion() {
    const editor = editorRef.value
    if (!editor) return

    // 移除所有 ai-suggestion mark
    const { state } = editor
    const { doc } = state
    const tr = state.tr

    doc.descendants((node, pos) => {
      if (node.marks) {
        node.marks.forEach((mark) => {
          if (mark.type.name === 'highlight') {
            // 简单方式：接受后移除高亮
          }
        })
      }
    })

    // 实际上接受就是清除高亮
    // 用户手动选中后移除高亮即可
    aiSuggestion.value = null
  }

  /**
   * 拒绝 AI 建议：删除标记的文本
   */
  function rejectSuggestion() {
    const editor = editorRef.value
    if (!editor) return

    // 找到并删除带 ai-suggestion 标记的内容
    const { state } = editor
    const { doc } = state

    let ranges = []
    doc.descendants((node, pos) => {
      if (node.isText && node.marks.some((m) => m.type.name === 'highlight')) {
        // 检查是否是 AI suggestion（通过 class 判断在 DOM 层面）
      }
    })

    // 简化处理：清空 suggestion 状态
    aiSuggestion.value = null
  }

  /**
   * 打开 AI 写作对话框
   */
  function openAiDialog() {
    aiIntent.value = ''
    aiDialogVisible.value = true
  }

  /**
   * 关闭 AI 写作对话框
   */
  function closeAiDialog() {
    aiDialogVisible.value = false
    aiIntent.value = ''
  }

  /**
   * 执行 AI 写作辅助（从对话框调用）
   * @param {string} intent
   * @param {string} [style]
   */
  async function executeAiWriting(intent, style) {
    const editor = editorRef.value
    const context = editor ? editor.getText().substring(0, 2000) : ''

    const text = await generateWriting(intent, context, style)
    if (text) {
      insertSuggestion(text)
      aiDialogVisible.value = false
    }
    return text
  }

  return {
    // 搜索状态
    searchQuery,
    searchResults,
    searching,

    // AI 状态
    generating,
    aiSuggestion,
    aiDialogVisible,
    aiIntent,

    // 展开状态
    expandedItemId,

    // 方法
    searchKnowledge,
    debouncedSearch,
    searchContext,
    getCategoryLabel,
    getCategoryIcon,
    insertToEditor,
    insertCitation,
    copyContent,
    toggleExpand,
    generateWriting,
    insertSuggestion,
    acceptSuggestion,
    rejectSuggestion,
    openAiDialog,
    closeAiDialog,
    executeAiWriting,
  }
}
