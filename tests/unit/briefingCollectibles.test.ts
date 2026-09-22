// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import BriefingModal from '../../src/shared/components/BriefingModal.vue'
import { briefingStyleOptions, dailyBriefingStyles, weeklyBriefingStyles } from '../../src/shared/briefingStyles'

vi.mock('html2canvas', () => ({ default: vi.fn() }))

describe('收藏版简报', () => {
  it('保留原预言家 ID，所有模板 ID 唯一且正确分组', () => {
    expect(new Set(briefingStyleOptions.map(style => style.id)).size).toBe(briefingStyleOptions.length)
    expect(dailyBriefingStyles.find(style => style.id === 'magic-prophet')?.label).toBe('预言家日报')
    expect(dailyBriefingStyles.some(style => style.id === 'receipt')).toBe(true)
    expect(weeklyBriefingStyles.some(style => style.id === 'lunar-log')).toBe(true)
    expect(weeklyBriefingStyles.some(style => style.id === 'airmail')).toBe(true)
  })

  it.each(['magic-prophet', 'receipt', 'herbarium', 'lunar-log', 'airmail'])('%s 保留真实标题、行动与正文，不填充样例', (styleVariant) => {
    const wrapper = mount(BriefingModal, {
      props: { visible: true, styleVariant, title: '我的真实简报', content: '本周已完成材料核对', nextAction: { taskName: '核对证据原件' } },
      global: { stubs: { teleport: true, transition: false, 'el-icon': true } },
    })
    expect(wrapper.find('article.report-sheet').attributes('data-style')).toBe(styleVariant)
    expect(wrapper.text()).toContain('我的真实简报')
    expect(wrapper.text()).toContain('核对证据原件')
    expect(wrapper.text()).toContain('本周已完成材料核对')
    expect(wrapper.text()).not.toContain('SAMPLE /')
    expect(wrapper.find('.collectible-art').exists()).toBe(styleVariant !== 'receipt')
    expect(wrapper.find('.receipt-barcode').exists()).toBe(styleVariant === 'receipt')
    wrapper.unmount()
  })
})
