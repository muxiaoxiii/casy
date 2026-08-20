import { Node, mergeAttributes } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import { ref, onMounted, watch } from 'vue'
import { casyContext } from '../../../core/plugin/context'

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
  setup(props) {
    const blockData = ref(null)
    const loading = ref(true)
    const error = ref(null)

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

    return { blockData, loading, error }
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
          <span class="block-ref-icon">📋</span>
          <span class="block-ref-title">{{ blockData.item?.title || '未知知识' }}</span>
          <span v-if="blockData.block?.blockType" class="block-ref-type">{{ blockData.block.blockType }}</span>
        </div>
        <div class="block-ref-body" v-html="blockData.block?.content || blockData.item?.content || ''" />
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
    return ['span', mergeAttributes(HTMLAttributes, { 'data-block-reference': '' }), '📋 引用']
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
