import { runSessionOperation } from '../../stores/sessionOperations'
import { tauriCallSafe } from "../../core/tauriBridge";
import { htmlToMd } from "../markdown/mdBridge";
import { documentFromContent } from "./schema";
import type { RichTextDocument } from "../../core/services/docs";
export type DocumentExportFormat = "md" | "pdf" | "docx";
export async function exportDocument(input: {
  content: string;
  contentFormat: "markdown" | "html";
  title: string;
  format: DocumentExportFormat;
  document?: RichTextDocument | null;
  layout?: import('../../core/services/docs').DocumentLayout;
}): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const name =
    input.title.replace(/[\\/:*?"<>|]/g, "_").slice(0, 120) || "未命名文档";
  const outputPath = await save({
    title: "导出文档",
    defaultPath: `${name}.${input.format}`,
    filters: [
      {
        name: { md: "Markdown", pdf: "PDF", docx: "Word" }[input.format],
        extensions: [input.format],
      },
    ],
  });
  if (!outputPath) return null;
  return runSessionOperation(`导出 ${input.title || "未命名文档"}（${input.format.toUpperCase()}）`, async () => {
  const document = (input.document ||
    documentFromContent(
      input.content,
      input.contentFormat,
    )) as RichTextDocument;
  const markdown =
    input.contentFormat === "markdown"
      ? input.content
      : htmlToMd(input.content);
  let result = await tauriCallSafe("export_editor_document", {
    document,
    markdown,
    title: input.title,
    format: input.format,
    outputPath,
    ...(input.layout ? {layout:input.layout} : {}),
  });
  if (!result.ok && input.format==='docx' && /图片|DOCX 导出尚不支持|暂不支持节点/.test(result.error || '')) {
    const {ElMessageBox}=await import('element-plus')
    try { await ElMessageBox.confirm(`标准 Word 导出未生成文件：${result.error}。可生成带标注的兼容副本：公式和脚注保留源码，图表保留代码，无法嵌入的图片保留来源标记。需要精确排版请取消并导出 PDF。`, 'Word 兼容副本', {confirmButtonText:'生成带标注的 Word',cancelButtonText:'取消',type:'warning'}) }
    catch { return null }
    result=await tauriCallSafe('export_editor_document',{document,markdown,title:input.title,format:'docx-annotated',outputPath})
  }
  if (!result.ok || !result.data) throw new Error(result.error || "导出失败");
  return result.data.outputPath;
  }, () => exportDocument(input));
}
