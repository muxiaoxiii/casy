<script setup lang="ts">
import { ref } from 'vue'
import { useSettingsStore } from '../../../stores/settings'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import {
  View,
  Reading,
  DataAnalysis,
} from '../../../shared/icons'
import BriefingModal from '../../../shared/components/BriefingModal.vue'
import BriefingIllustration from '../../../shared/components/briefing/BriefingIllustration.vue'
import { dailyBriefingStyles as dailyStyles, weeklyBriefingStyles as weeklyStyles } from '../../../shared/briefingStyles'

const settingsStore = useSettingsStore()

// 预览模态框控制
const previewVisible = ref(false)
const previewType = ref<'daily' | 'weekly'>('daily')
const previewStyle = ref('')

const saving = ref(false)
async function saveStyle(key: 'daily_brief_style' | 'weekly_report_style', id: string) {
  if (saving.value || settingsStore[key] === id) return
  saving.value = true
  try {
    const result = await casyContext.settings.save({ [key]: id })
    if (!result.ok) throw new Error(result.error || '样式保存失败，请重试')
    settingsStore[key] = id
    settingsStore.persisted[key] = id
    ElMessage.success('样式已保存')
  } catch (e) { ElMessage.error(String(e)) }
  finally { saving.value = false }
}
function selectDailyStyle(id: string) { return saveStyle('daily_brief_style', id) }
function selectWeeklyStyle(id: string) { return saveStyle('weekly_report_style', id) }

function openPreview(type: 'daily' | 'weekly', styleId: string) {
  previewType.value = type
  previewStyle.value = styleId
  previewVisible.value = true
}
</script>

<template>
  <div class="briefing-settings-container" :aria-busy="saving">
    <div class="settings-section-header">
      <div class="header-titles">
        <h3 class="sec-title">早报与周报样式 (Briefing & Reports)</h3>
        <p class="sec-desc">
          {{ dailyStyles.length + weeklyStyles.length }} 套可收藏的日报与周报，让认真度过的日子有迹可循。共享真实数据，预览明确标注样例，支持高清 PNG 长图。
        </p>
      </div>
    </div>

    <!-- ═══ 1. 每日早报样式选择器 (16 种) ═══ -->
    <div class="report-style-group">
      <div class="group-title-bar">
        <div class="bar-left">
          <el-icon class="icon-blue"><Reading /></el-icon>
          <span class="group-name">每日早报样式 (Daily Pulse Reports)</span>
        </div>
        <span class="group-count">{{ dailyStyles.length }} 种可选风格</span>
      </div>

      <div class="styles-cards-grid">
        <div
          v-for="item in dailyStyles"
          :key="item.id"
          class="style-card"
          :class="{ active: settingsStore.daily_brief_style === item.id }"
          tabindex="0"
          role="button" :aria-disabled="saving"
          :aria-label="`应用${item.label}`"
          :aria-pressed="settingsStore.daily_brief_style === item.id"
          @keydown.enter.self.prevent="selectDailyStyle(item.id)"
          @keydown.space.self.prevent="selectDailyStyle(item.id)"
          @click="selectDailyStyle(item.id)"
        >
          <!-- 选中徽章 -->
          <div v-if="settingsStore.daily_brief_style === item.id" class="active-badge">
            <span class="pulse-dot"></span>
            <span>当前使用</span>
          </div>

          <div
            class="card-thumb-mock"
            :data-style="item.id"
            :data-tone="item.tone"
            :data-family="item.family"
            :style="{ '--style-color': item.themeColor }"
          >
            <div class="thumb-sheet">
              <BriefingIllustration v-if="['magic-prophet', 'herbarium'].includes(item.id)" :variant="item.id" class="thumb-illustration" />
              <span class="thumb-ornament" aria-hidden="true">C</span>
              <span class="thumb-kicker">CASY / {{ item.label }}</span>
              <strong>{{ item.name }}</strong>
              <div class="thumb-rule"></div>
              <div class="thumb-columns"><i></i><i></i><i></i></div>
            </div>
          </div>

          <!-- 卡片文字信息 -->
          <div class="card-info-body">
            <div class="card-title-row">
              <h4 class="c-name">{{ item.label }}</h4>
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
              <span class="apply-hint">{{ settingsStore.daily_brief_style === item.id ? '已应用' : '点击卡片应用' }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ 2. 每周报告样式选择器 (16 种) ═══ -->
    <div class="report-style-group mt-8">
      <div class="group-title-bar">
        <div class="bar-left">
          <el-icon class="icon-amber"><DataAnalysis /></el-icon>
          <span class="group-name">每周复盘样式 (Weekly Synthesis Reports)</span>
        </div>
        <span class="group-count">{{ weeklyStyles.length }} 种可选风格</span>
      </div>

      <div class="styles-cards-grid">
        <div
          v-for="item in weeklyStyles"
          :key="item.id"
          class="style-card"
          :class="{ active: settingsStore.weekly_report_style === item.id }"
          tabindex="0"
          role="button" :aria-disabled="saving"
          :aria-label="`应用${item.label}`"
          :aria-pressed="settingsStore.weekly_report_style === item.id"
          @keydown.enter.self.prevent="selectWeeklyStyle(item.id)"
          @keydown.space.self.prevent="selectWeeklyStyle(item.id)"
          @click="selectWeeklyStyle(item.id)"
        >
          <!-- 选中徽章 -->
          <div v-if="settingsStore.weekly_report_style === item.id" class="active-badge">
            <span class="pulse-dot"></span>
            <span>当前使用</span>
          </div>

          <div
            class="card-thumb-mock"
            :data-style="item.id"
            :data-tone="item.tone"
            :data-family="item.family"
            :style="{ '--style-color': item.themeColor }"
          >
            <div class="thumb-sheet">
              <BriefingIllustration v-if="['lunar-log', 'airmail'].includes(item.id)" :variant="item.id" class="thumb-illustration" />
              <span class="thumb-ornament" aria-hidden="true">C</span>
              <span class="thumb-kicker">CASY / {{ item.label }}</span>
              <strong>{{ item.name }}</strong>
              <div class="thumb-rule"></div>
              <div class="thumb-columns"><i></i><i></i><i></i></div>
            </div>
          </div>

          <!-- 卡片文字信息 -->
          <div class="card-info-body">
            <div class="card-title-row">
              <h4 class="c-name">{{ item.label }}</h4>
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
              <span class="apply-hint">{{ settingsStore.weekly_report_style === item.id ? '已应用' : '点击卡片应用' }}</span>
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
      preview
    />
  </div>
</template>

<style scoped>
.style-card:focus-visible { outline: 2px solid var(--c-accent, #49634a); outline-offset: 4px; }
.thumb-illustration { position: absolute; right: 4px; bottom: 3px; width: 100px; height: 64px; color: var(--style-color); }
.card-thumb-mock:has(.thumb-illustration) .thumb-columns { max-width: 48%; }
.card-thumb-mock[data-style='magic-prophet'] .thumb-sheet .thumb-ornament { display: none; }
.card-thumb-mock[data-style='receipt'] .thumb-sheet { width: 55%; margin-inline: auto; border-block: 3px dotted #697064; border-radius: 0; background: #faf9f3; font-family: monospace; }
.card-thumb-mock[data-style='receipt'] .thumb-rule { height: 14px; background: repeating-linear-gradient(90deg, #45483f 0 2px, transparent 2px 4px, #45483f 4px 5px, transparent 5px 8px); }
.card-thumb-mock[data-style='herbarium'] .thumb-sheet { background: #f0f0e4; border: 5px solid #dce0cd; }
.card-thumb-mock[data-style='lunar-log'] .thumb-sheet { background: #14242e; color: #e2e9ed; border: 1px solid #b6cfdf70; }
.card-thumb-mock[data-style='airmail'] .thumb-sheet { background: #f8f0dd; border-left: 5px dashed #a64c3d; border-right: 5px dashed #476b80; }
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
  --style-color: var(--c-primary);
  --thumb-paper: #f5f0e5;
  --thumb-ink: #27231f;
  height: 112px;
  padding: 13px 18px 0;
  background: #d8d5ce;
  border-bottom: 1px solid var(--c-border);
  position: relative;
  overflow: hidden;
}

.thumb-sheet {
  position: relative;
  height: 100%;
  padding: 12px 14px;
  overflow: hidden;
  color: var(--thumb-ink);
  background: var(--thumb-paper);
  border-radius: 2px 2px 0 0;
  box-shadow: 0 7px 18px rgba(15, 23, 42, 0.18);
  transform: rotate(-0.4deg);
}

.thumb-ornament {
  position: absolute;
  top: 10px;
  right: 11px;
  display: none;
  place-items: center;
  width: 26px;
  height: 26px;
  color: var(--style-color);
  border: 1px solid var(--style-color);
  border-radius: 50%;
  font: 700 13px/1 Georgia, serif;
  opacity: 0.55;
}

.thumb-kicker {
  display: block;
  font-family: var(--font-mono);
  font-size: 7px;
  font-weight: 700;
  color: var(--style-color);
  letter-spacing: 0.06em;
}

.thumb-sheet strong {
  display: block;
  margin-top: 7px;
  overflow: hidden;
  font-family: Georgia, serif;
  font-size: 13px;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thumb-rule {
  height: 2px;
  margin: 8px 0 6px;
  background: var(--style-color);
}

.thumb-columns {
  display: grid;
  grid-template-columns: 1.4fr 1fr 1fr;
  gap: 5px;
}

.thumb-columns i {
  display: block;
  height: 13px;
  border-top: 1px solid currentColor;
  border-bottom: 1px solid currentColor;
  opacity: 0.42;
}

.card-thumb-mock[data-tone='dark'] { --thumb-paper: #111b29; --thumb-ink: #edf6f7; background: #111827; }
.card-thumb-mock[data-tone='vivid'] { background: var(--style-color); }
.card-thumb-mock[data-tone='vivid'] .thumb-sheet { --thumb-paper: #f6ff56; --thumb-ink: #111; border: 2px solid #111; box-shadow: 5px 5px 0 #111; }
.card-thumb-mock[data-family='technical'] .thumb-sheet { font-family: var(--font-mono); background-image: linear-gradient(rgba(127, 233, 240, 0.06) 1px, transparent 1px), linear-gradient(90deg, rgba(127, 233, 240, 0.06) 1px, transparent 1px); background-size: 10px 10px; }
.card-thumb-mock[data-family='collectible'] .thumb-sheet { border-left: 5px solid var(--style-color); }
.card-thumb-mock[data-family='ceremonial'] .thumb-sheet { outline: 1px double var(--style-color); outline-offset: -7px; }
.card-thumb-mock[data-family='action'] .thumb-sheet { background: #f5df88; transform: rotate(-1.2deg); }
.card-thumb-mock[data-family='grid'] .thumb-sheet strong { font-family: Arial, sans-serif; font-weight: 900; text-transform: uppercase; }
.card-thumb-mock[data-family='document'] .thumb-sheet strong { font-family: 'Courier New', monospace; }
.card-thumb-mock[data-family='heritage'] .thumb-sheet { background-color: #eadbb7; }

.card-thumb-mock[data-style='gazette'] .thumb-sheet { border-top: 4px double #27231f; }
.card-thumb-mock[data-style='gazette'] .thumb-columns { grid-template-columns: repeat(3, 1fr); }
.card-thumb-mock[data-style='typewriter'] { background: #bdbdb9; }
.card-thumb-mock[data-style='typewriter'] .thumb-sheet { padding-left: 22px; background-image: linear-gradient(90deg, transparent 13px, rgba(170, 45, 45, 0.35) 14px, transparent 15px), repeating-linear-gradient(0deg, transparent 0 11px, rgba(0, 0, 0, 0.05) 11px 12px); }
.card-thumb-mock[data-style='swiss-grid'] { background: #c62f24; }
.card-thumb-mock[data-style='swiss-grid'] .thumb-sheet { transform: none; border-top: 7px solid #111; }
.card-thumb-mock[data-style='swiss-grid'] .thumb-sheet strong { max-width: 70%; font-size: 17px; line-height: 0.82; white-space: normal; }
.card-thumb-mock[data-style='vogue'] { background: #e7e3dd; }
.card-thumb-mock[data-style='vogue'] .thumb-sheet { text-align: center; transform: none; }
.card-thumb-mock[data-style='vogue'] .thumb-sheet strong { margin-top: 18px; font-size: 18px; font-weight: 400; }
.card-thumb-mock[data-style='narrative-air'] { background: #b7d3ca; }
.card-thumb-mock[data-style='narrative-air'] .thumb-sheet { --thumb-paper: #f4fbf8; border-radius: 18px 18px 0 0; transform: none; }
.card-thumb-mock[data-style='narrative-air'] .thumb-rule { width: 46%; border-radius: 3px; }
.card-thumb-mock[data-style='glassmorphism'] .thumb-sheet { background-image: radial-gradient(circle at 15% 15%, rgba(66, 209, 205, 0.25), transparent 36%); border: 1px solid rgba(117, 230, 218, 0.3); transform: none; }
.card-thumb-mock[data-style='action-board'] { background: #807565; }
.card-thumb-mock[data-style='action-board'] .thumb-sheet { box-shadow: 6px 7px 0 rgba(246, 223, 131, 0.35); }
.card-thumb-mock[data-style='executive'] { background: #172335; }
.card-thumb-mock[data-style='executive'] .thumb-sheet { border-left: 9px solid #172335; outline: 1px solid #a97728; outline-offset: -6px; }
.card-thumb-mock[data-style='magic-prophet'] .thumb-sheet { border-block: 4px double #302517; text-align: center; }
.card-thumb-mock[data-style='magic-prophet'] .thumb-ornament { display: grid; top: 42px; right: 50%; transform: translateX(50%); }
.card-thumb-mock[data-style='telegraph'] { background: #876f4f; }
.card-thumb-mock[data-style='telegraph'] .thumb-sheet { --thumb-paper: #e7d9ad; border: 1px dashed #372d22; }
.card-thumb-mock[data-style='telegraph'] .thumb-rule { background: transparent; border-top: 1px dashed #372d22; }
.card-thumb-mock[data-style='bulletin'] .thumb-sheet { --thumb-paper: #d9bb79; border: 5px solid #573723; outline: 1px solid #8d281f; outline-offset: -8px; text-align: center; }
.card-thumb-mock[data-style='blueprint'] { background: #092541; }
.card-thumb-mock[data-style='blueprint'] .thumb-sheet { --thumb-paper: #153e71; --thumb-ink: #eefaff; border: 1px solid #7fe9f0; transform: none; }
.card-thumb-mock[data-style='terminal'] .thumb-sheet { --thumb-paper: #080d0a; --thumb-ink: #9fffb5; border: 4px solid #171d19; transform: none; }
.card-thumb-mock[data-style='terminal'] .thumb-sheet strong::before { content: '> '; color: #d7ff5b; }
.card-thumb-mock[data-style='polaroid'] { background: #72797d; padding-inline: 28px; }
.card-thumb-mock[data-style='polaroid'] .thumb-sheet { padding-top: 48px; border: 7px solid #f7f5ee; border-bottom-width: 22px; background: linear-gradient(155deg, #233a4d, #617a82 55%, #c7b99b); box-shadow: 0 7px 18px rgba(15, 23, 42, 0.28); }
.card-thumb-mock[data-style='polaroid'] .thumb-kicker, .card-thumb-mock[data-style='polaroid'] .thumb-rule, .card-thumb-mock[data-style='polaroid'] .thumb-columns { display: none; }
.card-thumb-mock[data-style='polaroid'] .thumb-sheet strong { color: #fff; }
.card-thumb-mock[data-style='ticket'] { background: #6d263d; }
.card-thumb-mock[data-style='ticket'] .thumb-sheet { --thumb-paper: #f5e4df; --thumb-ink: #641c33; padding-left: 28px; border: 2px solid #9c2348; background-image: linear-gradient(90deg, transparent 15px, rgba(156, 35, 72, 0.5) 16px, transparent 17px); }
.card-thumb-mock[data-style='scroll'] .thumb-sheet { --thumb-paper: #f7f0df; border-block: 8px solid #8f252b; text-align: center; }
.card-thumb-mock[data-style='dossier'] { background: #6b4a36; padding-top: 22px; }
.card-thumb-mock[data-style='dossier'] .thumb-sheet { --thumb-paper: #d7bd91; padding-top: 20px; border-left: 8px solid #6f4d32; }
.card-thumb-mock[data-style='dossier'] .thumb-sheet::before { position: absolute; top: -1px; right: 12px; width: 58px; height: 10px; content: ''; background: #6f4d32; }
.card-thumb-mock[data-style='analytics'] .thumb-sheet { background-image: repeating-linear-gradient(90deg, transparent 0 29px, rgba(85, 182, 255, 0.07) 29px 30px); border-top: 2px solid #55b6ff; transform: none; }
.card-thumb-mock[data-style='ledger'] .thumb-sheet { --thumb-paper: #111418; --thumb-ink: #f4e8c6; border: 3px double #d4a84e; transform: none; }
.card-thumb-mock[data-style='ledger'] .thumb-columns i { border-color: #d4a84e; }
.card-thumb-mock[data-style='milestones'] { background: #d0b4c7; }
.card-thumb-mock[data-style='milestones'] .thumb-sheet { --thumb-paper: #fff8fb; border-radius: 20px 5px 0 0; transform: none; }
.card-thumb-mock[data-style='milestones'] .thumb-columns i { border: 0; border-left: 3px solid #b94080; border-radius: 8px; }
.card-thumb-mock[data-style='partner-brief'] { background: #5b4631; padding-inline: 30px; }
.card-thumb-mock[data-style='partner-brief'] .thumb-sheet { --thumb-paper: #f8f2e4; --thumb-ink: #392919; padding-inline: 19px; transform: none; }
.card-thumb-mock[data-style='partner-brief'] .thumb-sheet strong { margin-top: 16px; font-style: italic; font-weight: 400; }
.card-thumb-mock[data-style='focus-matrix'] .thumb-sheet { transform: rotate(-0.7deg); }
.card-thumb-mock[data-style='focus-matrix'] .thumb-rule { height: 5px; background: #ed3e98; }
.card-thumb-mock[data-style='chronicle'] .thumb-sheet { --thumb-paper: #f2eee5; --thumb-ink: #1e2933; padding-left: 24px; background-image: linear-gradient(90deg, transparent 14px, rgba(54, 84, 108, 0.35) 15px, transparent 16px); }
.card-thumb-mock[data-style='chronicle'] .thumb-sheet strong { font-style: italic; }
.card-thumb-mock[data-style='cyber-matrix'] .thumb-sheet { --thumb-paper: #08131d; --thumb-ink: #d9fbff; border: 1px solid #33e6d7; transform: none; box-shadow: inset 0 0 18px rgba(51, 230, 215, 0.12); }
.card-thumb-mock[data-style='hogwarts-letter'] { background: #405b52; padding-inline: 28px; }
.card-thumb-mock[data-style='hogwarts-letter'] .thumb-sheet { --thumb-paper: #eee2bd; --thumb-ink: #183f35; text-align: center; transform: none; }
.card-thumb-mock[data-style='hogwarts-letter'] .thumb-ornament { display: grid; top: 42px; right: 50%; transform: translateX(50%); }
.card-thumb-mock[data-style='classified-file'] { background: #564536; padding-top: 22px; }
.card-thumb-mock[data-style='classified-file'] .thumb-sheet { --thumb-paper: #d6b676; --thumb-ink: #201b14; border-top: 12px solid #a1121a; }
.card-thumb-mock[data-style='vinyl-record'] { background: #28211e; }
.card-thumb-mock[data-style='vinyl-record'] .thumb-sheet { --thumb-paper: #d45b32; --thumb-ink: #fff3d7; transform: none; }
.card-thumb-mock[data-style='vinyl-record'] .thumb-sheet::after { position: absolute; top: 12px; right: -19px; width: 76px; height: 76px; content: ''; border-radius: 50%; background: repeating-radial-gradient(circle, #181513 0 3px, #2d2824 4px 6px); opacity: 0.75; }
.card-thumb-mock[data-style='tarot'] .thumb-sheet { --thumb-paper: #17152f; --thumb-ink: #f2e5ad; border: 5px solid #28234b; outline: 1px solid #d2ac54; text-align: center; transform: none; }
.card-thumb-mock[data-style='tarot'] .thumb-ornament { display: grid; top: 38px; right: 50%; transform: translateX(50%); }
.card-thumb-mock[data-style='bank-note'] { background: #365c4b; }
.card-thumb-mock[data-style='bank-note'] .thumb-sheet { --thumb-paper: #dfe7d7; --thumb-ink: #174c36; border: 5px double #174c36; text-align: center; transform: none; background-image: repeating-radial-gradient(ellipse, transparent 0 7px, rgba(23, 76, 54, 0.07) 8px 9px); }
.card-thumb-mock[data-style='bank-note'] .thumb-ornament { display: grid; top: 39px; right: 50%; transform: translateX(50%); }
.card-thumb-mock[data-style='passport'] { background: #182c46; }
.card-thumb-mock[data-style='passport'] .thumb-sheet { --thumb-paper: #e9edf0; --thumb-ink: #182c46; padding-left: 24px; border-left: 9px solid #173b67; transform: none; background-image: repeating-linear-gradient(135deg, transparent 0 8px, rgba(32, 77, 131, 0.06) 8px 9px); }
.card-thumb-mock[data-style='steampunk'] .thumb-sheet { --thumb-paper: #27231f; --thumb-ink: #f0d596; border: 6px ridge #9a632d; transform: none; }
.card-thumb-mock[data-style='steampunk'] .thumb-ornament { display: grid; border: 5px double #c58434; border-radius: 50%; }
.card-thumb-mock[data-style='wax-sealed-parchment'] { background: #6a293c; padding-inline: 28px; }
.card-thumb-mock[data-style='wax-sealed-parchment'] .thumb-sheet { --thumb-paper: #ede0b9; --thumb-ink: #4b2430; border: 5px double #a46e1f; text-align: center; transform: none; }
.card-thumb-mock[data-style='wax-sealed-parchment'] .thumb-ornament { display: grid; top: 40px; right: 50%; transform: translateX(50%); }

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

.apply-hint {
  font-size: 11px;
  font-weight: 600;
  color: var(--c-text-secondary);
}

.mt-8 { margin-top: 24px; }
</style>
