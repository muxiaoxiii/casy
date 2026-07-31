<script setup>
import { ref, onMounted, computed } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'

const loading = ref(false)
const knowledgeList = ref([])
const activeStyle = ref('complaint')

// 5 种文书风格定义
const styles = [
  {
    key: 'complaint',
    label: '起诉状',
    icon: '📜',
    color: '#409eff',
    features: [
      '诉讼请求明确、具体、可执行',
      '事实陈述简洁，按时间线组织',
      '法条引用精准，与诉求一一对应',
      '证据清单与事实陈述相互印证',
    ],
    scenarios: ['民事起诉', '行政起诉', '知识产权侵权诉讼'],
  },
  {
    key: 'defense_brief',
    label: '代理词',
    icon: '⚖️',
    color: '#67c23a',
    features: [
      '论证层次清晰，分点论述',
      '证据引用规范，标注证据编号',
      '反驳有力，针对对方主张逐项回应',
      '法律适用准确，结合案情解释法条',
    ],
    scenarios: ['庭审代理意见', '书面辩论意见', '庭后代理词'],
  },
  {
    key: 'legal_opinion',
    label: '法律意见',
    icon: '📋',
    color: '#e6a23c',
    features: [
      '结论先行，开门见山',
      '风险分析全面，按可能性分级',
      '建议具体可操作',
      '引用法律依据充分',
    ],
    scenarios: ['客户法律咨询', '内部案件分析', '交易法律尽调'],
  },
  {
    key: 'lawyer_letter',
    label: '律师函',
    icon: '✉️',
    color: '#f56c6c',
    features: [
      '立场明确，不回避核心问题',
      '期限警告清晰，给出合理期限',
      '法律依据充分，引用具体法条',
      '措辞专业但保持礼貌克制',
    ],
    scenarios: ['侵权警告函', '催告函', '停止侵权通知', '律师声明'],
  },
  {
    key: 'reply_brief',
    label: '答辩状',
    icon: '🛡️',
    color: '#909399',
    features: [
      '逐项反驳对方诉讼请求',
      '证据清单完整，标注三性意见',
      '时效抗辩等程序性抗辩优先',
      '事实认定争议逐点澄清',
    ],
    scenarios: ['民事答辩', '行政答辩', '反诉答辩'],
  },
]

const currentStyle = computed(() => styles.find(s => s.key === activeStyle.value))

const filteredKnowledge = computed(() => {
  return knowledgeList.value.filter(item => item.category === activeStyle.value)
})

async function loadKnowledge() {
  loading.value = true
  const result = await tauriCallSafe('list_knowledge', { filter: {} })
  if (result.ok) {
    knowledgeList.value = result.data
  }
  loading.value = false
}

function selectStyle(key) {
  activeStyle.value = key
}

function truncate(text, max) {
  if (!text) return ''
  return text.length > max ? text.substring(0, max) + '...' : text
}

function formatDate(dateStr) {
  if (!dateStr) return '-'
  return dateStr.replace('T', ' ').substring(0, 10)
}

onMounted(() => {
  loadKnowledge()
})
</script>

<template>
  <div class="style-guide-page">
    <div class="style-guide-header">
      <h2>📝 文书风格指南</h2>
      <p class="subtitle">每种文书风格的特征描述与知识条目库</p>
    </div>

    <div class="style-guide-body">
      <!-- 左侧风格列表 -->
      <div class="style-nav">
        <div
          v-for="style in styles"
          :key="style.key"
          class="style-nav-item"
          :class="{ active: activeStyle === style.key }"
          @click="selectStyle(style.key)"
        >
          <span class="style-icon">{{ style.icon }}</span>
          <span class="style-label">{{ style.label }}</span>
          <el-badge
            :value="knowledgeList.filter(k => k.category === style.key).length"
            :max="99"
            class="style-count"
          />
        </div>
      </div>

      <!-- 右侧详情 -->
      <div class="style-detail" v-if="currentStyle">
        <el-card shadow="never">
          <template #header>
            <div class="detail-title" :style="{ borderLeftColor: currentStyle.color }">
              <span class="detail-icon">{{ currentStyle.icon }}</span>
              <h3>{{ currentStyle.label }}</h3>
            </div>
          </template>

          <!-- 特征描述 -->
          <div class="section">
            <h4>📋 风格特征</h4>
            <ul class="feature-list">
              <li v-for="(feat, idx) in currentStyle.features" :key="idx">
                {{ feat }}
              </li>
            </ul>
          </div>

          <!-- 适用场景 -->
          <div class="section">
            <h4>🎯 适用场景</h4>
            <div class="scenario-tags">
              <el-tag
                v-for="scene in currentStyle.scenarios"
                :key="scene"
                :color="currentStyle.color + '20'"
                :style="{ color: currentStyle.color, borderColor: currentStyle.color + '40' }"
                size="default"
              >
                {{ scene }}
              </el-tag>
            </div>
          </div>

          <!-- 该风格下的知识条目 -->
          <div class="section">
            <h4>📚 知识条目（{{ filteredKnowledge.length }}）</h4>
            <div v-if="filteredKnowledge.length === 0" class="empty-hint">
              暂无{{ currentStyle.label }}风格的知识条目。<br>
              在编辑器中选中文本，右键选择"标注：{{ currentStyle.label }}风格"即可入库。
            </div>
            <div v-else class="knowledge-cards">
              <div
                v-for="item in filteredKnowledge"
                :key="item.id"
                class="knowledge-card"
              >
                <div class="card-title">{{ item.title }}</div>
                <div class="card-content">{{ truncate(item.content, 120) }}</div>
                <div class="card-meta">
                  <span v-if="item.tags" class="card-tags">
                    <el-tag v-for="tag in item.tags.split(',').slice(0, 3)" :key="tag" size="small" type="info">
                      {{ tag.trim() }}
                    </el-tag>
                  </span>
                  <span class="card-date">{{ formatDate(item.updatedAt) }}</span>
                </div>
              </div>
            </div>
          </div>
        </el-card>
      </div>
    </div>
  </div>
</template>

<style scoped>
.style-guide-page {
  padding: 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.style-guide-header {
  margin-bottom: 20px;
}

.style-guide-header h2 {
  margin: 0 0 4px;
}

.subtitle {
  margin: 0;
  color: #909399;
  font-size: 14px;
}

.style-guide-body {
  flex: 1;
  display: flex;
  gap: 20px;
  overflow: hidden;
}

/* 左侧导航 */
.style-nav {
  width: 180px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.style-nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  position: relative;
}

.style-nav-item:hover {
  background: #f5f7fa;
}

.style-nav-item.active {
  background: #ecf5ff;
  color: #409eff;
  font-weight: 600;
}

.style-icon {
  font-size: 18px;
}

.style-label {
  flex: 1;
  font-size: 14px;
}

.style-count {
  margin-left: auto;
}

/* 右侧详情 */
.style-detail {
  flex: 1;
  overflow-y: auto;
}

.detail-title {
  display: flex;
  align-items: center;
  gap: 10px;
  border-left: 3px solid #409eff;
  padding-left: 12px;
}

.detail-icon {
  font-size: 24px;
}

.detail-title h3 {
  margin: 0;
  font-size: 18px;
}

.section {
  margin-bottom: 24px;
}

.section h4 {
  margin: 0 0 12px;
  font-size: 15px;
  color: #303133;
}

.feature-list {
  margin: 0;
  padding-left: 20px;
}

.feature-list li {
  font-size: 14px;
  line-height: 2;
  color: #606266;
}

.scenario-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.empty-hint {
  padding: 24px;
  text-align: center;
  color: #909399;
  font-size: 13px;
  line-height: 1.8;
  background: #fafafa;
  border-radius: 8px;
  border: 1px dashed #e4e7ed;
}

.knowledge-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.knowledge-card {
  padding: 14px;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  transition: box-shadow 0.2s;
}

.knowledge-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.card-title {
  font-size: 14px;
  font-weight: 600;
  color: #303133;
  margin-bottom: 6px;
}

.card-content {
  font-size: 13px;
  color: #606266;
  line-height: 1.6;
  margin-bottom: 8px;
}

.card-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-tags {
  display: flex;
  gap: 4px;
}

.card-date {
  font-size: 12px;
  color: #c0c4cc;
}
</style>
