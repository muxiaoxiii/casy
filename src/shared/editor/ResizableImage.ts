import Image, { type ImageOptions } from "@tiptap/extension-image";
import { VueNodeViewRenderer } from "@tiptap/vue-3";
import ImageView from "./ImageView.vue";

/** 外置配图解析器：把 `assets/<sha>.png` 映射为可显示的 URL，仅用于展示。 */
export type AssetResolver = (assetId: string) => Promise<string>;

declare module "@tiptap/extension-image" {
  interface ImageOptions {
    resolveAsset?: AssetResolver;
  }
}

export const ResizableImage = Image.extend({
  addOptions() {
    return { ...this.parent?.(), resolveAsset: undefined } as ImageOptions;
  },
  addAttributes() {
    return {
      ...this.parent?.(),
      width: {
        default: null,
        parseHTML: (el) => Number(el.getAttribute("width")) || null,
      },
      height: {
        default: null,
        parseHTML: (el) => Number(el.getAttribute("height")) || null,
      },
    };
  },
  addNodeView() {
    return VueNodeViewRenderer(ImageView);
  },
}).configure({ allowBase64: true, inline: false });
