import { createI18n } from 'vue-i18n'
import zhCN from './zh-CN.json'
import enUS from './en-US.json'

const savedLanguage = localStorage.getItem('casy_language') || 'zh-CN'

const i18n = createI18n({
  legacy: false, // use Composition API
  locale: savedLanguage,
  fallbackLocale: 'en-US',
  messages: {
    'zh-CN': zhCN,
    'en-US': enUS,
  },
})

export default i18n
