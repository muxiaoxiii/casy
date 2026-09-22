<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { Cpu, Lock, Document, Connection, Refresh } from '../../../shared/icons'

const appVersion = ref('4.0.0-pro')
const dbStatus = ref('SQLite (Local Encrypted)')
const rustCoreStatus = ref('Running (Tauri v2 Core)')
const buildTime = ref('2026.08')

const dbInfo = ref<Record<string, unknown>>({})
const loading = ref(false)

async function refreshInfo() {
  loading.value = true
  try {
    const res = await casyContext.backup.list()
    if (res.ok) {
      dbInfo.value = {
        backupCount: res.data?.length || 0,
      }
    }
  } catch (e) {
    console.warn('Failed to fetch about meta', e)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  refreshInfo()
})
</script>

<template>
  <div class="about-settings-wrap">
    <!-- 头部品牌卡片 -->
    <div class="about-hero-card">
      <div class="brand-logo-badge">
        <div class="brand-grid-icon">
          <span class="grid-cell" />
          <span class="grid-cell" />
          <span class="grid-cell" />
          <span class="grid-cell" />
        </div>
      </div>
      <div class="brand-text-block">
        <h2 class="app-name">Casy Workspace</h2>
        <p class="app-tagline">专为专业律师打造的本地优先 (Local-First) 智能诉讼管理与文书工作台</p>
        <div class="version-badges">
          <span class="badge-version">v{{ appVersion }}</span>
          <span class="badge-status">
            <span class="status-dot"></span>
            本地环境安全就绪
          </span>
        </div>
      </div>
    </div>

    <!-- 系统与底层架构卡片 -->
    <div class="about-section">
      <h3 class="section-heading">系统架构与存储引擎</h3>
      <div class="arch-grid">
        <div class="arch-card">
          <div class="arch-icon"><Lock /></div>
          <div class="arch-content">
            <span class="arch-title">数据底座</span>
            <strong class="arch-val">{{ dbStatus }}</strong>
            <p class="arch-desc">所有案件、证据、任务与文书均存放于本地数据库，绝不擅自上传云端。</p>
          </div>
        </div>

        <div class="arch-card">
          <div class="arch-icon"><Cpu /></div>
          <div class="arch-content">
            <span class="arch-title">核心驱动</span>
            <strong class="arch-val">{{ rustCoreStatus }}</strong>
            <p class="arch-desc">基于 Rust 高性能并发引擎，毫秒级检索全文与期限计算，丝滑稳定。</p>
          </div>
        </div>

        <div class="arch-card">
          <div class="arch-icon"><Connection /></div>
          <div class="arch-content">
            <span class="arch-title">开放连接</span>
            <strong class="arch-val">WebDAV / CalDAV / SMTP</strong>
            <p class="arch-desc">支持自由同步至个人私有云盘及本地系统日历，拥有完全的数据主权。</p>
          </div>
        </div>

        <div class="arch-card">
          <div class="arch-icon"><Document /></div>
          <div class="arch-content">
            <span class="arch-title">文书与知识引擎</span>
            <strong class="arch-val">Notion-Style Block Flow</strong>
            <p class="arch-desc">模块化富文本块编辑 + 专属智库双链穿插，支持 Word 文档一键导出。</p>
          </div>
        </div>
      </div>
    </div>

    <!-- 隐私与免责声明 -->
    <div class="about-section">
      <h3 class="section-heading">免责声明与安全准则</h3>
      <div class="disclaimer-box">
        <p>1. <strong>隐私保证</strong>：Casy 为独立客户端软件。除非您主动在设置中配置第三方大模型 API 或 WebDAV 账号，否则本软件不会向任何外部服务器发送您的案卷数据。</p>
        <p>2. <strong>AI 辅助提示</strong>：AI 生成的质证提纲、法律备忘录与文书片段仅供参考，执业律师应对所有对外签署或提交法院的文件进行严格的专业复核。</p>
        <p>3. <strong>诉讼时效注意</strong>：系统内的期限倒计时基于法定工作日及您设定的规则计算，重要绝限请结合法院正式传票或裁定书核对。</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.about-settings-wrap {
  display: flex;
  flex-direction: column;
  gap: 24px;
  max-width: 820px;
}

.about-hero-card {
  display: flex;
  align-items: center;
  gap: 20px;
  padding: 24px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
}

.brand-logo-badge {
  width: 64px;
  height: 64px;
  border-radius: 16px;
  background: var(--c-primary);
  display: grid;
  place-content: center;
  flex-shrink: 0;
  box-shadow: 0 8px 24px rgba(36, 68, 129, 0.25);
}

.brand-grid-icon {
  display: grid;
  grid-template-columns: repeat(2, 12px);
  grid-template-rows: repeat(2, 12px);
  gap: 6px;
}

.grid-cell {
  width: 12px;
  height: 12px;
  border: 2.5px solid var(--c-primary-contrast);
  border-radius: 2px;
}

.brand-text-block {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.app-name {
  font-size: 22px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
  letter-spacing: -0.5px;
}

.app-tagline {
  font-size: 13px;
  color: var(--c-text-secondary);
  margin: 0 0 6px;
  line-height: 1.5;
}

.version-badges {
  display: flex;
  align-items: center;
  gap: 8px;
}

.badge-version {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--c-bg-subtle);
  color: var(--c-text);
  border: 1px solid var(--c-border);
}

.badge-status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  color: var(--status-success);
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--bg-success-weak);
  font-weight: 500;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--status-success);
}

.about-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.section-heading {
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.arch-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
}

.arch-card {
  display: flex;
  gap: 14px;
  padding: 16px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  box-shadow: var(--shadow-sm);
}

.arch-icon {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  display: grid;
  place-items: center;
  font-size: 18px;
  flex-shrink: 0;
}

.arch-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.arch-title {
  font-size: 11px;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.arch-val {
  font-size: 13.5px;
  color: var(--c-text-heading);
}

.arch-desc {
  font-size: 12px;
  color: var(--c-text-secondary);
  line-height: 1.45;
  margin: 4px 0 0;
}

.disclaimer-box {
  padding: 16px 20px;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 12.5px;
  color: var(--c-text-secondary);
  line-height: 1.6;
}

.disclaimer-box p {
  margin: 0;
}

.disclaimer-box strong {
  color: var(--c-text-heading);
}
</style>
