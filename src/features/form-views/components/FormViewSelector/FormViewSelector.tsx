import type { UseFormViewsResult } from "../../hooks/useFormViews";
import { useViewSwitchConfirmation } from "../../hooks/useViewSwitchConfirmation";
import type { ResolvedFormView } from "../../types/resolvedFormView";
import type { FormViewOrigin, SelectedFormViewRef } from "../../types/formViews";
import type { HiddenFieldDiagnosticsSummary } from "../../lib/hiddenFieldDiagnostics";
import styles from "./FormViewSelector.module.css";

interface Props {
  controller: UseFormViewsResult;
  onOpenManager: () => void;
  /** Issue 08 (Plan.md section 8 "Hidden validation feedback"): validation diagnostics for the
   * selected Def that map to a currently hidden top-level root. `null`/omitted (and a summary
   * with `totalCount === 0`) renders no summary/reveal affordance at all -- callers that don't
   * yet compute this (or Patch/About/raw, which never render this component) get byte-identical
   * behavior to before this issue. */
  hiddenIssues?: HiddenFieldDiagnosticsSummary | null;
  /** `Reveal fields with issues` (Plan.md section 8): unhides only `hiddenIssues.affectedRootIds`
   * via the SAME `setOverrideHiddenFieldIds` override mechanism issue 07's checkboxes use --
   * never a Custom View mutation, never a view switch. Required whenever `hiddenIssues` has a
   * nonzero count; unused otherwise. */
  onReveal?: () => void;
}

/** Stable DOM id for the `<select>` -- used both by tests and as the focus-restoration target
 * when a Form View visibility change hides the currently-focused form field (Plan.md section 7:
 * "restore focus to the selector/customize control if the focused field is removed"). */
export const FORM_VIEW_SELECTOR_SELECT_ID = "form-view-selector-select";

function optionValue(view: Pick<ResolvedFormView, "origin" | "id">): string {
  return `${view.origin}:${view.id}`;
}

function parseOptionValue(value: string): SelectedFormViewRef {
  const idx = value.indexOf(":");
  return { origin: value.slice(0, idx) as FormViewOrigin, id: value.slice(idx + 1) };
}

function sourceText(view: ResolvedFormView): string | null {
  if (view.origin === "schema") {
    return view.source
      ? `Schema view · ${view.source.packId} ${view.source.packVersion}`
      : "Schema view · read-only";
  }
  return null;
}

/**
 * The compact Form View row rendered directly below `XmlFormEditor`'s existing multi-Def
 * selector and above `.fields` (Plan.md section 8). Only ever rendered for Def forms with a
 * resolvable schema -- `controller.applicable` mirrors that gate so callers don't need to
 * duplicate the profile/schema check.
 */
export function FormViewSelector({ controller, onOpenManager, hiddenIssues, onReveal }: Props) {
  const { requestSwitch, switchConfirmDialog } = useViewSwitchConfirmation(controller);

  if (!controller.applicable) return null;

  const { availableViews, selectedView, hasDirtyOverride, hiddenCount, persistWarning } = controller;
  const hasHiddenIssues = !!hiddenIssues && hiddenIssues.totalCount > 0;
  const defaultView = availableViews.find((v) => v.origin === "default");
  const schemaViews = availableViews.filter((v) => v.origin === "schema");
  const customViews = availableViews.filter((v) => v.origin === "custom");

  function handleSelectChange(e: React.ChangeEvent<HTMLSelectElement>) {
    requestSwitch(parseOptionValue(e.target.value));
  }

  const showFullFormDisabled = selectedView.origin === "default" && !hasDirtyOverride;
  const currentSourceText = sourceText(selectedView);

  return (
    <div className={styles.root}>
      <label htmlFor={FORM_VIEW_SELECTOR_SELECT_ID} className={styles.label}>
        View
      </label>
      <select
        id={FORM_VIEW_SELECTOR_SELECT_ID}
        className={styles.select}
        value={optionValue(selectedView)}
        onChange={handleSelectChange}
      >
        {defaultView && <option value={optionValue(defaultView)}>{defaultView.label}</option>}
        {schemaViews.length > 0 && (
          <optgroup label="Schema views">
            {schemaViews.map((v) => (
              <option key={optionValue(v)} value={optionValue(v)}>
                {v.label}
                {v.recommended ? " (recommended)" : ""}
              </option>
            ))}
          </optgroup>
        )}
        {customViews.length > 0 && (
          <optgroup label="Custom views">
            {customViews.map((v) => (
              <option key={optionValue(v)} value={optionValue(v)}>
                {v.label}
              </option>
            ))}
          </optgroup>
        )}
      </select>

      {currentSourceText && <span className={styles.sourceText}>{currentSourceText}</span>}

      <button type="button" className={styles.actionBtn} onClick={onOpenManager}>
        Customize view
      </button>

      <button
        type="button"
        className={styles.actionBtn}
        onClick={() => requestSwitch({ origin: "default", id: defaultView?.id ?? "default" })}
        disabled={showFullFormDisabled}
      >
        Show full form
      </button>

      <div className={styles.rightGroup}>
        {/* Issue 08 (Plan.md section 8 "Hidden validation feedback"): counts validation
         * diagnostics for the selected Def that map to a root the active Form View is currently
         * hiding. Wording is accessible text, not color alone -- the blocking count (if any) is
         * spelled out rather than conveyed by a color swatch. Clicking `Reveal fields with
         * issues` is the ONLY thing that ever changes visibility here -- merely having a hidden
         * diagnostic present never auto-reveals anything (Plan.md: "Do not automatically reveal
         * errors"). */}
        {hasHiddenIssues && (
          <div className={styles.hiddenIssuesIndicator} role="status">
            <span className={styles.hiddenIssuesText}>
              {hiddenIssues!.totalCount} hidden field issue{hiddenIssues!.totalCount === 1 ? "" : "s"}
              {hiddenIssues!.blockingCount > 0 &&
                ` (${hiddenIssues!.blockingCount} blocking)`}
            </span>
            <button type="button" className={styles.linkBtn} onClick={onReveal}>
              Reveal fields with issues
            </button>
          </div>
        )}

        {hasDirtyOverride && (
          <div className={styles.overrideIndicator} role="status">
            <span className={styles.overrideText}>
              Modified · {hiddenCount} hidden
            </span>
            <button type="button" className={styles.linkBtn} onClick={controller.resetOverride}>
              Reset
            </button>
            <button type="button" className={styles.linkBtn} onClick={controller.resetOverride}>
              Discard
            </button>
          </div>
        )}
      </div>

      {/* Visible even when the manager dialog is closed -- a failed persist can happen from
       * this row alone (plain selector change or "Show full form"), so it must not only be
       * discoverable by opening the dialog. Cleared automatically once a later persist
       * succeeds (see `useFormViews`). */}
      {persistWarning && (
        <span className={styles.persistWarning} role="alert">
          {persistWarning}
        </span>
      )}

      {switchConfirmDialog}
    </div>
  );
}
