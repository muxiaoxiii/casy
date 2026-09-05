import { describe, expect, it } from 'vitest'
import { newIntake, intakePayload, parseIntakeText, setIntakeRoute } from '../../src/modules/cases/components/caseIntake'

describe('case intake', () => {
  it('preserves roles, third parties, money, dates, and children through the payload', () => {
    const form = newIntake()
    Object.assign(form,{caseName:'  测试案件  ',ourRole:'专利权人',opponentRole:'请求人',caseAmount:'100.50',filingDate:'2026-09-01',
      thirdParties:[{name:'第三人甲',role:'第三人',agent:'代理人甲',firm:'律所甲',contact:'123'}],
      hearings:[{hearingName:'口审',hearingDate:'2026-09-20 09:30:00'}],
      relatedCases:[{caseId:'case-other',relationType:'same_patent'}],
    })
    const result = intakePayload(form)
    expect(result.caseName).toBe('测试案件')
    expect(result.ourRole).toBe('专利权人')
    expect(JSON.parse(result.thirdParties)).toEqual(form.thirdParties)
    expect(result.caseAmount).toBe('100.50')
    expect(result.hearings[0].hearingDate).toBe('2026-09-20 09:30:00')
    expect(result.relatedCases[0].caseId).toBe('case-other')
    expect(result.civilStatus).toBeNull()
    expect(result.caseLevel).toBeNull()
  })
  it('changes default roles with the procedure but retains explicitly chosen roles', () => {
    const form = newIntake()
    setIntakeRoute(form,'民事诉讼')
    expect(form.ourRole).toBe('原告')
    form.ourRole = '第三人'
    setIntakeRoute(form,'专利无效')
    expect(form.ourRole).toBe('第三人')
    setIntakeRoute(form,'其他')
    form.caseName='仲裁事项'
    expect(intakePayload(form)).toMatchObject({track:'other',caseRoute:'其他',invalidationStatus:null})
  })
  it('rejects incomplete children and invalid amounts without mutating the draft', () => {
    const form = newIntake()
    form.caseName='测试案件'
    form.thirdParties=[{name:' '}]
    expect(()=>intakePayload(form)).toThrow('第三人')
    expect(form.thirdParties).toEqual([{name:' '}])
    form.thirdParties=[]
    form.caseAmount='-2'
    expect(()=>intakePayload(form)).toThrow('金额')
    form.caseAmount=''
    form.hearings=[{hearingName:'口审',hearingDate:''}]
    expect(()=>intakePayload(form)).toThrow('庭审')
  })
  it('parses labeled source text and retains unmatched content for review', () => {
    const result=parseIntakeText('案件信息：测试案件\n客户名称：客户甲\n第三人：第三人甲\n待核实的事实')
    expect(result.fields).toMatchObject({caseName:'测试案件',clientName:'客户甲',thirdParties:[{name:'第三人甲'}]})
    expect(result.unmatched).toEqual(['待核实的事实'])
  })
  it('retains every third party and preserves conflicting source lines', () => {
    const result = parseIntakeText('第三人：甲公司\n第三人：乙公司\n案号：案号甲\n案号：案号乙')
    expect(result.fields.thirdParties.map(p => p.name)).toEqual(['甲公司', '乙公司'])
    expect(result.fields.caseNo).toBe('案号甲')
    expect(result.unmatched).toEqual(['案号：案号乙'])
  })
})
