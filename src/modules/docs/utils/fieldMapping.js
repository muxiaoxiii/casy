// ============================================================
// Docsy 字段映射纯模块（不依赖 Vue / IPC / 组件）
// ============================================================
// 将案件数据映射为 Docsy 模板字段填充值，并转换为「字段映射预览」表格行。
// 从 useDocsyBridge 抽出沉淀为纯函数，便于单测与复用（预览与编辑共用同一份映射）。
// 注意：本模块不触碰任何渲染/导出竞态逻辑，那些在 useDocsyBridge 与视图层。

import { joinAttorneys } from '../../../core/caseNormalize'

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
  values['律师'] = joinAttorneys(caseData.attorneys)

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

/**
 * 按关键字过滤字段行（field / value 均参与匹配，大小写不敏感）。
 * @param {Array<{field: string, value: string, type: string}>} rows
 * @param {string} keyword
 * @returns {Array<{field: string, value: string, type: string}>}
 */
export function filterFieldRows(rows, keyword) {
  if (!keyword) return rows
  const lower = keyword.toLowerCase()
  return rows.filter(
    (r) =>
      r.field.toLowerCase().includes(lower) ||
      r.value.toLowerCase().includes(lower)
  )
}

/**
 * 字段类型 → 展示标签（用于表格「类型」列）。
 * @param {string} type
 * @returns {string}
 */
export function fieldTypeLabel(type) {
  const labels = {
    text: '文本',
    date: '日期',
    party_list: '当事人',
    checkbox: '勾选',
    radio_group: '单选',
  }
  return labels[type] || type
}

/**
 * 字段类型 → el-tag type（用于表格「类型」列标签配色）。
 * @param {string} type
 * @returns {string}
 */
export function fieldTypeTag(type) {
  const tags = {
    text: '',
    date: 'warning',
    party_list: 'success',
    checkbox: 'info',
    radio_group: 'danger',
  }
  return tags[type] || ''
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
