import { confirm } from "@tauri-apps/plugin-dialog";

export function confirmDiscardChanges(message: string): Promise<boolean> {
  return confirm(message, {
    title: "Discard unsaved changes?",
    kind: "warning",
    okLabel: "Discard",
    cancelLabel: "Cancel",
  });
}
