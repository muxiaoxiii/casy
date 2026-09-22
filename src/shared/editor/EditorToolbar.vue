<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import type { Editor } from "@tiptap/core";
import { ElPopover, ElInputNumber, ElCheckbox, ElButton } from "element-plus";
import QuoteSourceMenu from "../components/QuoteSourceMenu.vue";
const props = defineProps<{ editor: Editor }>();
const emit = defineEmits<{ link: []; wiki: []; image: []; task: [] }>();
const root = ref<HTMLElement>(),
  width = ref(800),
  more = ref(false),
  tableOpen = ref(false),
  rows = ref(3),
  cols = ref(3),
  header = ref(true);
let observer: ResizeObserver | undefined;
const actions = [
  { id: "bold", label: "粗体", icon: "B", run: (c: any) => c.toggleBold() },
  { id: "italic", label: "斜体", icon: "I", run: (c: any) => c.toggleItalic() },
  {
    id: "heading1",
    label: "一级标题",
    icon: "H1",
    run: (c: any) => c.toggleHeading({ level: 1 }),
  },
  {
    id: "heading2",
    label: "二级标题",
    icon: "H2",
    run: (c: any) => c.toggleHeading({ level: 2 }),
  },
  {
    id: "bulletList",
    label: "无序列表",
    icon: "•≡",
    run: (c: any) => c.toggleBulletList(),
  },
  {
    id: "orderedList",
    label: "有序列表",
    icon: "1.",
    run: (c: any) => c.toggleOrderedList(),
  },
  {
    id: "taskList",
    label: "待办清单",
    icon: "☑",
    run: (c: any) => c.toggleTaskList(),
  },
  {
    id: "table",
    label: "插入表格",
    icon: "▦",
    run: () => {
      tableOpen.value = true;
    },
  },
  { id: "image", label: "插入图片", icon: "▧", run: () => emit("image") },
  { id: "link", label: "插入链接", icon: "↗", run: () => emit("link") },
  {
    id: "underline",
    label: "下划线",
    icon: "U",
    run: (c: any) => c.toggleUnderline(),
  },
  {
    id: "strike",
    label: "删除线",
    icon: "S̶",
    run: (c: any) => c.toggleStrike(),
  },
  {
    id: "highlight",
    label: "高亮",
    icon: "标",
    run: (c: any) => c.toggleHighlight(),
  },
  {
    id: "heading3",
    label: "三级标题",
    icon: "H3",
    run: (c: any) => c.toggleHeading({ level: 3 }),
  },
  {
    id: "heading4",
    label: "四级标题",
    icon: "H4",
    run: (c: any) => c.toggleHeading({ level: 4 }),
  },
  {
    id: "left",
    label: "左对齐",
    icon: "左",
    run: (c: any) => c.setTextAlign("left"),
  },
  {
    id: "center",
    label: "居中",
    icon: "中",
    run: (c: any) => c.setTextAlign("center"),
  },
  {
    id: "right",
    label: "右对齐",
    icon: "右",
    run: (c: any) => c.setTextAlign("right"),
  },
  {
    id: "code",
    label: "行内代码",
    icon: "<>",
    run: (c: any) => c.toggleCode(),
  },
  {
    id: "codeBlock",
    label: "代码块",
    icon: "{}",
    run: (c: any) => c.toggleCodeBlock(),
  },
  {
    id: "horizontalRule",
    label: "分隔线",
    icon: "—",
    run: (c: any) => c.setHorizontalRule(),
  },
  { id: "wiki", label: "知识双向链接", icon: "[[ ]]", run: () => emit("wiki") },
  { id: "task", label: "关联任务", icon: "务", run: () => emit("task") },
  { id: "mathInline", label: "行内公式", icon: "∑", run: (c:any) => c.insertContent({type:'mathInline',attrs:{source:'x_i'}}) },
  { id: "mathBlock", label: "独立公式", icon: "ƒ", run: (c:any) => c.insertContent({type:'mathBlock',attrs:{source:'x_i + y_i'}}) },
  { id: "mermaid", label: "流程图", icon: "图", run: (c:any) => c.insertContent({type:'codeBlock',attrs:{language:'mermaid'},content:[{type:'text',text:'graph LR\n  A[立案] --> B[开庭]'}]}) },
  { id: "footnote", label: "脚注", icon: "注", run: (c:any) => {
    const label=`note-${Date.now().toString(36)}`;
    return c.insertContent({type:'footnoteReference',attrs:{label}}).command(({tr,state}:any) => {
      tr.insert(tr.doc.content.size,state.schema.nodes.footnoteDefinition.create({label,source:'填写脚注正文'})); return true;
    });
  } },
];
const count = computed(() =>
  Math.max(0, Math.min(actions.length, Math.floor((width.value - 190) / 34))),
);
const visible = computed(() => actions.slice(0, count.value)),
  overflow = computed(() => actions.slice(count.value));
function run(action: (typeof actions)[number]) {
  more.value = false;
  const result = action.run(props.editor.chain().focus());
  result?.run?.();
}
function active(id: string) {
  return id.startsWith("heading")
    ? props.editor.isActive("heading", { level: Number(id.slice(-1)) })
    : props.editor.isActive(id);
}
function insertTable() {
  props.editor
    .chain()
    .focus()
    .insertTable({
      rows: rows.value,
      cols: cols.value,
      withHeaderRow: header.value,
    })
    .run();
  tableOpen.value = false;
}
onMounted(() => {
  if (root.value) {
    width.value = root.value.clientWidth;
    if (typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver(([entry]) => {
        width.value = entry?.contentRect.width || 800;
      });
      observer.observe(root.value);
    }
  }
});
onBeforeUnmount(() => observer?.disconnect());
defineExpose({
  openTable: () => {
    tableOpen.value = true;
  },
});
</script>
<template>
  <div
    ref="root"
    class="document-toolbar"
    role="toolbar"
    aria-label="正文编辑工具"
  >
    <button
      aria-label="撤销"
      title="撤销"
      :disabled="!editor.can().undo()"
      @mousedown.prevent
      @click="editor.chain().focus().undo().run()"
    >
      ↶
    </button>
    <button
      aria-label="重做"
      title="重做"
      :disabled="!editor.can().redo()"
      @mousedown.prevent
      @click="editor.chain().focus().redo().run()"
    >
      ↷
    </button>
    <span class="separator" />
    <button
      v-for="action in visible"
      :key="action.id"
      :class="{ active: active(action.id) }"
      :aria-label="action.label"
      :title="action.label"
      @mousedown.prevent
      @click="run(action)"
    >
      {{ action.icon }}
    </button>
    <ElPopover
      v-model:visible="more"
      trigger="click"
      placement="bottom-end"
      :width="290"
    >
      <template #reference
        ><button v-if="overflow.length" class="more" aria-label="更多编辑工具">
          更多 ···
        </button></template
      >
      <div class="overflow-tools" role="menu" aria-label="更多编辑工具">
        <button
          v-for="action in overflow"
          :key="action.id"
          role="menuitem"
          @mousedown.prevent
          @click="run(action)"
        >
          <span>{{ action.icon }}</span
          >{{ action.label }}
        </button>
      </div>
    </ElPopover>
    <QuoteSourceMenu :editor="editor" />
    <ElPopover
      v-model:visible="tableOpen"
      placement="right-start"
      :width="270"
      :persistent="false"
    >
      <template #reference><span class="table-anchor" /></template>
      <div class="table-picker">
        <strong>插入表格</strong><span>{{ rows }} 行 × {{ cols }} 列</span>
        <div class="table-grid">
          <button
            v-for="n in 64"
            :key="n"
            :class="{
              chosen: Math.ceil(n / 8) <= rows && ((n - 1) % 8) + 1 <= cols,
            }"
            :aria-label="`${Math.ceil(n / 8)}行${((n - 1) % 8) + 1}列表格`"
            @mouseenter="
              rows = Math.ceil(n / 8);
              cols = ((n - 1) % 8) + 1;
            "
            @click="
              rows = Math.ceil(n / 8);
              cols = ((n - 1) % 8) + 1;
              insertTable();
            "
          />
        </div>
        <label
          >行数
          <ElInputNumber
            v-model="rows"
            :min="1"
            :max="100"
            size="small"
            aria-label="表格行数" /></label
        ><label
          >列数
          <ElInputNumber
            v-model="cols"
            :min="1"
            :max="20"
            size="small"
            aria-label="表格列数" /></label
        ><ElCheckbox v-model="header">第一行为表头</ElCheckbox
        ><ElButton type="primary" @click="insertTable">插入表格</ElButton>
      </div>
    </ElPopover>
  </div>
</template>
<style scoped>
.document-toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  width: 100%;
  box-sizing: border-box;
  padding: 8px 10px;
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-card);
  flex-shrink: 0;
}
.document-toolbar > button {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
}
.document-toolbar button,
.overflow-tools button {
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--c-text);
  cursor: pointer;
  font-size: 12px;
}
.document-toolbar button:hover,
.overflow-tools button:hover {
  background: var(--c-bg-hover);
}
.document-toolbar button.active {
  color: var(--c-primary);
  background: var(--c-bg-selected);
}
.document-toolbar button:disabled {
  opacity: 0.35;
  cursor: default;
}
.document-toolbar .more {
  width: auto;
  padding: 0 8px;
  white-space: nowrap;
  margin-left: auto;
}
.separator {
  width: 1px;
  height: 18px;
  background: var(--c-border);
  margin: 0 5px;
}
.overflow-tools {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 3px;
}
.overflow-tools button {
  text-align: left;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px;
}
.overflow-tools button span {
  width: 25px;
  color: var(--c-text-secondary);
}
.table-picker {
  display: grid;
  gap: 10px;
}
.table-picker > span {
  font-size: 12px;
  color: var(--c-text-secondary);
}
.table-grid {
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  gap: 4px;
}
.table-grid button {
  aspect-ratio: 1;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  cursor: pointer;
}
.table-grid .chosen {
  border-color: var(--c-primary);
  background: var(--el-color-primary-light-9);
}
.table-picker label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
}
.table-anchor {
  width: 0;
}
</style>
