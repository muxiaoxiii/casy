// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { readFileSync } from 'node:fs'
import { applyThemePreference, disposeThemeListener, normalizeTheme, THEME_OPTIONS } from '../../src/shared/theme'

afterEach(() => { disposeThemeListener(); document.head.innerHTML = ''; vi.unstubAllGlobals() })

describe('theme compatibility', () => {
  it('retains the five original saved preferences and adds independent docket and paper themes', () => {
    for (const theme of ['slate', 'dark', 'luminous-terra', 'solarized-light', 'solarized-dark', 'docket-light', 'docket-dark', 'rice-paper']) {
      expect(normalizeTheme(theme)).toBe(theme)
      applyThemePreference(theme)
      expect(document.documentElement.dataset.theme).toBe(theme)
      expect(document.documentElement.style.colorScheme).toBe(['dark', 'solarized-dark', 'docket-dark'].includes(theme) ? 'dark' : 'light')
    }
    expect(THEME_OPTIONS.find(t => t.value === 'slate')?.label).toBe('石墨蓝')
    expect(THEME_OPTIONS.find(t => t.value === 'dark')?.label).toBe('暗夜深色')
    expect(normalizeTheme('unknown')).toBe('system')
  })
  it('system follows the original palettes and stops listening after an explicit choice', () => {
    let listener: (event: { matches: boolean }) => void
    const removeEventListener = vi.fn()
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: false, addEventListener: (_: string, cb: typeof listener) => { listener = cb }, removeEventListener })))
    applyThemePreference('system')
    expect(document.documentElement.dataset.theme).toBe('slate')
    listener!({ matches: true })
    expect(document.documentElement.dataset.theme).toBe('dark')
    applyThemePreference('rice-paper')
    expect(removeEventListener).toHaveBeenCalledOnce()
  })
  it('the loaded stylesheet cascade keeps a distinct palette for every explicit theme', () => {
    for (const path of ['theme', 'workspace', 'docket']) {
      const style = document.createElement('style')
      style.textContent = readFileSync(`src/assets/${path}.css`, 'utf8')
      document.head.appendChild(style)
    }
    const expected = { slate: '#F7F9FB', dark: '#111318', 'luminous-terra': '#FCF9F2', 'solarized-light': '#FAF7ED', 'solarized-dark': '#002B36', 'docket-light': '#f4f6fa', 'docket-dark': '#0b1220', 'rice-paper': '#f3ead7' }
    for (const [theme, color] of Object.entries(expected)) {
      applyThemePreference(theme)
      expect(getComputedStyle(document.documentElement).getPropertyValue('--c-bg-page').trim(), theme).toBe(color)
    }
  })
})
