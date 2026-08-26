import { describe, expect, it } from 'vitest'
import { normalizeCombo } from '../src/shared/keyboard'

describe('KeyboardCenter · normalizeCombo', () => {
  it('修饰键排序归一', () => {
    expect(normalizeCombo('ctrl+alt+k')).toBe('alt+ctrl+k')
    expect(normalizeCombo('Meta + K')).toBe('meta+k')
  })

  it('无修饰键仅主键', () => {
    expect(normalizeCombo('Escape')).toBe('escape')
  })

  it('空串安全', () => {
    expect(normalizeCombo('')).toBe('')
  })
})
