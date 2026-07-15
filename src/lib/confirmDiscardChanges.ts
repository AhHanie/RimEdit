import { confirm } from "@tauri-apps/plugin-dialog";
import { initI18n } from "../i18n";

/** `message` is pre-translated by the caller (each call site knows its own context/count), while
 * the shared title and button labels are resolved here from the app-wide i18next singleton -- this
 * is a plain utility, not a React component, so there is no `useTranslation()` hook to call. Uses
 * the "key, defaultValue" call form (like `src/i18n/diagnostics.ts`'s `translate` helper) since a
 * bare `i18n.t()` call outside a `useTranslation()`-scoped namespace list does not get the typed
 * literal-key narrowing that in-component calls do. */
export function confirmDiscardChanges(message: string): Promise<boolean> {
  const i18n = initI18n();
  return confirm(message, {
    title: i18n.t("shell:confirm.discardChangesTitle", "Discard unsaved changes?"),
    kind: "warning",
    okLabel: i18n.t("common:actions.discard", "Discard"),
    cancelLabel: i18n.t("common:actions.cancel", "Cancel"),
  });
}
