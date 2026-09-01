<script setup lang="ts">
import { ref } from 'vue'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage } from 'element-plus'
import {
  Check,
  View,
  Document,
  Reading,
  DataAnalysis,
  Calendar,
  Grid,
  Memo,
  Postcard,
  Cpu,
} from '@element-plus/icons-vue'
import BriefingModal from '../../../shared/components/BriefingModal.vue'

const settingsStore = useSettingsStore()

// 预览模态框控制
const previewVisible = ref(false)
const previewType = ref<'daily' | 'weekly'>('daily')
const previewStyle = ref('')

// 8 大早报样式
const dailyStyles = [
  {
    id: 'gazette',
    name: 'The Casy Dispatch',
    tagline: '复古早报 · 锯齿撕边小票',
    tags: ['Newspaper', 'Tear-Off Receipt', 'Official Seal'],
    desc: '经典律政晨报与官方核验小票排版，带有报头刊号、硬红线专栏与条形码。',
    themeColor: '#244481',
  },
  {
    id: 'typewriter',
    name: 'Typewriter Classic',
    tagline: '打字机公文 · 极简黑白',
    tags: ['Serif Mono', 'Monochrome', 'High Contrast'],
    desc: '模仿经典机械打字机与正式律师备忘录，纯文本高对比度呈现。',
    themeColor: '#18181b',
  },
  {
    id: 'swiss-grid',
    name: 'Swiss Grid',
    tagline: '瑞士国际主义网格',
    tags: ['Modular Grid', 'Bold Sans', 'Metric-Forward'],
    desc: '严谨包豪斯/瑞士设计网格，大号数字指标与醒目红黑警报。',
    themeColor: '#b91c1c',
  },
  {
    id: 'vogue',
    name: 'Vogue Editorial',
    tagline: '时尚杂志 · 优雅衬线',
    tags: ['Fashion', 'Serif', 'Editorial'],
    desc: '宛如高级时尚杂志的排版，大号衬线字体与极致留白，提供优雅的晨读体验。',
    themeColor: '#0f172a',
  },
  {
    id: 'narrative-air',
    name: 'Narrative Air',
    tagline: '清风叙事 · 散文晨读',
    tags: ['Prose', 'Low Cognitive Load', 'Elegant'],
    desc: '段落式自然连贯叙述，大面积留白与柔和圆角，极低认知负荷。',
    themeColor: '#059669',
  },
  {
    id: 'glassmorphism',
    name: 'Aura Glass',
    tagline: '毛玻璃 · 弥散光晕',
    tags: ['Glassmorphism', 'Vibrant', 'Blur'],
    desc: '极具现代感的毛玻璃半透明材质与鲜艳弥散渐变光晕，带来时尚前卫的视觉冲击。',
    themeColor: '#a855f7',
  },
  {
    id: 'action-board',
    name: 'Action Kanban',
    tagline: '行动便签看板',
    tags: ['Sticky Notes', '3-Column Kanban', 'Actionable'],
    desc: '便签式三栏卡片（必达、红线、排期），手账与便签张贴感。',
    themeColor: '#ca8a04',
  },
  {
    id: 'executive',
    name: 'Executive Memo',
    tagline: '管理合伙人简报',
    tags: ['Gold Trim', 'KPI Row', 'High-End Legal'],
    desc: '律所执行合伙人专享简报，金色烫金饰边与高阶 KPI 聚焦。',
    themeColor: '#d97706',
  },
  {
    id: 'magic-prophet',
    name: 'The Daily Prophet',
    tagline: '魔法预言家日报',
    tags: ['Wizardry', 'Parchment', 'Serif Headline'],
    desc: '充满魔法气息的泛黄羊皮纸底纹，古典英伦衬线报头与密集排版。',
    themeColor: '#78350f',
  },
  {
    id: 'telegraph',
    name: 'Vintage Telegram',
    tagline: '老式电报机打字带',
    tags: ['Telegram', 'Monospace', 'Typewriter'],
    desc: '泛黄老旧的电报纸带，等宽大写字体，还原上世纪的紧迫复古感。',
    themeColor: '#d97706',
  },
  {
    id: 'bulletin',
    name: 'Police Bulletin',
    tagline: '复古警情通报 / 通缉令',
    tags: ['Wild West', 'Wanted Poster', 'Slab Serif'],
    desc: '粗重的西部风格字体与做旧纸张，仿佛张贴在警局门口的紧急布告。',
    themeColor: '#92400e',
  },
  {
    id: 'blueprint',
    name: 'Architectural Blueprint',
    tagline: '工程师蓝图 · 青花白线',
    tags: ['Cyanotype', 'Grid', 'Technical'],
    desc: '严谨的深蓝色底图配白色网格与制图手写体，展现极致的工程控制感。',
    themeColor: '#1e3a8a',
  },
  {
    id: 'terminal',
    name: 'Retro Terminal',
    tagline: '80年代终端机 · MS-DOS',
    tags: ['CLI', 'Phosphor Green', 'Hacker'],
    desc: '黑底绿字的复古计算机终端界面，块状光标与像素感极强的呈现。',
    themeColor: '#22c55e',
  },
  {
    id: 'polaroid',
    name: 'Vintage Polaroid',
    tagline: '拍立得底片 · 手写记事',
    tags: ['Polaroid', 'Marker Font', 'Shadow'],
    desc: '仿佛用马克笔写在拍立得相纸下方的随性记录，带着随性的生活气息。',
    themeColor: '#fafafa',
  },
  {
    id: 'ticket',
    name: 'Theatre Ticket',
    tagline: '复古剧院票根 / 登机牌',
    tags: ['Perforated', 'Barcode', 'Boarding Pass'],
    desc: '带有打孔边缘、流水号与虚线撕口的复古票根，极具收藏质感。',
    themeColor: '#be123c',
  },
  {
    id: 'scroll',
    name: 'Ancient Scroll',
    tagline: '东方卷轴 · 水墨印泥',
    tags: ['Scroll', 'Calligraphy', 'Inkan Seal'],
    desc: '宣纸纹理与朱砂红印泥，留白意境深远，如展开一幅水墨画轴。',
    themeColor: '#991b1b',
  },
]

// 8 大周报样式
const weeklyStyles = [
  {
    id: 'dossier',
    name: 'Executive Dossier',
    tagline: '卷宗档案特刊 · 绝密归档',
    tags: ['Folder Cover', 'Risk Exposure', 'Two-Column'],
    desc: '律政绝密档案封套质感，印章与双栏重大专案推进综述。',
    themeColor: '#8c5338',
  },
  {
    id: 'analytics',
    name: 'Neon Dashboard',
    tagline: '暗黑仪表盘 · 极光渐变',
    tags: ['Dark Mode', 'Neon', 'Dashboard'],
    desc: '深色模式下的沉浸式数据看板，荧光色彩与发光组件凸显关键数据。',
    themeColor: '#3b82f6',
  },
  {
    id: 'ledger',
    name: 'Supreme Ledger',
    tagline: '高定黑金台账',
    tags: ['Luxury', 'Black & Gold', 'Finance'],
    desc: '深邃黑底与烫金配色的奢华碰撞，将复式办案台账升格为顶级金融报告质感。',
    themeColor: '#fbbf24',
  },
  {
    id: 'milestones',
    name: 'Fluid Milestones',
    tagline: '流体时间轴 · 动感流线',
    tags: ['Fluid Design', 'Gradients', 'Timeline'],
    desc: '采用流体渐变与圆润气泡设计，让原本枯燥的案件流程呈现生动的视觉流向。',
    themeColor: '#ec4899',
  },
  {
    id: 'partner-brief',
    name: 'Senior Partner Review',
    tagline: '合伙人复盘信 · 典雅信函',
    tags: ['Formal Letter', 'Georgia Serif', 'Strategic Focus'],
    desc: '高级合伙人专属书信体（Dear Partner），宏观把控全局风控策略。',
    themeColor: '#78350f',
  },
  {
    id: 'focus-matrix',
    name: 'Neo Brutalism',
    tagline: '新粗野主义 · 潮流波普',
    tags: ['Neo Brutalism', 'Pop Art', 'High Contrast'],
    desc: '极具冲击力的粗体边框与明亮色块组合，新粗野主义设计打破沉闷。',
    themeColor: '#000000',
  },
  {
    id: 'chronicle',
    name: 'The Weekly Chronicle',
    tagline: '周度编年史 · 时序纵览',
    tags: ['Timeline Journal', 'Daily Flow', 'Archival'],
    desc: '周一至周五办案日志编年史，沉淀律所诉讼经验与裁判要旨。',
    themeColor: '#1e293b',
  },
  {
    id: 'cyber-matrix',
    name: 'Cyber Grid',
    tagline: '赛博战力周报 · HUD 仪表盘',
    tags: ['Cyberpunk HUD', 'Power Index', 'Glow Neon'],
    desc: 'HUD 战力评估仪表盘，AI 辅助量化战力指数与案件攻防态势。',
    themeColor: '#0284c7',
  },
  {
    id: 'hogwarts-letter',
    name: 'Hogwarts Acceptance',
    tagline: '魔法学校录取信函',
    tags: ['Emerald Ink', 'Wax Seal', 'Parchment'],
    desc: '厚重的羊皮纸，翡翠绿色的手写体，带有鲜红的火漆印章。',
    themeColor: '#065f46',
  },
  {
    id: 'classified-file',
    name: 'Classified Dossier',
    tagline: '冷战绝密档案 · TOP SECRET',
    tags: ['Manila Folder', 'Red Stamp', 'Redacted'],
    desc: '牛皮纸袋配色，醒目的红色机密印章与回形针，极具悬疑与保密氛围。',
    themeColor: '#ca8a04',
  },
  {
    id: 'vinyl-record',
    name: 'Vintage Vinyl',
    tagline: '70年代黑胶唱片封套',
    tags: ['Retro 70s', 'Circular Grooves', 'Groovy'],
    desc: '唱片纹路底纹与怀旧的70年代暖色系，带来如爵士乐般的复古情调。',
    themeColor: '#ea580c',
  },
  {
    id: 'tarot',
    name: 'Mystic Tarot',
    tagline: '神秘塔罗牌 · 哥特占星',
    tags: ['Gothic', 'Gold Ornaments', 'Mystic'],
    desc: '中世纪哥特风格与繁复对称的烫金边框，散发神秘学与星空元素。',
    themeColor: '#4c1d95',
  },
  {
    id: 'bank-note',
    name: 'Vintage Banknote',
    tagline: '复古防伪证券 / 纸钞',
    tags: ['Guilloche', 'Engraving', 'Currency'],
    desc: '繁复的防伪扭索纹理与复古雕刻排版，如同高价值的历史证券。',
    themeColor: '#166534',
  },
  {
    id: 'passport',
    name: 'International Passport',
    tagline: '签证护照 · 出入境盖章',
    tags: ['Passport Cover', 'Visa Stamps', 'MRZ'],
    desc: '深蓝皮纹，内部印满签证图章，底部附带防伪机读码。',
    themeColor: '#1e40af',
  },
  {
    id: 'steampunk',
    name: 'Steampunk Brass',
    tagline: '蒸汽朋克 · 齿轮与黄铜',
    tags: ['Victorian', 'Gears', 'Leather'],
    desc: '黄铜渐变、齿轮装饰与铆钉镶边，展现维多利亚时代的复古机械美学。',
    themeColor: '#b45309',
  },
  {
    id: 'wax-sealed-parchment',
    name: 'Royal Decree',
    tagline: '文艺复兴王室诏书',
    tags: ['Drop Caps', 'Blackletter', 'Gold Wax'],
    desc: '哥特黑体与华丽的下沉首字母（Drop Caps），辅以金箔与重火漆印。',
    themeColor: '#831843',
  },
]

async function selectDailyStyle(id: string) {
  settingsStore.daily_brief_style = id
  await settingsStore.save()
  ElMessage.success('已切换并保存早报样式')
}

async function selectWeeklyStyle(id: string) {
  settingsStore.weekly_report_style = id
  await settingsStore.save()
  ElMessage.success('已切换并保存周报样式')
}

function openPreview(type: 'daily' | 'weekly', styleId: string) {
  previewType.value = type
  previewStyle.value = styleId
  previewVisible.value = true
}
</script>

<template>
  <div class="briefing-settings-container">
    <div class="settings-section-header">
      <div class="header-titles">
        <h3 class="sec-title">早报与周报样式 (Briefing & Reports)</h3>
        <p class="sec-desc">
          为您的工作台配置自动化「每日早报」与「每周复盘报告」的视觉呈现风格。所有样式均支持一键全屏弹窗、复制与打印便签。
        </p>
      </div>
    </div>

    <!-- ═══ 1. 每日早报样式选择器 (8 种) ═══ -->
    <div class="report-style-group">
      <div class="group-title-bar">
        <div class="bar-left">
          <el-icon class="icon-blue"><Reading /></el-icon>
          <span class="group-name">每日早报样式 (Daily Pulse Reports)</span>
        </div>
        <span class="group-count">16 种可选风格</span>
      </div>

      <div class="styles-cards-grid">
        <div
          v-for="item in dailyStyles"
          :key="item.id"
          class="style-card"
          :class="{ active: settingsStore.daily_brief_style === item.id }"
          @click="selectDailyStyle(item.id)"
        >
          <!-- 选中徽章 -->
          <div v-if="settingsStore.daily_brief_style === item.id" class="active-badge">
            <span class="pulse-dot"></span>
            <span>当前使用</span>
          </div>

          <!-- 卡片头部色块与缩略图标 -->
          <div class="card-thumb-mock" :style="{ borderColor: item.themeColor }">
            <div class="thumb-inner-pattern" :class="`pattern-${item.id}`">
              <span class="thumb-label">{{ item.name }}</span>
            </div>
          </div>

          <!-- 卡片文字信息 -->
          <div class="card-info-body">
            <div class="card-title-row">
              <h4 class="c-name">{{ item.name }}</h4>
              <span class="c-tagline">{{ item.tagline }}</span>
            </div>
            <p class="c-desc">{{ item.desc }}</p>

            <div class="tags-row">
              <span v-for="tag in item.tags" :key="tag" class="chip-tag">{{ tag }}</span>
            </div>

            <!-- 底部操作按钮 -->
            <div class="card-actions-bar">
              <button
                class="btn-preview"
                @click.stop="openPreview('daily', item.id)"
              >
                <el-icon><View /></el-icon>
                <span>预览效果</span>
              </button>
              <button
                class="btn-select"
                :class="{ 'is-selected': settingsStore.daily_brief_style === item.id }"
              >
                <el-icon v-if="settingsStore.daily_brief_style === item.id"><Check /></el-icon>
                <span>{{ settingsStore.daily_brief_style === item.id ? '已应用' : '点击应用' }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ 2. 每周报告样式选择器 (8 种) ═══ -->
    <div class="report-style-group mt-8">
      <div class="group-title-bar">
        <div class="bar-left">
          <el-icon class="icon-amber"><DataAnalysis /></el-icon>
          <span class="group-name">每周复盘样式 (Weekly Synthesis Reports)</span>
        </div>
        <span class="group-count">16 种可选风格</span>
      </div>

      <div class="styles-cards-grid">
        <div
          v-for="item in weeklyStyles"
          :key="item.id"
          class="style-card"
          :class="{ active: settingsStore.weekly_report_style === item.id }"
          @click="selectWeeklyStyle(item.id)"
        >
          <!-- 选中徽章 -->
          <div v-if="settingsStore.weekly_report_style === item.id" class="active-badge">
            <span class="pulse-dot"></span>
            <span>当前使用</span>
          </div>

          <!-- 卡片头部色块与缩略图标 -->
          <div class="card-thumb-mock" :style="{ borderColor: item.themeColor }">
            <div class="thumb-inner-pattern" :class="`pattern-${item.id}`">
              <span class="thumb-label">{{ item.name }}</span>
            </div>
          </div>

          <!-- 卡片文字信息 -->
          <div class="card-info-body">
            <div class="card-title-row">
              <h4 class="c-name">{{ item.name }}</h4>
              <span class="c-tagline">{{ item.tagline }}</span>
            </div>
            <p class="c-desc">{{ item.desc }}</p>

            <div class="tags-row">
              <span v-for="tag in item.tags" :key="tag" class="chip-tag">{{ tag }}</span>
            </div>

            <!-- 底部操作按钮 -->
            <div class="card-actions-bar">
              <button
                class="btn-preview"
                @click.stop="openPreview('weekly', item.id)"
              >
                <el-icon><View /></el-icon>
                <span>预览效果</span>
              </button>
              <button
                class="btn-select"
                :class="{ 'is-selected': settingsStore.weekly_report_style === item.id }"
              >
                <el-icon v-if="settingsStore.weekly_report_style === item.id"><Check /></el-icon>
                <span>{{ settingsStore.weekly_report_style === item.id ? '已应用' : '点击应用' }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 实时预览模态框 -->
    <BriefingModal
      v-model:visible="previewVisible"
      :type="previewType"
      :style-variant="previewStyle"
    />
  </div>
</template>

<style scoped>
.briefing-settings-container {
  padding: 8px 4px 32px;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.settings-section-header {
  border-bottom: 1px solid var(--c-border);
  padding-bottom: 12px;
}

.sec-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0 0 4px;
}

.sec-desc {
  font-size: 12.5px;
  color: var(--c-text-secondary);
  line-height: 1.5;
  margin: 0;
}

.report-style-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.group-title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.bar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.icon-blue { color: var(--c-primary); font-size: 18px; }
.icon-amber { color: #d97706; font-size: 18px; }

.group-name {
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.group-count {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
  background: var(--c-bg-subtle);
  padding: 2px 8px;
  border-radius: 4px;
}

/* 4 列网格 */
.styles-cards-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 16px;
}

.style-card {
  position: relative;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-base) var(--ease-out);
}

.style-card:hover {
  transform: translateY(-2px);
  border-color: var(--c-border-strong);
  box-shadow: var(--shadow-md);
}

.style-card.active {
  border-color: var(--c-primary);
  box-shadow: 0 0 0 2px var(--c-primary-lighter);
}

.active-badge {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 10;
  background: rgba(0, 0, 0, 0.75);
  color: #fff;
  backdrop-filter: blur(6px);
  font-size: 10px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  gap: 4px;
}

.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--status-success);
  box-shadow: 0 0 6px var(--status-success);
}

.card-thumb-mock {
  height: 72px;
  background: var(--c-bg-page);
  border-bottom: 2px solid var(--c-border);
  position: relative;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}

.thumb-inner-pattern {
  width: 90%;
  height: 70%;
  border-radius: 4px;
  background: var(--c-bg-card);
  border: 1px dashed var(--c-border-strong);
  display: flex;
  align-items: center;
  justify-content: center;
}

.thumb-label {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
  color: var(--c-text-secondary);
}

.card-info-body {
  padding: 12px;
  display: flex;
  flex-direction: column;
  flex: 1;
  gap: 6px;
}

.card-title-row {
  display: flex;
  flex-direction: column;
}

.c-name {
  font-size: 13.5px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.c-tagline {
  font-size: 11px;
  color: var(--c-primary);
  font-weight: 600;
}

.c-desc {
  font-size: 11.5px;
  color: var(--c-text-secondary);
  line-height: 1.4;
  margin: 0;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  height: 32px;
}

.tags-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: auto;
  padding-top: 6px;
}

.chip-tag {
  font-family: var(--font-mono);
  font-size: 9.5px;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}

.card-actions-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px solid var(--c-border-light);
}

.btn-preview {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: 1px solid var(--c-border);
  color: var(--c-text-regular);
  font-size: 11px;
  font-weight: 600;
  padding: 4px 8px;
  border-radius: var(--c-radius-sm);
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-preview:hover {
  border-color: var(--c-primary);
  color: var(--c-primary);
  background: var(--c-primary-light);
}

.btn-select {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  color: var(--c-text-regular);
  font-size: 11px;
  font-weight: 600;
  padding: 4px 10px;
  border-radius: var(--c-radius-sm);
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-select.is-selected {
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  border-color: var(--c-primary);
}

.btn-select:hover:not(.is-selected) {
  background: var(--c-bg-hover);
  border-color: var(--c-border-strong);
}

.mt-8 { margin-top: 24px; }
</style>
