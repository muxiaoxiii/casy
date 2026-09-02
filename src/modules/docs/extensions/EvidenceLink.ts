import { Node, mergeAttributes } from '@tiptap/core'

/**
 * 证据链接目标类型（与后端 links 表类型域一致）
 * doc 不在可选目标内（文书之间暂用 WikiLink/BlockReference），但渲染兜底支持
 */
export type EvidenceTargetType = 'file' | 'knowledge' | 'task' | 'case' | 'doc'

export interface EvidenceLinkAttrs {
  /** 后端 links.id（删除/对账用；HTML 序列化进 data-link-id） */
  linkId?: string | null
  targetType: EvidenceTargetType
  targetId: string
  /** 页码等定位信息，如 "page:12" */
  anchor?: string | null
  /** 展示文本（默认取目标名） */
  label?: string | null
  /**
   * file 目标的所属案件 id（跳转 /files/:caseId 需要）。
   * 仅存于文档 HTML（data-case-id），不入 links 表。
   */
  caseId?: string | null
}

/**
 * 命令类型注册（TipTap 约定）：让 editor.commands.insertEvidenceLink 获得完整类型
 */
declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    evidenceLink: {
      insertEvidenceLink: (attrs: EvidenceLinkAttrs) => ReturnType
    }
  }
}

/** 各目标类型的徽标字（纯样式徽标，禁 emoji） */
const TYPE_BADGES: Record<string, string> = {
  file: '卷',
  knowledge: '知',
  task: '务',
  case: '案',
  doc: '文',
}

export function evidenceBadge(targetType: string | null | undefined): string {
  return TYPE_BADGES[targetType || ''] || '链'
}

/**
 * 证据链接扩展（W4 · Hookmark/Obsidian 式跨模块双链）
 *
 * 文书正文中引用卷宗文件（可带页码）/知识条目/任务/案件的 inline 原子节点。
 * 铁律：节点一律携带内部 ID（targetId + linkId），OS 层重命名文件不断链。
 *
 * HTML 输出结构（导出 docx 兼容处理时按此解析）：
 * ```html
 * <span data-evidence-link class="evidence-link evidence-link--file"
 *       data-badge="卷"
 *       data-link-id="lnk_xxx"
 *       data-target-type="file" data-target-id="file_xxx"
 *       data-case-id="case_xxx"
 *       data-anchor="page:12" data-label="合同扫描件">合同扫描件</span>
 * ```
 * 文本内容 = label（导出 docx 时退化为纯文本锚， data-* 属性承载全部链接信息）。
 * 点击交互由编辑器容器统一代理（LegalEditor 散发 CustomEvent 'casy:evidence-link-activate'）。
 */
export const EvidenceLink = Node.create({
  name: 'evidenceLink',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      linkId: {
        default: null,
        parseHTML: element => element.getAttribute('data-link-id'),
        renderHTML: attributes => (attributes.linkId ? { 'data-link-id': attributes.linkId } : {}),
      },
      targetType: {
        default: null,
        parseHTML: element => element.getAttribute('data-target-type'),
        renderHTML: attributes => ({
          'data-target-type': attributes.targetType,
        }),
      },
      targetId: {
        default: null,
        parseHTML: element => element.getAttribute('data-target-id'),
        renderHTML: attributes => ({
          'data-target-id': attributes.targetId,
        }),
      },
      anchor: {
        default: null,
        parseHTML: element => element.getAttribute('data-anchor'),
        renderHTML: attributes => (attributes.anchor ? { 'data-anchor': attributes.anchor } : {}),
      },
      label: {
        default: null,
        parseHTML: element => element.getAttribute('data-label'),
        renderHTML: attributes => (attributes.label ? { 'data-label': attributes.label } : {}),
      },
      caseId: {
        default: null,
        parseHTML: element => element.getAttribute('data-case-id'),
        renderHTML: attributes => (attributes.caseId ? { 'data-case-id': attributes.caseId } : {}),
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-evidence-link]',
      },
    ]
  },

  renderHTML({ node, HTMLAttributes }) {
    const targetType = node.attrs.targetType || 'unknown'
    const text = node.attrs.label || node.attrs.targetId || '证据链接'
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-evidence-link': '',
        class: `evidence-link evidence-link--${targetType}`,
        'data-badge': evidenceBadge(targetType),
      }),
      String(text),
    ]
  },

  addCommands() {
    return {
      insertEvidenceLink: attrs => ({ commands }) => {
        return commands.insertContent({
          type: this.name,
          attrs,
        })
      },
    }
  },
})

export default EvidenceLink
