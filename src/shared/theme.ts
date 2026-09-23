export const THEME_OPTIONS = [
  {
    value: 'system',
    label: '跟随系统',
    description: '在石墨蓝与暗夜主题之间自动切换',
    swatches: ['#F7F9FB', '#FFFFFF', '#244481'],
  },
  {
    value: 'slate',
    label: '石墨蓝',
    description: '冷静、清晰，适合长时间专业工作',
    swatches: ['#F7F9FB', '#FFFFFF', '#244481'],
  },
  {
    value: 'luminous-terra',
    label: '暖砂大地',
    description: '柔和的暖灰底色与森林绿重点色',
    swatches: ['#FCF9F2', '#FFFFFF', '#416353'],
  },
  {
    value: 'solarized-light',
    label: '日光浅色',
    description: '偏暖白的纯色工作面，柔和蓝色重点',
    swatches: ['#FAF7ED', '#FFFDF7', '#247BAA'],
  },
  {
    value: 'solarized-dark',
    label: '日光深色',
    description: '蓝绿色深色背景，保留柔和语义色',
    swatches: ['#002B36', '#073642', '#2AA6D6'],
  },
  {
    value: 'dark',
    label: '暗夜深色',
    description: '中性深灰表面与更清晰的蓝色焦点',
    swatches: ['#111318', '#181D27', '#6487D4'],
  },
  {
    value: 'docket-light',
    label: '卷宗明亮',
    description: '卷宗墨卷 · 清透灰蓝与印章蓝',
    swatches: ['#F4F6FA', '#FFFFFF', '#1A4FD6'],
  },
  {
    value: 'docket-dark',
    label: '卷宗墨色',
    description: '卷宗墨卷 · 深墨蓝与柔和亮蓝',
    swatches: ['#0B1220', '#111C2E', '#8AAFFF'],
  },
  {
    value: 'rice-paper',
    label: '黄宣纸',
    description: '淡米色纸纹、烟墨文字与朱砂印色',
    swatches: ['#F3EAD7', '#FCF6E8', '#924536'],
  },
] as const

export type ThemePreference = (typeof THEME_OPTIONS)[number]['value']
export type EffectiveTheme = Exclude<ThemePreference, 'system'>

const VALID_THEMES = new Set<ThemePreference>(THEME_OPTIONS.map((option) => option.value))
const DARK_THEMES = new Set<EffectiveTheme>(['solarized-dark', 'dark', 'docket-dark'])

let systemMedia: MediaQueryList | null = null
let systemMediaListener: ((event: MediaQueryListEvent) => void) | null = null

export function normalizeTheme(theme: unknown): ThemePreference {
  return typeof theme === 'string' && VALID_THEMES.has(theme as ThemePreference)
    ? theme as ThemePreference
    : 'system'
}

function resolveSystemTheme(): EffectiveTheme {
  if (typeof window === 'undefined') return 'slate'
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'slate'
}

function setEffectiveTheme(preference: ThemePreference, effectiveTheme: EffectiveTheme) {
  const root = document.documentElement
  root.dataset.themePreference = preference
  root.dataset.theme = effectiveTheme
  root.style.colorScheme = DARK_THEMES.has(effectiveTheme) ? 'dark' : 'light'
}

function clearSystemListener() {
  if (systemMedia && systemMediaListener) {
    systemMedia.removeEventListener('change', systemMediaListener)
  }
  systemMedia = null
  systemMediaListener = null
}

export function applyThemePreference(rawTheme: unknown): ThemePreference {
  const preference = normalizeTheme(rawTheme)
  clearSystemListener()

  if (preference === 'system') {
    systemMedia = window.matchMedia('(prefers-color-scheme: dark)')
    setEffectiveTheme(preference, resolveSystemTheme())
    systemMediaListener = (event) => {
      setEffectiveTheme('system', event.matches ? 'dark' : 'slate')
    }
    systemMedia.addEventListener('change', systemMediaListener)
  } else {
    setEffectiveTheme(preference, preference)
  }

  return preference
}

export function disposeThemeListener() {
  clearSystemListener()
}
