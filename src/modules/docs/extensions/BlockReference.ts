import { Node, mergeAttributes } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import { ref, computed, onMounted, watch } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { mdToSafeHtml } from '../../../shared/markdown/mdBridge'

/**
 * 命令类型注册（TipTap 约定）：让 editor.commands.insertBlockReference 获得完整类型
 */
declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    blockReference: {
      insertBlockReference: (knowledgeId: string, blockId?: string | null) => ReturnType
    }
  }
}

// 块引用节点视图组件
const BlockReferenceNodeView = {
  props: {
    node: {
      type: Object,
      required: true,
    },
    updateAttributes: {
      type: Function,
      required: true,
    },
  },
  setup(props: { node: { attrs: { knowledgeId?: string | null; blockId?: string | null } } }) {
    const blockData = ref<{ item?: Record<string, unknown>; block: Record<string, unknown> | null } | null>(null)
    const loading = ref(true)
    const error = ref<string | null>(null)

    async function loadBlock() {
      const { knowledgeId, blockId } = props.node.attrs
      if (!knowledgeId) {
        loading.value = false
        return
      }

      try {
        loading.value = true
        const result = await casyContext.knowledge.getWithBlocks(knowledgeId)
        if (result.ok && result.data?.blocks) {
          const block = blockId
            ? result.data.blocks.find(b => b.id === blockId)
            : result.data.blocks[0] // 默认引用第一个块
          blockData.value = {
            item: result.data.item,
            block: block || null
          }
        } else {
          error.value = '知识不存在'
        }
      } catch (e) {
        error.value = '加载失败'
      } finally {
        loading.value = false
      }
    }

    onMounted(loadBlock)
    watch(() => [props.node.attrs.knowledgeId, props.node.attrs.blockId], loadBlock)

    // 安全（审查 P0-1）：知识库正文是跨信任边界数据（导入 Markdown / OCR 卷宗 /
    // 飞书同步 / AI 生成），可含 `<img onerror>`。此处原本裸 v-html 执行。
    // 正文为 Markdown，与知识笔记本同源，故统一走 mdToSafeHtml（渲染 + DOM 白名单消毒）。
    const safeContent = computed(() => {
      const raw =
        (blockData.value?.block?.content as string | undefined) ||
        (blockData.value?.item?.content as string | undefined) ||
        ''
      return raw ? mdToSafeHtml(raw) : ''
    })

    return { blockData, loading, error, safeContent }
  },
  template: `
    <node-view-wrapper class="block-reference" :class="{ 'is-loading': loading, 'is-error': error }">
      <div v-if="loading" class="block-ref-loading">
        <el-icon class="is-loading"><Loading /></el-icon>
        <span>加载中...</span>
      </div>
      <div v-else-if="error" class="block-ref-error">
        <el-icon><WarningFilled /></el-icon>
        <span>{{ error }}</span>
      </div>
      <div v-else-if="blockData" class="block-ref-content">
        <div class="block-ref-header">
          <el-icon class="block-ref-icon"><Document /></el-icon>
          <span class="block-ref-title">{{ blockData.item?.title || '未知知识' }}</span>
          <span v-if="blockData.block?.blockType" class="block-ref-type">{{ blockData.block.blockType }}</span>
        </div>
        <div class="block-ref-body" v-html="safeContent" />
      </div>
    </node-view-wrapper>
  `,
}

/**
 * TipTap 块引用扩展
 * 
 * 语法：@knowledge:KNOWLEDGE_ID 或 @knowledge:KNOWLEDGE_ID:BLOCK_ID
 * 
 * 设计哲学 §9.3：块级引用——文书中引用知识库块，内容自动同步更新
 */
export const BlockReference = Node.create({
  name: 'blockReference',
  group: 'inline',
  inline: true,
  atom: true,

  addAttributes() {
    return {
      knowledgeId: {
        default: null,
        parseHTML: element => element.getAttribute('data-knowledge-id'),
        renderHTML: attributes => ({
          'data-knowledge-id': attributes.knowledgeId,
        }),
      },
      blockId: {
        default: null,
        parseHTML: element => element.getAttribute('data-block-id'),
        renderHTML: attributes => ({
          'data-block-id': attributes.blockId,
        }),
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-block-reference]',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { 'data-block-reference': '' }), '引用']
  },

  addNodeView() {
    return VueNodeViewRenderer(BlockReferenceNodeView)
  },

  addCommands() {
    return {
      insertBlockReference: (knowledgeId, blockId) => ({ commands }) => {
        return commands.insertContent({
          type: this.name,
          attrs: { knowledgeId, blockId },
        })
      },
    }
  },
})

export default BlockReference
