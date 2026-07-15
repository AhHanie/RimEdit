import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { formatError } from "../../../../lib/formatError";
import {
  collectTopLevelFieldSummaries,
  type TopLevelFieldSummary,
} from "../../../xml-editor/lib/formDescriptors";
import type { DefEditorView } from "../../../xml-editor/types/xmlDocument";
import type { DefTypeSchema, SchemaCatalog } from "../../../schema-catalog";
import type { UseFormViewsResult } from "../../hooks/useFormViews";
import { toggleHiddenFieldId } from "../../lib/resolveFormViews";
import styles from "./FormViewFieldChecklist.module.css";

interface Props {
  controller: UseFormViewsResult;
  def: DefEditorView;
  defSchema: DefTypeSchema;
  catalog: SchemaCatalog;
}

function badgeLabel(summary: TopLevelFieldSummary, t: TFunction<"editor">): string {
  if (summary.isSection) return t("formViews.checklist.badge.section");
  switch (summary.controlKind) {
    case "checkbox":
      return t("formViews.checklist.badge.boolean");
    case "number":
      return t("formViews.checklist.badge.number");
    case "select":
      return t("formViews.checklist.badge.enum");
    case "reference":
      return t("formViews.checklist.badge.reference");
    case "list":
      return t("formViews.checklist.badge.list");
    case "objectList":
      return t("formViews.checklist.badge.list");
    case "namedMap":
      return t("formViews.checklist.badge.map");
    case "flags":
      return t("formViews.checklist.badge.flags");
    case "typedReferenceList":
      return t("formViews.checklist.badge.referenceList");
    case "color":
      return t("formViews.checklist.badge.color");
    case "object":
      return t("formViews.checklist.badge.object");
    case "readonlyUnknown":
      return t("formViews.checklist.badge.unknown");
    case "textarea":
    case "text":
    default:
      return t("formViews.checklist.badge.text");
  }
}

/**
 * The per-field checkbox grid inside `FormViewManagerDialog`'s customize area (issue 07,
 * Plan.md section 8). Sourced from `collectTopLevelFieldSummaries` -- the canonical top-level
 * schema field universe, not rendered `FormFieldModel`s -- so a currently-hidden field still has
 * a checkbox row here even though its own control never mounts in the form itself. Every toggle
 * (search-filtered checkbox, Show all, Hide all) funnels through the same
 * `toggleHiddenFieldId`/`setOverrideHiddenFieldIds` pair `XmlFormEditor`'s inline hide buttons
 * use, so there is exactly one place that decides what "hidden" means.
 */
export function FormViewFieldChecklist({ controller, def, defSchema, catalog }: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const [filter, setFilter] = useState("");
  const [savingChanges, setSavingChanges] = useState(false);
  const [saveAsOpen, setSaveAsOpen] = useState(false);
  const [saveAsName, setSaveAsName] = useState("");
  const [savingAs, setSavingAs] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const summaries = useMemo(
    () => collectTopLevelFieldSummaries(def, defSchema, catalog),
    [def, defSchema, catalog],
  );

  const { effectiveHidden, selectedView, hasDirtyOverride, hiddenCount } = controller;

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return summaries;
    return summaries.filter(
      (s) => s.label.toLowerCase().includes(q) || s.id.toLowerCase().includes(q),
    );
  }, [summaries, filter]);

  function toggle(id: string) {
    controller.setOverrideHiddenFieldIds(toggleHiddenFieldId(effectiveHidden, id));
  }

  function showAll() {
    controller.setOverrideHiddenFieldIds(new Set());
  }

  function hideAll() {
    controller.setOverrideHiddenFieldIds(new Set(summaries.map((s) => s.id)));
  }

  async function handleSaveChanges() {
    if (selectedView.origin !== "custom" || !controller.override) return;
    setSavingChanges(true);
    setError(null);
    // Same staleness-guard idiom as `useFormViews`'s own chained-async call sites (see
    // `saveOverrideAsCustomView`'s doc comment): if the Def/tab/game-version scope has moved on
    // by the time `updateCustomView` resolves, the write itself already landed correctly under
    // its ORIGINAL scope, but clearing the override here would clear whatever (unrelated)
    // override belongs to the scope that's active NOW.
    const startScopeKey = controller.getScopeKey();
    try {
      // Preserve orphaned ids (Plan.md section 12 / issue 07 edge case: "a root no longer in
      // schema remains stored but is not listed until it returns"). `effectiveHidden`/the
      // override are already intersected with the CURRENT known field universe -- they can
      // never carry a stale field id forward. Without this union, saving would silently
      // overwrite the view's stored `hiddenFieldIds` with only the checklist-representable
      // subset, permanently dropping anything the checklist can't show a row for (a field
      // removed from the schema, or a nested/legacy id predating this issue). Union the
      // locally-toggled known-field set with whatever ids were ALREADY stored on this exact
      // view that this checklist doesn't/can't represent, so an unrelated toggle never erases
      // them.
      const knownIds = new Set(summaries.map((s) => s.id));
      const orphanedIds = selectedView.hiddenFieldIds.filter((id) => !knownIds.has(id));
      const hiddenFieldIds = [...new Set([...controller.override.hiddenFieldIds, ...orphanedIds])];
      await controller.updateCustomView(selectedView.id, { hiddenFieldIds });
      if (controller.getScopeKey() !== startScopeKey) return;
      controller.resetOverride();
    } catch (e: unknown) {
      if (controller.getScopeKey() !== startScopeKey) return;
      setError(formatError(e));
    } finally {
      setSavingChanges(false);
    }
  }

  async function handleSaveAsCustom() {
    const trimmed = saveAsName.trim();
    if (!trimmed || savingAs) return;
    setSavingAs(true);
    setError(null);
    // Same staleness guard as `handleSaveChanges` above -- `saveOverrideAsCustomView` already
    // guards its OWN internal auto-select side effect, but that does not stop THIS component's
    // local UI state (the inline name input, its open/closed state, any error banner) from
    // being mutated by a stale completion after the scope has moved on to a different Def. This
    // component instance is not remounted on a Def switch (it's re-rendered with new props in
    // place), so without this guard a save started for one Def could clear/overwrite whatever
    // the user is now doing in this same panel for a DIFFERENT Def -- e.g. wiping a name they
    // are mid-typing into a fresh "Save as custom view" prompt that has nothing to do with the
    // stale completion.
    const startScopeKey = controller.getScopeKey();
    try {
      await controller.saveOverrideAsCustomView(trimmed);
      if (controller.getScopeKey() !== startScopeKey) return;
      setSaveAsOpen(false);
      setSaveAsName("");
    } catch (e: unknown) {
      if (controller.getScopeKey() !== startScopeKey) return;
      setError(formatError(e));
    } finally {
      setSavingAs(false);
    }
  }

  const allHidden = summaries.length > 0 && summaries.every((s) => effectiveHidden.has(s.id));

  return (
    <div className={styles.root}>
      <div className={styles.toolbar}>
        <input
          type="text"
          className={styles.filterInput}
          placeholder={t("formViews.checklist.filterPlaceholder")}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          aria-label={t("formViews.checklist.filterAriaLabel")}
        />
        <button type="button" className={styles.smallBtn} onClick={showAll}>
          {t("formViews.checklist.showAll")}
        </button>
        <button type="button" className={styles.smallBtn} onClick={hideAll}>
          {t("formViews.checklist.hideAll")}
        </button>
        <button
          type="button"
          className={styles.smallBtn}
          onClick={controller.resetOverride}
          disabled={!hasDirtyOverride}
        >
          {t("formViews.checklist.resetToSelected")}
        </button>
      </div>

      <div className={styles.summaryRow}>
        <span className={styles.hiddenCount}>
          {t("formViews.checklist.hiddenSummary", {
            hiddenCount,
            total: summaries.length,
          })}
        </span>
      </div>

      {allHidden && (
        <p className={styles.warningBanner} role="status">
          {t("formViews.checklist.allHiddenWarning")}
        </p>
      )}

      <ul className={styles.checklist}>
        {filtered.map((s) => (
          <li key={s.id} className={styles.checklistRow}>
            <label className={styles.checklistLabel}>
              <input
                type="checkbox"
                checked={!effectiveHidden.has(s.id)}
                onChange={() => toggle(s.id)}
                aria-label={s.label}
              />
              <span className={styles.fieldLabelText}>{s.label}</span>
              <span className={styles.typeBadge}>{badgeLabel(s, t)}</span>
              {s.hasValue && (
                <span className={styles.valueBadge}>{t("formViews.checklist.hasValue")}</span>
              )}
            </label>
          </li>
        ))}
        {filtered.length === 0 && (
          <li className={styles.emptyRow}>{t("formViews.checklist.noMatch", { filter })}</li>
        )}
      </ul>

      <p className={styles.unknownNote}>{t("formViews.checklist.unknownNote")}</p>

      {error && <p className={styles.errorBanner}>{error}</p>}

      <div className={styles.saveRow}>
        {selectedView.origin === "custom" ? (
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={() => void handleSaveChanges()}
            disabled={!hasDirtyOverride || savingChanges}
          >
            {savingChanges
              ? t("formViews.checklist.saving")
              : t("formViews.checklist.saveChanges")}
          </button>
        ) : saveAsOpen ? (
          <div className={styles.saveAsRow}>
            <input
              type="text"
              className={styles.saveAsInput}
              placeholder={t("formViews.checklist.saveAsPlaceholder")}
              value={saveAsName}
              onChange={(e) => setSaveAsName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSaveAsCustom();
                if (e.key === "Escape") setSaveAsOpen(false);
              }}
              autoFocus
              aria-label={t("formViews.checklist.saveAsAriaLabel")}
            />
            <button
              type="button"
              className={styles.primaryBtn}
              onClick={() => void handleSaveAsCustom()}
              disabled={!saveAsName.trim() || savingAs}
            >
              {savingAs ? t("formViews.checklist.saving") : tCommon("actions.save")}
            </button>
            <button
              type="button"
              className={styles.smallBtn}
              onClick={() => setSaveAsOpen(false)}
              disabled={savingAs}
            >
              {tCommon("actions.cancel")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={() => setSaveAsOpen(true)}
            disabled={!hasDirtyOverride}
          >
            {t("formViews.checklist.saveAsCustomView")}
          </button>
        )}
      </div>
    </div>
  );
}
