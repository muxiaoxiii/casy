<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import type { AssetResolver } from "./ResizableImage";
const props = defineProps(nodeViewProps);
const width = computed(() => Number(props.node.attrs.width) || 480);

// 外置配图（`assets/<sha>.png`）按需解析为可显示 URL；node.attrs.src 保持原引用，
// 序列化回 Markdown 时不会被 blob/data 地址污染。
const ASSET_REF = /^assets\/([a-f0-9]{64}\.png)$/;
const displaySrc = ref<string | null>(null);
const failed = ref(false);
let generation = 0;
let revoke: string | null = null;

function resolveAsset(): AssetResolver | undefined {
  return (props.extension as { options?: { resolveAsset?: AssetResolver } })?.options
    ?.resolveAsset;
}

watch(
  () => [props.node.attrs.src, resolveAsset()],
  async () => {
    const current = ++generation;
    const src = String(props.node.attrs.src || "");
    const match = ASSET_REF.exec(src);
    const resolver = resolveAsset();
    if (!match || !resolver) {
      displaySrc.value = src || null;
      failed.value = false;
      return;
    }
    displaySrc.value = null;
    failed.value = false;
    try {
      const url = await resolver(match[1]);
      if (current !== generation) return;
      displaySrc.value = url;
      revoke = url.startsWith("blob:") ? url : null;
    } catch {
      if (current !== generation) return;
      failed.value = true;
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  generation++;
  if (revoke) URL.revokeObjectURL(revoke);
});
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
      v-if="displaySrc"
      :src="displaySrc"
      :alt="node.attrs.alt || ''"
      :title="node.attrs.title || ''"
      draggable="false"
      @click="select"
    />
    <div v-else class="image-pending" @click="select">
      {{ failed ? "图片加载失败" : "图片加载中…" }}
    </div>
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
.image-pending {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 80px;
  padding: 16px;
  color: var(--c-text-secondary);
  background: var(--c-bg-page);
  border: 1px dashed var(--c-border);
  border-radius: 2px;
  font: 12px var(--font-family);
  line-height: 1.5;
  cursor: pointer;
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
