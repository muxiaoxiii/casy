import os
import re

modal_path = '/Users/only/Documents/PythonProgram/Casy/src/shared/components/BriefingModal.vue'
settings_path = '/Users/only/Documents/PythonProgram/Casy/src/modules/settings/components/BriefingStyleSettings.vue'

with open(settings_path, 'r', encoding='utf-8') as f:
    settings_code = f.read()

# Update settings
# Replace blueprint with vogue
settings_code = re.sub(
    r"id:\s*'blueprint',.*?desc:\s*'.*?',\s*themeColor:\s*'.*?'",
    "id: 'vogue',\n    name: 'Vogue Editorial',\n    tagline: '时尚杂志 · 优雅衬线',\n    tags: ['Fashion', 'Serif', 'Editorial'],\n    desc: '宛如高级时尚杂志的排版，大号衬线字体与极致留白，提供优雅的晨读体验。',\n    themeColor: '#0f172a'",
    settings_code,
    flags=re.DOTALL
)

# Replace terminal with glassmorphism
settings_code = re.sub(
    r"id:\s*'terminal',.*?desc:\s*'.*?',\s*themeColor:\s*'.*?'",
    "id: 'glassmorphism',\n    name: 'Aura Glass',\n    tagline: '毛玻璃 · 弥散光晕',\n    tags: ['Glassmorphism', 'Vibrant', 'Blur'],\n    desc: '极具现代感的毛玻璃半透明材质与鲜艳弥散渐变光晕，带来时尚前卫的视觉冲击。',\n    themeColor: '#a855f7'",
    settings_code,
    flags=re.DOTALL
)

# Update weekly styles to make them sound more diverse
settings_code = re.sub(
    r"id:\s*'focus-matrix',.*?desc:\s*'.*?',\s*themeColor:\s*'.*?'",
    "id: 'focus-matrix',\n    name: 'Neo Brutalism',\n    tagline: '新粗野主义 · 潮流波普',\n    tags: ['Neo Brutalism', 'Pop Art', 'High Contrast'],\n    desc: '极具冲击力的粗体边框与明亮色块组合，新粗野主义设计打破沉闷。',\n    themeColor: '#000000'",
    settings_code,
    flags=re.DOTALL
)

settings_code = re.sub(
    r"id:\s*'analytics',.*?desc:\s*'.*?',\s*themeColor:\s*'.*?'",
    "id: 'analytics',\n    name: 'Neon Dashboard',\n    tagline: '暗黑仪表盘 · 极光渐变',\n    tags: ['Dark Mode', 'Neon', 'Dashboard'],\n    desc: '深色模式下的沉浸式数据看板，荧光色彩与发光组件凸显关键数据。',\n    themeColor: '#3b82f6'",
    settings_code,
    flags=re.DOTALL
)

settings_code = re.sub(
    r"id:\s*'milestones',.*?desc:\s*'.*?',\s*themeColor:\s*'.*?'",
    "id: 'milestones',\n    name: 'Fluid Milestones',\n    tagline: '流体时间轴 · 动感流线',\n    tags: ['Fluid Design', 'Gradients', 'Timeline'],\n    desc: '采用流体渐变与圆润气泡设计，让原本枯燥的案件流程呈现生动的视觉流向。',\n    themeColor: '#ec4899'",
    settings_code,
    flags=re.DOTALL
)

settings_code = re.sub(
    r"id:\s*'ledger',.*?desc:\s*'.*?',\s*themeColor:\s*'.*?'",
    "id: 'ledger',\n    name: 'Supreme Ledger',\n    tagline: '高定黑金台账',\n    tags: ['Luxury', 'Black & Gold', 'Finance'],\n    desc: '深邃黑底与烫金配色的奢华碰撞，将复式办案台账升格为顶级金融报告质感。',\n    themeColor: '#fbbf24'",
    settings_code,
    flags=re.DOTALL
)

with open(settings_path, 'w', encoding='utf-8') as f:
    f.write(settings_code)
print("Updated settings!")
