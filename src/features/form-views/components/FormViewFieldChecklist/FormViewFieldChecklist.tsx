import { useMemo, useState } from "react";
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

function badgeLabel(summary: TopLevelFieldSummary): string {
  if (summary.isSection) return "Section";
  switch (summary.controlKind) {
    case "checkbox":
      return "Boolean";
    case "number":
      return "Number";
    case "select":
      return "Enum";
    case "reference":
      return "Reference";
    case "list":
      return "List";
    case "objectList":
      return "List";
    case "namedMap":
      return "Map";
    case "flags":
      return "Flags";
    case "typedReferenceList":
      return "Reference list";
    case "color":
      return "Color";
    case "object":
      return "Object";
    case "readonlyUnknown":
      return "Unknown";
    case "textarea":
    case "text":
    default:
      return "Text";
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
          placeholder="Filter fields..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          aria-label="Filter fields"
        />
        <button type="button" className={styles.smallBtn} onClick={showAll}>
          Show all
        </button>
        <button type="button" className={styles.smallBtn} onClick={hideAll}>
          Hide all
        </button>
        <button
          type="button"
          className={styles.smallBtn}
          onClick={controller.resetOverride}
          disabled={!hasDirtyOverride}
        >
          Reset to selected view
        </button>
      </div>

      <div className={styles.summaryRow}>
        <span className={styles.hiddenCount}>
          {hiddenCount} of {summaries.length} hidden
        </span>
      </div>

      {allHidden && (
        <p className={styles.warningBanner} role="status">
          All fields are hidden. Default View and unknown XML fields remain available.
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
              <span className={styles.typeBadge}>{badgeLabel(s)}</span>
              {s.hasValue && <span className={styles.valueBadge}>Has value</span>}
            </label>
          </li>
        ))}
        {filtered.length === 0 && (
          <li className={styles.emptyRow}>No fields match &quot;{filter}&quot;.</li>
        )}
      </ul>

      <p className={styles.unknownNote}>
        XML content not described by the active schema is always shown separately below, and is
        never affected by Form Views.
      </p>

      {error && <p className={styles.errorBanner}>{error}</p>}

      <div className={styles.saveRow}>
        {selectedView.origin === "custom" ? (
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={() => void handleSaveChanges()}
            disabled={!hasDirtyOverride || savingChanges}
          >
            {savingChanges ? "Saving…" : "Save changes"}
          </button>
        ) : saveAsOpen ? (
          <div className={styles.saveAsRow}>
            <input
              type="text"
              className={styles.saveAsInput}
              placeholder="Custom view name"
              value={saveAsName}
              onChange={(e) => setSaveAsName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSaveAsCustom();
                if (e.key === "Escape") setSaveAsOpen(false);
              }}
              autoFocus
              aria-label="Custom view name"
            />
            <button
              type="button"
              className={styles.primaryBtn}
              onClick={() => void handleSaveAsCustom()}
              disabled={!saveAsName.trim() || savingAs}
            >
              {savingAs ? "Saving…" : "Save"}
            </button>
            <button
              type="button"
              className={styles.smallBtn}
              onClick={() => setSaveAsOpen(false)}
              disabled={savingAs}
            >
              Cancel
            </button>
          </div>
        ) : (
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={() => setSaveAsOpen(true)}
            disabled={!hasDirtyOverride}
          >
            Save as custom view
          </button>
        )}
      </div>
    </div>
  );
}
