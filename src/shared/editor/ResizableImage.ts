import Image from "@tiptap/extension-image";
import { VueNodeViewRenderer } from "@tiptap/vue-3";
import ImageView from "./ImageView.vue";
export const ResizableImage = Image.extend({
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
