/**
 * 知识捕获 composable
 *
 * 从 TipTap 编辑器选中文本创建知识条目，支持关联法条和案件。
 */
import { ref } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage } from 'element-plus'

/**
 * @param {Object} options
 * @param {string} [options.sourceType] - 来源类型（如 'editor', 'document'）
 * @param {string} [options.sourceId]   - 来源 ID（如文档/草稿 ID）
 * @param {string} [options.caseId]     - 当前关联的案件 ID
 */
export function useKnowledgeCapture(options = {}) {
  const capturing = ref(false)
  const showDialog = ref(false)
  const captureForm = ref({
    text: '',
    title: '',
    category: 'reference',
    tags: '',
    lawName: '',
    articleNo: '',
  })

  // 6 职能分类
  const captureCategories = [
    { value: 'inspiration', label: '灵感', color: '#8B5CF6' },
    { value: 'method', label: '方法', color: '#409EFF' },
    { value: 'reference', label: '参考', color: '#67C23A' },
    { value: 'question', label: '问题', color: '#F56C6C' },
    { value: 'experience', label: '经验', color: '#E6A23C' },
    { value: 'log', label: '日志', color: '#909399' },
  ]

  /**
   * 从选中文本发起捕获流程
   * @param {string} selectedText - 编辑器中选中的文本
   * @param {string} [title]      - 可选标题（默认取前 50 字符）
   */
  function startCapture(selectedText, title) {
    if (!selectedText || !selectedText.trim()) {
      ElMessage.warning('请先选中文本')
      return
    }
    captureForm.value = {
      text: selectedText.trim(),
      title: title || selectedText.trim().substring(0, 50),
      category: 'reference',
      tags: '',
      lawName: '',
      articleNo: '',
    }
    showDialog.value = true
  }

  /**
   * 确认捕获：创建知识条目
   * @returns {Promise<string|null>} 创建的知识条目 ID
   */
  async function confirmCapture() {
    if (!captureForm.value.text) {
      ElMessage.warning('内容不能为空')
      return null
    }

    capturing.value = true
    const result = await tauriCallSafe('create_knowledge', {
      data: {
        title: captureForm.value.title,
        category: captureForm.value.category,
        content: captureForm.value.text,
        tags: captureForm.value.tags || null,
        sourceType: options.sourceType || 'editor',
        sourceId: options.sourceId || null,
        linkedCaseId: options.caseId || null,
        lawName: captureForm.value.lawName || null,
        articleNo: captureForm.value.articleNo || null,
        status: 'current',
      },
    })

    capturing.value = false

    if (result.ok) {
      ElMessage.success('知识已入库')
      showDialog.value = false
      return result.data
    } else {
      ElMessage.error(result.error || '入库失败')
      return null
    }
  }

  /**
   * 直接从选中文本创建知识条目（无弹窗）
   * @param {string} text     - 选中文本
   * @param {string} source   - 来源描述
   * @param {string[]} tags   - 标签数组
   * @param {string} [category] - 分类
   * @returns {Promise<string|null>} 知识条目 ID
   */
  async function captureFromSelection(text, source, tags, category) {
    if (!text || !text.trim()) return null

    capturing.value = true
    const result = await tauriCallSafe('create_knowledge', {
      data: {
        title: text.trim().substring(0, 50),
        category: category || 'reference',
        content: text.trim(),
        tags: tags ? tags.join(',') : null,
        sourceType: source || 'editor',
        sourceId: options.sourceId || null,
        linkedCaseId: options.caseId || null,
        status: 'current',
      },
    })

    capturing.value = false

    if (result.ok) {
      ElMessage.success('知识已入库')
      return result.data
    } else {
      ElMessage.error(result.error || '入库失败')
      return null
    }
  }

  /**
   * 关联知识条目到法条
   * @param {string} knowledgeId - 知识条目 ID
   * @param {string} lawName     - 法律名称
   * @param {string} articleNo   - 条款号
   */
  async function linkToLawArticle(knowledgeId, lawName, articleNo) {
    const result = await tauriCallSafe('link_knowledge_to_law', {
      knowledgeId,
      lawName,
      articleNo,
    })
    if (result.ok) {
      ElMessage.success('已关联法条')
    } else {
      ElMessage.error(result.error || '关联失败')
    }
    return result.ok
  }

  /**
   * 关联知识条目到案件
   * @param {string} knowledgeId  - 知识条目 ID
   * @param {string} caseId       - 案件 ID
   * @param {string} relationType - 关系类型
   */
  async function linkToCase(knowledgeId, caseId, relationType = 'related') {
    const result = await tauriCallSafe('link_knowledge_to_case', {
      knowledgeId,
      caseId,
      relationType,
    })
    if (result.ok) {
      ElMessage.success('已关联案件')
    } else {
      ElMessage.error(result.error || '关联失败')
    }
    return result.ok
  }

  return {
    capturing,
    showDialog,
    captureForm,
    captureCategories,
    styleCategories,
    startCapture,
    confirmCapture,
    captureFromSelection,
    linkToLawArticle,
    linkToCase,
  }
}
