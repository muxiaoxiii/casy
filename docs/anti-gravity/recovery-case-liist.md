<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { ref, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useCasesStore } from '../../../stores/cases'
import { useTasksStore } from '../../../stores/tasks'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Search,
  Filter,
  Plus,
  ArrowRight,
  FolderOpened,
  Folder,
  Document,
  Clock,
  Briefcase,
  Check,
  More,
  Delete,
  View,
  TopRight,
  Share,
  Files,
  Timer,
  Calendar,
  Rank,
  TrendCharts,
  User,
  Location,
  Edit,
  OfficeBuilding,
  ScaleToOriginal,
  DocumentCopy,
  CopyDocument,
  FolderAdd,
  ArrowRightBold,
  ArrowLeft,
  Notebook,
  List,
  FullScreen,
  Position,
  Warning,
  Tickets,
  CollectionTag,
  MagicStick,
  Opportunity,
  ChatDotRound,
  Bell,
  Reading,
  Close,
  Upload,
} from '@element-plus/icons-vue'
import CaseFilterBar from '../components/CaseFilterBar.vue'
import CaseGroupPanel from '../components/CaseGroupPanel.vue'
import CaseWizard from '../components/CaseWizard.vue'
import CaseImportDialog from '../components/CaseImportDialog.vue'
import {
  CIVIL_STATUS_LABELS,
  INVALIDATION_STATUS_LABELS,
  ADMIN_STATUS_LABELS,
} from '../../../types'
import StateFeedback from '../../../shared/components/StateFeedback.vue'
const router = useRouter()
