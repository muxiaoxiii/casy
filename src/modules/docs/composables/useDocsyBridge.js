import { ref, computed } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'

/**
 * Docsy 桥接 composable
 * 提供模板列表、渲染、导出等功能
 */
export function useDocsyBridge() {
  const templates = ref([])
  const loading = ref(false)
  const error = ref(null)
  const renderResult = ref(null)
  const exportResult = ref(null)
  // 渲染请求序号/派生状态（P2-3 竞态修复）：
  // - renderRequestId 单调递增，仅“最新”请求允许写入 renderResult/error 并释放渲染忙碌状态；
  // - renderLoading 是渲染忙碌的唯一状态源：stale 请求绝不触碰它，
  //   因此旧请求既不会提前复位（把 spinner 关掉）也不会悬挂（最新请求或 clearRenderResult 必释放）。
  // - 与模板列表/导出共用 loading 不同，渲染用独立的 renderLoading，避免双重状态源互相打架。
  let renderRequestId = 0
  const renderLoading = ref(false)

  /**
   * 加载模板列表
   */
  async function loadTemplates() {
    loading.value = true
    error.value = null

    const result = await tauriCallSafe('list_docsy_templates', {})

    if (result.ok) {
      templates.value = result.data?.templates || []
    } else {
      error.value = result.error
      templates.value = []
    }

    loading.value = false
    return result
  }

  /**
   * 渲染模板（预览）
   * @param {string} templateId - 模板 ID
   * @param {string} caseId - 案件 ID
   */
  async function renderTemplate(templateId, caseId) {
    const requestId = ++renderRequestId
    renderLoading.value = true
    error.value = null

    const result = await tauriCallSafe('render_docsy_template', {
      templateId,
      caseId,
    })

    // 连续切换案件/模板时，旧请求不得覆盖较新的选择，
    // 也不得提前释放渲染忙碌状态（那属于最新请求的职责）。
    if (requestId !== renderRequestId) {
      return { ...result, stale: true, requestId }
    }

    if (result.ok) {
      renderResult.value = result.data
    } else {
      error.value = result.error
      renderResult.value = null
    }

    renderLoading.value = false
    return { ...result, stale: false, requestId }
  }

  function clearRenderResult() {
    renderRequestId += 1
    renderResult.value = null
    error.value = null
    renderLoading.value = false
  }

  /**
   * 导出 DOCX 文件
   * @param {string} templateId - 模板 ID
   * @param {string} caseId - 案件 ID
   * @param {string} [outputPath] - 输出路径（可选）
   */
  async function exportDocx(templateId, caseId, outputPath = null) {
    loading.value = true
    error.value = null

    const result = await tauriCallSafe('export_docx', {
      templateId,
      caseId,
      outputPath: outputPath || null,
    })

    if (result.ok) {
      exportResult.value = result.data
    } else {
      error.value = result.error
      exportResult.value = null
    }

    loading.value = false
    return result
  }

  /**
   * 按分类分组的模板列表
   */
  const templatesByCategory = computed(() => {
    const groups = {}
    for (const tpl of templates.value) {
      const cat = tpl.category || '其他'
      if (!groups[cat]) {
        groups[cat] = []
      }
      groups[cat].push(tpl)
    }
    return groups
  })

  /**
   * 搜索模板
   * @param {string} keyword - 搜索关键词
   */
  function searchTemplates(keyword) {
    if (!keyword) return templates.value
    const lower = keyword.toLowerCase()
    return templates.value.filter(
      (t) =>
        t.name.toLowerCase().includes(lower) ||
        t.category.toLowerCase().includes(lower) ||
        (t.description || '').toLowerCase().includes(lower)
    )
  }

  return {
    // 状态
    templates,
    loading,
    error,
    renderResult,
    renderLoading,
    exportResult,

    // 计算属性
    templatesByCategory,

    // 方法
    loadTemplates,
    renderTemplate,
    clearRenderResult,
    exportDocx,
    searchTemplates,
  }
}

/**
 * 将案件数据映射为模板值（40+ 字段）
 * 前端版本，用于预览和编辑
 *
 * @param {Object} caseData - 案件对象
 * @param {Object} [settings={}] - 设置（律所名称等）
 * @returns {Object} Docsy values 对象
 */
export function mapCaseToTemplate(caseData, settings = {}) {
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
    立案日期: 'filingDate',
    收到起诉状日期: 'complaintReceivedDate',
    开庭日期: 'trialDate',
    二审日期: 'trial2Date',
    三审日期: 'trial3Date',
    判决日期: 'verdictDate',
    中止日期: 'stayDate',
    救济期限: 'reliefDeadline',
    请求人首次无效日期: 'petitionerFirstInvalid',
    请求人补充意见期限: 'petitionerSuppDeadline',
    请求人提交日期: 'petitionerSubmitDate',
    请求人收到日期: 'petitionerReceivedDate',
    请求人答复期限: 'petitionerReplyDeadline',
    专利权人收到日期: 'patenteeReceivedDate',
    专利权人陈述期限: 'patenteeStatementDeadline',
    专利权人收到补充日期: 'patenteeReceivedSuppDate',
    专利权人补充期限: 'patenteeSuppDeadline',
    专利权人提交补充日期: 'patenteeSubmitSuppDate',
  }

  for (const [tplField, caseKey] of Object.entries(dateFields)) {
    values[tplField] = caseData[caseKey] ? formatDateStr(caseData[caseKey]) : ''
  }

  // 今日日期
  values['日期'] = formatDate(new Date())
  values['今日日期'] = formatDate(new Date())

  // ---- party_list 字段 → [{name, suffix}] 数组 ----
  const ourParties = []
  if (caseData.clientName) {
    ourParties.push({
      name: caseData.clientName,
      suffix: caseData.ourRole || '请求人',
    })
  }
  values['我方当事人'] = ourParties

  const opponentParties = []
  if (caseData.opponentName) {
    opponentParties.push({
      name: caseData.opponentName,
      suffix: caseData.opponentRole || '被请求人',
    })
  }
  if (caseData.opponentAgent) {
    opponentParties.push({
      name: caseData.opponentAgent,
      suffix: '代理人',
    })
  }
  values['对方当事人'] = opponentParties

  // 合并当事人列表
  values['当事人'] = [...ourParties, ...opponentParties]

  // ---- reference 字段 ----
  values['审理机关'] = caseData.court || ''
  values['审级'] = caseData.caseLevel || ''
  values['对方代理律所'] = caseData.opponentFirm || ''

  // ---- checkbox/radio 字段 ----
  values['普通程序'] = caseData.procedureType === '普通'
  values['简易程序'] = caseData.procedureType === '简易'
  values['判决类型'] = caseData.verdictType || ''
  values['胜诉'] = caseData.caseResult === '胜诉'
  values['败诉'] = caseData.caseResult === '败诉'
  values['部分胜诉'] = caseData.caseResult === '部分胜诉'

  // 清理空值
  for (const key of Object.keys(values)) {
    if (values[key] === undefined || values[key] === null) {
      values[key] = typeof values[key] === 'boolean' ? false : ''
    }
  }

  return values
}

/**
 * 将映射结果转换为字段行数组（用于表格展示）
 * @param {Object} values - mapCaseToTemplate 的返回值
 * @returns {Array<{field: string, value: string, type: string}>}
 */
export function mapToFieldRows(values) {
  if (!values) return []

  return Object.entries(values).map(([field, value]) => {
    let displayValue = ''
    let type = 'text'

    if (Array.isArray(value)) {
      // party_list
      type = 'party_list'
      displayValue = value
        .map((p) => {
          if (typeof p === 'object' && p.name) {
            return p.suffix ? `${p.name}(${p.suffix})` : p.name
          }
          return String(p)
        })
        .join('、')
    } else if (typeof value === 'boolean') {
      type = 'checkbox'
      displayValue = value ? '✓' : '✗'
    } else if (value === '' || value === null || value === undefined) {
      displayValue = '(空)'
    } else {
      displayValue = String(value)
    }

    return { field, value: displayValue, type }
  })
}

// ---- 辅助函数 ----

function formatDate(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function formatDateStr(s) {
  if (!s) return ''
  // 已经是 YYYY-MM-DD 格式
  if (/^\d{4}-\d{2}-\d{2}/.test(s)) return s.slice(0, 10)
  // 飞书时间戳（毫秒）
  if (/^\d{13}$/.test(s)) {
    return formatDate(new Date(Number(s)))
  }
  return s
}
