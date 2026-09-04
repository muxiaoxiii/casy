// @vitest-environment node
import { describe, it, expect } from 'vitest'
import {
  normalizeCase,
  normalizeCaseList,
  normalizeCaseAttorneys,
  normalizeCaseInput,
  joinAttorneys,
} from '../../src/core/caseNormalize'

describe('normalizeCaseAttorneys · 兼容三种来源形态', () => {
  it('null / undefined → []', () => {
    expect(normalizeCaseAttorneys(null)).toEqual([])
    expect(normalizeCaseAttorneys(undefined)).toEqual([])
    expect(normalizeCaseAttorneys('')).toEqual([])
  })

  it('数组 → 过滤非字符串元素并 trim', () => {
    expect(normalizeCaseAttorneys([' 张律师 ', '李律师'])).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys(['张律师', 42, '', '李律师'])).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys([])).toEqual([])
  })

  it('JSON 数组字符串 → 解析为数组', () => {
    expect(normalizeCaseAttorneys('["张律师","李律师"]')).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys('["张律师", "李律师"]')).toEqual(['张律师', '李律师'])
    // 含空字符串/非字符串元素
    expect(normalizeCaseAttorneys('["张律师", "", 42]')).toEqual(['张律师'])
  })

  it('裸分隔字符串 → 按分隔符切分（中英文逗号/顿号/分号/换行）', () => {
    expect(normalizeCaseAttorneys('张律师,李律师')).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys('张律师，李律师')).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys('张律师、李律师')).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys('张律师；李律师')).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys('张律师\n李律师')).toEqual(['张律师', '李律师'])
    expect(normalizeCaseAttorneys('张律师')).toEqual(['张律师'])
  })

  it('非法 JSON 数组字符串 → 回退按裸分隔字符串处理', () => {
    expect(normalizeCaseAttorneys('[张律师,李律师]')).toEqual(['张律师', '李律师'])
  })
})

describe('joinAttorneys · 防 .join 崩溃', () => {
  it('数组 → 以顿号拼接', () => {
    expect(joinAttorneys(['张律师', '李律师'])).toBe('张律师、李律师')
  })

  it('裸字符串 / JSON 字符串均无需调用方判断类型', () => {
    expect(joinAttorneys('张律师,李律师')).toBe('张律师、李律师')
    expect(joinAttorneys('["张律师","李律师"]')).toBe('张律师、李律师')
  })

  it('null / undefined / 空 → 空字符串（不再抛 TypeError）', () => {
    expect(joinAttorneys(null)).toBe('')
    expect(joinAttorneys(undefined)).toBe('')
    expect(joinAttorneys('')).toBe('')
  })

  it('自定义分隔符', () => {
    expect(joinAttorneys(['张律师', '李律师'], ', ')).toBe('张律师, 李律师')
  })
})

describe('normalizeCase · wire 可空字段归一化为业务 Case', () => {
  it('完整 wire Case → 业务 Case：可空字符串归为 ""，attorneys 归为数组', () => {
    const wireless = normalizeCase({
      id: 'c1',
      caseName: '隆基无效案',
      caseNo: '(2024)国知局第244号',
      track: 'patent_invalidation',
      caseRoute: '专利无效',
      caseStatus: '进行中',
      clientName: '隆基绿能',
      opponentName: '晶科能源',
      attorneys: '["张律师","李律师"]',
      internalNo: 'INV-244',
      folderTemplateId: 'tpl-1',
      deadlineUrgency: 'red',
      court: '国知局',
    })

    expect(wireless.attorneys).toEqual(['张律师', '李律师'])
    expect(wireless.attorneys).not.toBe('["张律师","李律师"]')
    expect(wireless.internalNo).toBe('INV-244')
    expect(wireless.folderTemplateId).toBe('tpl-1')
    expect(wireless.deadlineUrgency).toBe('red')
    expect(wireless.track).toBe('patent_invalidation')
    expect(wireless.caseRoute).toBe('专利无效')
    expect(wireless.caseStatus).toBe('进行中')
    // 未提供的可空字段归为空字符串
    expect(wireless.causeAction).toBe('')
    expect(wireless.court).toBe('国知局')
    expect(wireless.caseProgress).toBe('')
  })

  it('空/缺省输入 → 回退到合法默认值（枚举不空、字符串为 ""、attorneys 为 []）', () => {
    const empty = normalizeCase({})

    expect(empty.attorneys).toEqual([])
    expect(empty.track).toBe('other')
    expect(empty.caseStatus).toBe('未知')
    expect(empty.caseRoute).toBe('民事诉讼')
    expect(empty.causeAction).toBe('')
    expect(empty.internalNo).toBe('')
    expect(empty.folderTemplateId).toBe('')
    expect(empty.deadlineUrgency).toBeNull()
    expect(empty.civilStatus).toBeNull()
    expect(empty.caseLevel).toBeNull()
  })

  it('可空枚举字段保留 null，而非归为 ""', () => {
    const c = normalizeCase({ caseLevel: null, procedureType: null, verdictType: null, civilStatus: null })
    expect(c.caseLevel).toBeNull()
    expect(c.procedureType).toBeNull()
    expect(c.verdictType).toBeNull()
    expect(c.civilStatus).toBeNull()
  })

  it('幂等：已归一化的业务 Case 再次归一化不发生变化', () => {
    const once = normalizeCase({
      id: 'c1',
      caseName: 'A',
      caseNo: 'B',
      track: 'civil_tort',
      caseRoute: '民事诉讼',
      caseStatus: '已完结',
      clientName: '客户',
      opponentName: '对方',
      attorneys: ['张律师', '李律师'],
      internalNo: 'I',
      deadlineUrgency: 'yellow',
    })
    const twice = normalizeCase(once)

    expect(twice).toEqual(once)
    expect(twice.attorneys).toEqual(['张律师', '李律师'])
  })
})

describe('normalizeCaseInput · create/update 入参归一化', () => {
  it('attorneys 数组 → JSON 字符串（Rust 端 create_case 以 serde 解析为 Option<String>）', () => {
    expect(normalizeCaseInput({ caseName: 'A', attorneys: ['张律师', '李律师'] })).toEqual({
      caseName: 'A',
      attorneys: '["张律师","李律师"]',
    })
  })

  it('attorneys 字符串 → 归一为 JSON 数组字符串；空 → null', () => {
    expect(normalizeCaseInput({ attorneys: '张律师,李律师' })).toEqual({
      attorneys: '["张律师","李律师"]',
    })
    expect(normalizeCaseInput({ attorneys: '' })).toEqual({ attorneys: null })
  })

  it('保留其它字段（含 AI 网关授权 origin/proposalToken）', () => {
    const out = normalizeCaseInput({
      caseName: 'A',
      origin: 'ai',
      proposalToken: 'tok',
      attorneys: ['张律师'],
    })
    expect(out.origin).toBe('ai')
    expect(out.proposalToken).toBe('tok')
    expect(out.attorneys).toBe('["张律师"]')
  })

  it('无 attorneys 键 → 原样透传', () => {
    expect(normalizeCaseInput({ caseName: 'A', caseNo: 'B' })).toEqual({ caseName: 'A', caseNo: 'B' })
  })
})

describe('normalizeCaseList', () => {
  it('对列表逐条归一化', () => {
    const list = normalizeCaseList([
      { id: 'c1', attorneys: '["张律师"]', track: 'civil_tort' },
      { id: 'c2', attorneys: '李律师,王律师' },
    ])
    expect(list[0].attorneys).toEqual(['张律师'])
    expect(list[0].track).toBe('civil_tort')
    expect(list[1].attorneys).toEqual(['李律师', '王律师'])
    expect(list[1].caseRoute).toBe('民事诉讼')
  })
})
