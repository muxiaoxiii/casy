import TaskItem from "@tiptap/extension-task-item";
export const LinkedTaskItem = TaskItem.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      taskId: {
        default: null,
        parseHTML: (el) => el.getAttribute("data-task-id"),
        renderHTML: (attrs) =>
          attrs.taskId ? { "data-task-id": attrs.taskId } : {},
      },
    };
  },
}).configure({ nested: true, HTMLAttributes: { "data-type": "taskItem" } });
