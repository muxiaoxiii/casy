import { ref, watch, onBeforeUnmount } from "vue";
import type { Editor } from "@tiptap/core";
import { ElMessage } from "element-plus";
import { tauriCallSafe } from "../../core/tauriBridge";
import { casyContext } from '../../core/plugin/context';
import { observeChanges } from '../../core/observeChanges';
export function useEditorTasks(
  getEditor: () => Editor | undefined,
  source: {
    sourceId?: string;
    sourceType?: "knowledge" | "doc";
    caseId?: string | null;
  },
) {
  const status = ref(""),
    busy = ref(false);
  const known = new Map<string, boolean>();
  let requestCount = 0;
  const bindingIds = new Map<string, string>();
  let sourceVersion = 0;
  let applying = false,
    disposed = false,
    queue = Promise.resolve();
  const target = () => ({
    sourceType: source.sourceType!,
    sourceId: source.sourceId!,
  });
  function items() {
    const found: Array<{ id: string; checked: boolean; pos: number }> = [];
    getEditor()?.state.doc.descendants((node, pos) => {
      if (node.type.name === "taskItem" && node.attrs.taskId)
        found.push({
          id: node.attrs.taskId,
          checked: !!node.attrs.checked,
          pos,
        });
    });
    return found;
  }
  function patch(id: string, checked: boolean) {
    const editor = getEditor();
    if (!editor || editor.isDestroyed) return;
    applying = true;
    let tr = editor.state.tr;
    for (const item of items())
      if (item.id === id) {
        const node = tr.doc.nodeAt(item.pos);
        if (node && !!node.attrs.checked !== checked)
          tr = tr.setNodeMarkup(item.pos, undefined, {
            ...node.attrs,
            checked,
          });
      }
    if (tr.docChanged) editor.view.dispatch(tr);
    applying = false;
  }
  async function refresh() {
    if (!source.sourceId || !source.sourceType || busy.value) return;
    if (!items().length) { known.clear(); status.value = ""; return; }
    const version = sourceVersion;
    const result = await tauriCallSafe("get_editor_tasks", target());
    if (disposed || version !== sourceVersion || busy.value) return;
    if (!result.ok || !result.data) {
      status.value = result.error || "任务状态读取失败";
      return;
    }
    status.value = "";
    const used = new Set(items().map((item) => item.id));
    const linked = new Set(result.data.map((task) => task.id));
    if ([...used].some((id) => !linked.has(id)))
      status.value = "有未登记的任务引用，暂不能同步；请核对来源文档";
    known.clear();
    for (const task of result.data) {
      if (!used.has(task.id)) continue;
      if (task.missing) {
        status.value = "有已移除的关联任务；正文清单仍保留";
        continue;
      }
      known.set(task.id, task.completed);
      patch(task.id, task.completed);
    }
  }
  function changed() {
    if (applying || !known.size || !source.sourceId || !source.sourceType) return;
    for (const item of items())
      if (known.has(item.id) && known.get(item.id) !== item.checked) {
        const expected = known.get(item.id)!;
        const requestTarget = target();
        const version = sourceVersion;
        known.set(item.id, item.checked);
        requestCount++;
        busy.value = true;
        queue = queue
          .then(async () => {
            const result = await tauriCallSafe("set_editor_task_completed", {
              ...requestTarget,
              taskId: item.id,
              completed: item.checked,
              expectedCompleted: expected,
            });
            if (result.ok) casyContext.emit('task:changed', { id: item.id });
            if (!result.ok && !disposed && version === sourceVersion) {
              status.value = result.error || "任务同步失败";
              known.set(item.id, expected);
              patch(item.id, expected);
              ElMessage.error(status.value);
            }
          })
          .finally(() => {
            requestCount--;
            busy.value = requestCount > 0;
          });
      }
  }
  function currentItem() {
    const editor = getEditor();
    if (!editor) return null;
    const { $from } = editor.state.selection;
    for (let depth = $from.depth; depth > 0; depth--)
      if ($from.node(depth).type.name === "taskItem")
        return { pos: $from.before(depth), node: $from.node(depth) };
    return null;
  }
  async function bind(taskId: string | null = null) {
    const editor = getEditor(),
      item = currentItem();
    if (!editor || !item)
      return ElMessage.info("先将光标放在需要关联的待办项中");
    if (!source.sourceId || !source.sourceType)
      return ElMessage.warning("请先保存文档再关联任务");
    if (item.node.attrs.taskId) return ElMessage.info("此待办已关联任务");
    const title = item.node.textContent.trim();
    if (!title) return ElMessage.warning("请先填写待办内容");
    if (busy.value) return;
    busy.value = true;
    const requestTarget = target();
    const version = sourceVersion;
    const bindingKey = `${requestTarget.sourceType}:${requestTarget.sourceId}:${item.pos}:${title}`;
    if (!bindingIds.has(bindingKey))
      bindingIds.set(bindingKey, crypto.randomUUID());
    try {
      const result = await tauriCallSafe("bind_editor_task", {
        ...requestTarget,
        bindingId: bindingIds.get(bindingKey)!,
        title,
        taskId,
        caseId: source.caseId || null,
      });
      if (!result.ok || !result.data)
        throw new Error(result.error || "关联失败");
      casyContext.emit('task:changed', { id: result.data });
      if (disposed || editor.isDestroyed || version !== sourceVersion) return;
      const current = editor.state.doc.nodeAt(item.pos);
      if (
        current?.type.name !== "taskItem" ||
        current.textContent !== item.node.textContent
      ) {
        status.value = "任务已创建，请在任务中心核对后重新关联";
        return;
      }
      editor.view.dispatch(
        editor.state.tr.setNodeMarkup(item.pos, undefined, {
          ...current.attrs,
          taskId: result.data,
        }),
      );
      if (!taskId && current.attrs.checked) {
        const completed = await tauriCallSafe("set_editor_task_completed", {
          ...requestTarget,
          taskId: result.data,
          completed: true,
          expectedCompleted: false,
        });
        if (!completed.ok)
          throw new Error(`任务已关联，但完成状态同步失败：${completed.error}`);
      }
      bindingIds.delete(bindingKey);
      ElMessage.success("已关联任务，勾选状态将同步");
    } catch (error) {
      status.value = String(error);
      ElMessage.error(status.value);
    } finally {
      busy.value = false;
      await refresh();
    }
  }
  function focus() {
    void queue.then(refresh);
  }
  watch(
    () => [source.sourceType, source.sourceId],
    () => {
      sourceVersion++;
      known.clear();
      status.value = "";
      void queue.then(refresh);
    },
  );
  const stopChanges = observeChanges(casyContext, ['task'], async () => { await queue; await refresh(); });
  if (typeof window !== "undefined") window.addEventListener("focus", focus);
  onBeforeUnmount(() => {
    disposed = true;
    stopChanges();
    window.removeEventListener("focus", focus);
  });
  return { status, busy, changed, refresh, bind, currentItem };
}
