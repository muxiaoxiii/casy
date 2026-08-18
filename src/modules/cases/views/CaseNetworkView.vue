<script setup>
import { ref, onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage } from 'element-plus'

const router = useRouter()
const loading = ref(false)
const cases = ref([])
const relations = ref([])
const selectedCaseId = ref(null)

// 关系类型映射
const relationTypeMap = {
  same_patent: { label: '同专利', color: '#409eff', icon: '📋' },
  same_party: { label: '同客户', color: '#67c23a', icon: '👥' },
  appeal_of: { label: '审级关联', color: '#e6a23c', icon: '⚖️' },
  cross_reference: { label: '交叉引用', color: '#909399', icon: '🔗' },
}

// 加载所有案件
async function loadCases() {
  loading.value = true
  const result = await tauriCallSafe('list_cases', { filter: {} })
  if (result.ok) {
    cases.value = result.data
  }
  loading.value = false
}

// 加载所有关系
async function loadAllRelations() {
  const allRelations = []
  for (const c of cases.value) {
    const result = await tauriCallSafe('get_relations', { caseId: c.id })
    if (result.ok) {
      for (const rel of result.data) {
        // 避免重复（双向关系只记录一次）
        const key = [rel.caseId, c.id].sort().join('-')
        if (!allRelations.find(r => r._key === key)) {
          allRelations.push({
            ...rel,
            _key: key,
            sourceCaseId: c.id,
            targetCaseId: rel.caseId,
          })
        }
      }
    }
  }
  relations.value = allRelations
}

// 按关系类型分组的关系
const groupedRelations = computed(() => {
  const groups = {}
  for (const [type, config] of Object.entries(relationTypeMap)) {
    groups[type] = {
      ...config,
      relations: relations.value.filter(r => r.relationType === type),
    }
  }
  return groups
})

// 获取案件信息
function getCaseById(caseId) {
  return cases.value.find(c => c.id === caseId)
}

// 获取关联案件列表
const relatedCases = computed(() => {
  if (!selectedCaseId.value) return []

  const caseRelations = relations.value.filter(
    r => r.sourceCaseId === selectedCaseId.value || r.targetCaseId === selectedCaseId.value
  )

  return caseRelations.map(r => {
    const relatedId = r.sourceCaseId === selectedCaseId.value ? r.targetCaseId : r.sourceCaseId
    const relatedCase = getCaseById(relatedId)
    return {
      ...r,
      relatedCase,
    }
  }).filter(r => r.relatedCase)
})

// 跳转到案件详情
function goToCase(caseId) {
  router.push(`/cases/${caseId}`)
}

// 选择案件
function selectCase(caseId) {
  selectedCaseId.value = selectedCaseId.value === caseId ? null : caseId
}

onMounted(async () => {
  await loadCases()
  await loadAllRelations()
})
</script>

<template>
  <div class="case-network-view">
    <el-card v-loading="loading">
      <template #header>
        <div class="card-header">
          <strong>🕸️ 案件关系网络</strong>
          <el-button size="small" @click="loadCases(); loadAllRelations()">刷新</el-button>
        </div>
      </template>

      <el-empty v-if="!loading && cases.length === 0" description="暂无案件数据" />

      <template v-else>
        <!-- 关系统计 -->
        <div class="relation-stats">
          <el-row :gutter="16">
            <el-col :span="6" v-for="(group, type) in groupedRelations" :key="type">
              <el-card class="stat-card" shadow="hover" :style="{ borderLeft: `4px solid ${group.color}` }">
                <div class="stat-content">
                  <span class="stat-icon">{{ group.icon }}</span>
                  <div class="stat-info">
                    <div class="stat-label">{{ group.label }}</div>
                    <div class="stat-value">{{ group.relations.length }}</div>
                  </div>
                </div>
              </el-card>
            </el-col>
          </el-row>
        </div>

        <!-- 关系网络列表 -->
        <div class="relation-groups">
          <el-collapse v-model="activeGroups">
            <el-collapse-item
              v-for="(group, type) in groupedRelations"
              :key="type"
              :name="type"
              :title="`${group.icon} ${group.label} (${group.relations.length})`"
            >
              <el-empty v-if="group.relations.length === 0" description="暂无此类关系" :image-size="60" />

              <div v-else class="relation-list">
                <div
                  v-for="rel in group.relations"
                  :key="rel._key"
                  class="relation-item"
                >
                  <div class="relation-cases">
                    <el-tag
                      class="case-tag"
                      effect="plain"
                      @click="selectCase(rel.sourceCaseId)"
                      :type="selectedCaseId === rel.sourceCaseId ? 'primary' : ''"
                    >
                      {{ getCaseById(rel.sourceCaseId)?.caseName || '未知案件' }}
                    </el-tag>

                    <el-icon class="relation-arrow" :style="{ color: group.color }">
                      <el-icon-connection />
                    </el-icon>

                    <el-tag
                      class="case-tag"
                      effect="plain"
                      @click="selectCase(rel.targetCaseId)"
                      :type="selectedCaseId === rel.targetCaseId ? 'primary' : ''"
                    >
                      {{ getCaseById(rel.targetCaseId)?.caseName || '未知案件' }}
                    </el-tag>
                  </div>

                  <div v-if="rel.label" class="relation-label">
                    <el-tag size="small" :color="group.color" style="color: white">
                      {{ rel.label }}
                    </el-tag>
                  </div>
                </div>
              </div>
            </el-collapse-item>
          </el-collapse>
        </div>

        <!-- 选中案件的关联详情 -->
        <el-card v-if="selectedCaseId" class="selected-case-card" style="margin-top: 20px">
          <template #header>
            <div class="card-header">
              <strong>📌 选中案件关联详情</strong>
              <el-button size="small" @click="selectedCaseId = null">取消选择</el-button>
            </div>
          </template>

          <el-descriptions :column="2" border size="small">
            <el-descriptions-item label="案件名称">
              {{ getCaseById(selectedCaseId)?.caseName }}
            </el-descriptions-item>
            <el-descriptions-item label="案号">
              {{ getCaseById(selectedCaseId)?.caseNo || '-' }}
            </el-descriptions-item>
            <el-descriptions-item label="客户">
              {{ getCaseById(selectedCaseId)?.clientName }}
            </el-descriptions-item>
            <el-descriptions-item label="状态">
              <el-tag :type="getCaseById(selectedCaseId)?.caseStatus === '已完结' ? 'success' : 'warning'" size="small">
                {{ getCaseById(selectedCaseId)?.caseStatus || '未知' }}
              </el-tag>
            </el-descriptions-item>
          </el-descriptions>

          <div class="related-cases-list" style="margin-top: 16px">
            <h4>关联案件 ({{ relatedCases.length }})</h4>
            <el-table :data="relatedCases" size="small" stripe>
              <el-table-column label="案件名称" min-width="200">
                <template #default="{ row }">
                  <el-link type="primary" @click="goToCase(row.relatedCase.id)">
                    {{ row.relatedCase.caseName }}
                  </el-link>
                </template>
              </el-table-column>
              <el-table-column label="案号" prop="relatedCase.caseNo" width="180" />
              <el-table-column label="关系类型" width="120">
                <template #default="{ row }">
                  <el-tag size="small" :color="relationTypeMap[row.relationType]?.color" style="color: white">
                    {{ relationTypeMap[row.relationType]?.label || row.relationType }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="操作" width="100">
                <template #default="{ row }">
                  <el-button size="small" type="primary" link @click="goToCase(row.relatedCase.id)">
                    查看详情
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </el-card>
      </template>
    </el-card>
  </div>
</template>

<style scoped>
.case-network-view {
  max-width: 1200px;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.relation-stats {
  margin-bottom: 24px;
}

.stat-card {
  cursor: pointer;
  transition: all 0.3s;
}

.stat-card:hover {
  transform: translateY(-2px);
}

.stat-content {
  display: flex;
  align-items: center;
  gap: 12px;
}

.stat-icon {
  font-size: 24px;
}

.stat-info {
  flex: 1;
}

.stat-label {
  font-size: 12px;
  color: #909399;
}

.stat-value {
  font-size: 20px;
  font-weight: bold;
  color: #303133;
}

.relation-groups {
  margin-top: 16px;
}

.relation-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.relation-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px;
  background: #f5f7fa;
  border-radius: 8px;
  transition: all 0.3s;
}

.relation-item:hover {
  background: #ecf5ff;
}

.relation-cases {
  display: flex;
  align-items: center;
  gap: 12px;
}

.case-tag {
  cursor: pointer;
  transition: all 0.3s;
}

.case-tag:hover {
  transform: scale(1.05);
}

.relation-arrow {
  font-size: 18px;
}

.relation-label {
  margin-left: 12px;
}

.selected-case-card {
  border-top: 3px solid #409eff;
}

.related-cases-list h4 {
  margin: 0 0 12px 0;
  color: #303133;
}
</style>
