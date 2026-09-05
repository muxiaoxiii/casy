import { CASE_ROUTE_LABELS, CIVIL_STATUS_LABELS, INVALIDATION_STATUS_LABELS, ADMIN_STATUS_LABELS } from '../../../types'

const field = (key, label, type = 'text', options = []) => ({ key, label, type, options })
export const roleOptions = ['原告', '被告', '第三人', '上诉人', '被上诉人', '再审申请人', '再审被申请人', '请求人', '专利权人', '申请人', '被申请人']
export const intakeSections = [
  { key: 'basic', label: '基本信息', fields: [
    field('caseName','案件名称'), field('caseNo','案号'), field('internalNo','内部卷号'),
    field('externalCaseNo','金助理案号'),
    field('causeAction','案由','select',['专利无效','专利侵权','专利行政','商标行政','专利权属','技术秘密','著作权权属','外观侵权','合同纠纷','不正当竞争']),
    field('caseLevel','审级','enum',['一审','二审','再审','结案']),
    field('caseProgress','案件进展','select',['待立案','待提无效','待开庭','待口审','待判决','待无效决定','待补充意见','待意见陈述','待再审听证','中止','结案']),
    field('caseResult','案件结果','select',['胜诉','败诉','结案','对方撤案','撤诉','解除委托','胜诉 和解']),
    field('caseGoal','办案目标','textarea'),
  ]},
  { key: 'parties', label: '当事各方', fields: [
    field('clientName','我方当事人','suggest'), field('ourRole','我方诉讼地位','select',roleOptions),
    field('opponentName','对方当事人','suggest'), field('opponentRole','对方诉讼地位','select',roleOptions),
    field('opponentFirm','对方代理律所','suggest'), field('opponentAgent','对方代理人','suggest'),
    field('attorneys','办案人','tags'),
  ]},
  { key: 'procedure', label: '审理与专利', fields: [
    field('court','审理机关','suggest'), field('judgePanel','合议庭','suggest'), field('clerk','书记员 / 助理及联系方式'),
    field('procedureType','诉讼程序','enum',['普通','简易']), field('jurisdictionObjection','管辖异议','textarea'),
    field('patentName','专利名称'), field('patentAppNo','专利申请号'),
    field('claims','诉讼请求','textarea'),
  ]},
  { key: 'dates', label: '日期与期限', fields: [
    field('filingDate','立案日期','date'), field('complaintReceivedDate','收到起诉状日期','date'),
    field('trialDate','首次开庭 / 口审','datetime'), field('trial2Date','二次开庭 / 口审','datetime'),
    field('trial3Date','三次开庭 / 口审','datetime'),
    field('verdictType','裁判类型','select',['一审判决','二审判决','无效决定','裁定','和解撤诉','裁驳']),
    field('verdictDate','收到裁判日期','date'), field('stayDate','裁定中止日','date'),
    field('defenseDeadline','答辩期限','date'), field('reliefDeadline','救济期限'),
    field('estimatedTrialEnd','预估审限','date'),
    field('petitionerFirstInvalid','请求人首次无效日期','date'), field('petitionerSuppDeadline','请求人补充意见期限','date'),
    field('petitionerSubmitDate','请求人提交补充意见日期','date'), field('petitionerReceivedDate','请求人收到专利权人意见日期','date'),
    field('petitionerReplyDeadline','请求人答复意见期限','date'),
    field('patenteeReceivedDate','专利权人收到受理通知日期','date'), field('patenteeStatementDeadline','专利权人陈述意见期限','date'),
    field('patenteeReceivedSuppDate','专利权人收到补充意见日期','date'), field('patenteeSuppDeadline','专利权人补充意见期限','date'),
    field('patenteeSubmitSuppDate','专利权人提交补充意见日期','date'),
    field('invalidationDecisionDate','无效决定日期','date'), field('invalidationDecisionType','无效决定类型'),
    field('adminFilingDate','行政立案日期','date'), field('adminVerdictDate','行政裁判日期','date'), field('adminTrial2Date','行政二审日期','date'),
  ]},
  { key: 'finance', label: '费用与备注', fields: [
    field('caseAmount','标的额（元）','amount'), field('legalFees','律师费（元）','amount'),
    field('feePayment','律师费到账情况','textarea'), field('completedText','已完成事项','textarea'), field('notes','备注','textarea'),
  ]},
]
export const routeOptions = Object.entries(CASE_ROUTE_LABELS)
export const statusGroups = [
  { route: '民事诉讼', key: 'civilStatus', label: '民事状态', options: Object.entries(CIVIL_STATUS_LABELS) },
  { route: '专利无效', key: 'invalidationStatus', label: '无效状态', options: Object.entries(INVALIDATION_STATUS_LABELS) },
  { route: '行政诉讼', key: 'adminStatus', label: '行政状态', options: Object.entries(ADMIN_STATUS_LABELS) },
]
export function newIntake() {
  return { ...Object.fromEntries(intakeSections.flatMap(s => s.fields).map(f => [f.key, ''])),
    track: 'patent_invalidation', caseRoute: '专利无效', civilStatus: 'intake', invalidationStatus: 'preparing', adminStatus: 'filed',
    ourRole: '请求人', opponentRole: '专利权人', attorneys: [], thirdParties: [],
    hearings: [], logs: [], tasks: [], officials: [], relatedCases: [],
  }
}
export function setIntakeRoute(form, route) {
  const previous = form.track
  form.caseRoute = route
  form.track = route === '专利无效' ? 'patent_invalidation' : route === '行政诉讼' ? 'admin_litigation' : route === '其他' ? 'other' : 'civil_tort'
  const before = previous === 'patent_invalidation' ? ['请求人','专利权人'] : ['原告','被告']
  const after = form.track === 'patent_invalidation' ? ['请求人','专利权人'] : ['原告','被告']
  if (!form.ourRole || form.ourRole === before[0]) form.ourRole = after[0]
  if (!form.opponentRole || form.opponentRole === before[1]) form.opponentRole = after[1]
}
export function intakePayload(form) {
  const payload = structuredClone(JSON.parse(JSON.stringify(form)))
  for (const [key, value] of Object.entries(payload)) if (typeof value === 'string') payload[key] = value.trim()
  if (!payload.caseName) throw new Error('请输入案件名称')
  if (payload.thirdParties.some(p => !p.name.trim())) throw new Error('请填写第三人名称或删除空行')
  for (const key of ['caseAmount','legalFees']) {
    if (payload[key] !== '' && (!Number.isFinite(Number(payload[key])) || Number(payload[key]) < 0)) throw new Error('金额必须是非负数字')
  }
  payload.thirdParties = JSON.stringify(payload.thirdParties.map(p => Object.fromEntries(Object.entries(p).map(([k,v]) => [k, String(v).trim()]))))
  payload.caseLevel ||= null
  payload.procedureType ||= null
  for (const group of statusGroups) if (!(payload.caseRoute === '三轨并行' || payload.caseRoute?.includes(group.route))) payload[group.key] = null
  for (const [key,label,required] of [['hearings','庭审',['hearingName','hearingDate']],['logs','日志',['eventSummary','eventDate']],['tasks','任务',['taskName']],['officials','联系人',['name','role']]]) {
    if (payload[key].some(row => required.some(k => !String(row[k] || '').trim()))) throw new Error(`请补全${label}的必填信息`)
  }
  return payload
}
export function parseIntakeText(text) {
  const fields = new Map(intakeSections.flatMap(s => s.fields).map(f => [f.label, f.key]))
  for (const [label,key] of [['案件信息','caseName'],['客户名称','clientName'],['委托方','clientName'],['对方名称','opponentName'],['审理法院','court'],['诉讼请求','claims'],['我方诉讼地位','ourRole'],['诉讼地位','opponentRole'],['第三人','thirdParties']]) fields.set(label,key)
  const result = {}
  const unmatched = []
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue
    const match = line.match(/^\s*([^:：\t]+)[:：\t]\s*(.*)$/)
    const key = match && fields.get(match[1].trim())
    if (key && match[2].trim()) {
      if (key === 'thirdParties') (result.thirdParties ||= []).push({ name: match[2].trim(), role: '第三人', firm: '', agent: '', contact: '' })
      else if (Object.hasOwn(result, key)) unmatched.push(line)
      else result[key] = match[2].trim()
    }
    else unmatched.push(line)
  }
  return { fields: result, unmatched }
}
