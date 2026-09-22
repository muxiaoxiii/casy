<script setup lang="ts">
import { computed } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
const props = defineProps(nodeViewProps);
const width = computed(() => Number(props.node.attrs.width) || 480);
function select() {
  const pos = props.getPos();
  if (typeof pos === "number") props.editor.commands.setNodeSelection(pos);
}
function resize(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  if (value >= 40 && value <= 1600)
    props.updateAttributes({ width: value, height: null });
}
function paragraph(after: boolean) {
  const pos = props.getPos();
  if (typeof pos !== "number") return;
  const target = pos + (after ? props.node.nodeSize : 0);
  props.editor
    .chain()
    .insertContentAt(target, { type: "paragraph" })
    .setTextSelection(target + 1)
    .focus()
    .run();
}
</script>
<template>
  <NodeViewWrapper
    class="document-image"
    :class="{ selected }"
    :style="{
      width: node.attrs.width ? `${width}px` : 'fit-content',
      maxWidth: '100%',
    }"
  >
    <img
      :src="node.attrs.src"
      :alt="node.attrs.alt || ''"
      :title="node.attrs.title || ''"
      draggable="false"
      @click="select"
    />
    <div
      v-if="selected"
      class="image-controls"
      contenteditable="false"
      @mousedown.stop
    >
      <label
        >宽度
        <input
          type="number"
          :value="width"
          min="40"
          max="1600"
          step="10"
          aria-label="图片宽度（像素）"
          @change="resize"
        />
        px</label
      >
      <input
        type="range"
        :value="width"
        min="80"
        max="1000"
        step="10"
        aria-label="调整图片大小"
        @input="resize"
      />
      <button type="button" @click="paragraph(false)">图前输入</button
      ><button type="button" @click="paragraph(true)">图后输入</button>
    </div>
  </NodeViewWrapper>
</template>
<style scoped>
.document-image {
  position: relative;
  box-sizing: border-box;
  margin: 16px 0;
  border: 2px solid transparent;
  border-radius: 4px;
  line-height: 0;
  cursor: pointer;
}
.document-image:hover {
  border-color: var(--c-border);
}
.document-image.selected {
  border-color: var(--c-primary);
}
.document-image img {
  display: block;
  width: 100%;
  height: auto;
  max-width: 100%;
  border-radius: 2px;
}
.image-controls {
  line-height: 1.5;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 9px;
  background: var(--c-bg-card);
  border-top: 1px solid var(--c-border);
  font: 12px var(--font-family);
  min-width: 180px;
  max-width: 100%;
}
.image-controls label {
  white-space: nowrap;
}
.image-controls input[type="number"] {
  width: 64px;
}
.image-controls input[type="range"] {
  width: 100px;
}
.image-controls button,
.image-controls input[type="number"] {
  color: var(--c-text);
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: 4px;
  padding: 4px;
}
.image-controls button {
  cursor: pointer;
}
</style>
