import { useRef, useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Check, Pencil, X } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import type { DefEditorView } from "../../../xml-editor/types/xmlDocument";
import type { DefTypeSchema, SchemaCatalog } from "../../../schema-catalog";
import type { UseFormViewsResult } from "../../hooks/useFormViews";
import { useViewSwitchConfirmation } from "../../hooks/useViewSwitchConfirmation";
import { useDialogKeyboard } from "../../lib/useDialogKeyboard";
import { isCustomViewBaseUnavailable } from "../../lib/resolveFormViews";
import type { ResolvedFormView } from "../../types/resolvedFormView";
import { FormViewFieldChecklist } from "../FormViewFieldChecklist/FormViewFieldChecklist";
import { FormViewSwitchConfirmDialog } from "../FormViewSwitchConfirmDialog/FormViewSwitchConfirmDialog";
import styles from "./FormViewManagerDialog.module.css";

/** The current selection's schema/catalog/live-XML context the field checklist needs to build
 * its row list (issue 07). `null` when Form Views aren't applicable for the current selection
 * (mirrors `controller.applicable`) -- the checklist section renders nothing in that case,
 * matching every other Form View control's "no schema, no controls" contract. */
export interface FormViewFieldChecklistTarget {
  def: DefEditorView;
  defSchema: DefTypeSchema;
  catalog: SchemaCatalog;
}

interface Props {
  controller: UseFormViewsResult;
  onClose: () => void;
  fieldChecklistTarget?: FormViewFieldChecklistTarget | null;
}

function isSelected(view: ResolvedFormView, selected: ResolvedFormView): boolean {
  return view.origin === selected.origin && view.id === selected.id;
}

function sourceText(view: ResolvedFormView): string {
  if (view.origin === "default") return "Always available · read-only";
  if (view.origin === "schema") {
    return view.source
      ? `Schema pack · ${view.source.packId} ${view.source.packVersion} · read-only`
      : "Schema pack · read-only";
  }
  return "Custom view";
}

/**
 * The "Customize view" overlay (Plan.md section 8): lists every available view with its
 * source/read-only annotation, and provides create/duplicate/rename/delete for custom views.
 * Schema views and Default are never editable/deletable, only duplicable into a new custom
 * view (Plan.md section 17: "schema-defined views cannot be edited/deleted but can be
 * duplicated"). The per-field visibility checkbox grid (Plan.md section 8's "lists effective
 * top-level fields ... uses checkboxes for visible state") is issue 07's addition -- this
 * dialog's list/CRUD surface is real now; the customization area below it is an explicit
 * placeholder issue 07 replaces, not a silently-broken button.
 */
export function FormViewManagerDialog({ controller, onClose, fieldChecklistTarget }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  // Every close path (Escape via `useDialogKeyboard`, the header X, the footer Close button)
  // routes through `requestClose`, not `onClose` directly, so a dirty override is never silently
  // discarded just because the user closed the dialog rather than switching views (issue 07 step
  // 6: "use same decision model as view switch").
  useDialogKeyboard(containerRef, () => requestClose());
  const { requestSwitch, switchConfirmDialog } = useViewSwitchConfirmation(controller);

  const [newViewName, setNewViewName] = useState("");
  const [creating, setCreating] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [busyViewId, setBusyViewId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [storeRecoveryBusy, setStoreRecoveryBusy] = useState(false);
  const [storeRecoveryMessage, setStoreRecoveryMessage] = useState<string | null>(null);
  // Close-with-dirty-override confirmation (issue 07 step 6): reuses `FormViewSwitchConfirmDialog`
  // verbatim -- the same three-way discard/save-as-custom/cancel decision `useViewSwitchConfirmation`
  // already presents for an ordinary view switch, not a second divergent confirmation UI.
  const [closeConfirmPending, setCloseConfirmPending] = useState(false);

  function requestClose() {
    if (controller.hasDirtyOverride) {
      setCloseConfirmPending(true);
      return;
    }
    onClose();
  }

  const { availableViews, selectedView } = controller;
  const defaultView = availableViews.find((v) => v.origin === "default");
  const schemaViews = availableViews.filter((v) => v.origin === "schema");
  const customViews = availableViews.filter((v) => v.origin === "custom");

  async function handleCreate() {
    const trimmed = newViewName.trim();
    if (!trimmed || creating) return;
    setCreating(true);
    setError(null);
    // Captured before the async call: this dialog instance stays MOUNTED across a Def/scope
    // change (`XmlFormEditor` re-renders it with a new `controller`/`fieldChecklistTarget`
    // rather than unmounting it), so every local state update below -- not just the
    // `requestSwitch`/`selectView` chain into shared controller state -- must be re-checked
    // against the live scope before it lands, or a stale completion from an abandoned Def can
    // still overwrite whatever the user is now doing for the CURRENT Def. `getScopeKey()` is a
    // stable function that always reads the live current scope, unlike everything else on
    // `controller`, which is a snapshot frozen at this render.
    const startScopeKey = controller.getScopeKey();
    try {
      const created = await controller.createCustomView(trimmed);
      if (controller.getScopeKey() !== startScopeKey) {
        // The scope moved on while this create was in flight -- the view was still created
        // (under its original scope, which is correct), but clearing `newViewName` here would
        // wipe out whatever the user is now typing for an unrelated create attempt under the
        // CURRENT scope, and auto-selecting it would apply to whatever Def/scope is active NOW,
        // which has nothing to do with this create.
        return;
      }
      setNewViewName("");
      // Creating a view doesn't itself change the selection (see `createCustomView`'s doc
      // comment) -- auto-selecting the freshly created view goes through the same
      // dirty-override switch confirmation as any other view change, so an in-progress
      // unsaved override is never silently discarded just because the user created an
      // unrelated new view.
      requestSwitch({ origin: "custom", id: created.id });
    } catch (e: unknown) {
      if (controller.getScopeKey() === startScopeKey) {
        setError(formatError(e));
      }
    } finally {
      // Unlike the content-bearing updates above, resetting the busy flag is always safe to
      // apply regardless of scope: it only re-enables the Create button, it never displays or
      // discards anything belonging to a different scope. Leaving it stuck `true` forever on a
      // stale completion would be a real (if lesser) bug of its own.
      setCreating(false);
    }
  }

  async function handleDuplicate(view: ResolvedFormView) {
    setBusyViewId(`${view.origin}:${view.id}`);
    setError(null);
    const startScopeKey = controller.getScopeKey();
    try {
      await controller.duplicateAsCustomView(view);
    } catch (e: unknown) {
      // A stale failure must not display an error banner against whatever scope/Def is
      // actually being looked at now -- see `handleCreate`'s doc comment for why this dialog
      // instance staying mounted across a scope change matters here.
      if (controller.getScopeKey() === startScopeKey) {
        setError(formatError(e));
      }
    } finally {
      setBusyViewId(null);
    }
  }

  function startRename(view: ResolvedFormView) {
    setRenamingId(view.id);
    setRenameDraft(view.label);
  }

  async function commitRename(viewId: string) {
    const trimmed = renameDraft.trim();
    if (!trimmed) {
      setRenamingId(null);
      return;
    }
    setBusyViewId(`custom:${viewId}`);
    setError(null);
    const startScopeKey = controller.getScopeKey();
    try {
      await controller.renameCustomView(viewId, trimmed);
      // Only exit rename mode if the scope hasn't moved on -- a stale rename completion must
      // not clear/overwrite a DIFFERENT rename the user has since started for the view that's
      // actually current now (`renamingId`/`renameDraft` are this dialog's own local state,
      // which persists across a scope change exactly like `newViewName` does).
      if (controller.getScopeKey() === startScopeKey) {
        setRenamingId(null);
      }
    } catch (e: unknown) {
      if (controller.getScopeKey() === startScopeKey) {
        setError(formatError(e));
      }
    } finally {
      setBusyViewId(null);
    }
  }

  // `handleRetryLoad`/`handleResetStore` are deliberately NOT scope-guarded the way the
  // handlers above are. Both operate on data whose own scope is coarser than -- and does not
  // depend on -- the `{project, gameVersion, defType, ordinal}` selection/override scope that
  // motivates the guards elsewhere in this file:
  //  - `reloadCustomViews` (-> `useCustomFormViews.reload`) already carries its OWN internal
  //    `scopeEpochRef` staleness guard (see `useCustomFormViews.ts`): a stale reload's result is
  //    discarded there regardless of whether this dialog's local `storeRecoveryBusy` state is
  //    guarded, so a second guard here would be redundant, not protective.
  //  - `resetCustomFormViewStore` operates per-PROJECT (it takes only `projectId`, not
  //    `gameVersion`/`defType` -- see `../../api/formViews.ts`), backing up/clearing the entire
  //    custom-view store file. Its success/failure message ("Store reset", a backup path, or an
  //    error) remains accurate regardless of which Def is now selected within the SAME project;
  //    only an actual project switch would invalidate it, and a project switch tears down every
  //    tab/pane (`useEditorWorkspace`), which unmounts this dialog outright rather than leaving
  //    it mounted with a stale scope.
  async function handleRetryLoad() {
    setStoreRecoveryBusy(true);
    setStoreRecoveryMessage(null);
    try {
      await controller.reloadCustomViews();
    } finally {
      setStoreRecoveryBusy(false);
    }
  }

  async function handleResetStore() {
    const ok = await confirm(
      "Reset the custom Form View store? The current (possibly corrupt) store file will be backed up, and you'll start with no custom views. This does not affect Default or schema views.",
      {
        title: "Reset Form View store",
        kind: "warning",
        okLabel: "Reset store",
        cancelLabel: "Cancel",
      },
    );
    if (!ok) return;
    setStoreRecoveryBusy(true);
    setStoreRecoveryMessage(null);
    try {
      const result = await controller.resetCustomViewStore();
      setStoreRecoveryMessage(
        result.backupPath
          ? `Store reset. The previous file was backed up to ${result.backupPath}.`
          : "Store reset.",
      );
    } catch (e: unknown) {
      setStoreRecoveryMessage(formatError(e));
    } finally {
      setStoreRecoveryBusy(false);
    }
  }

  async function handleDelete(view: ResolvedFormView) {
    const ok = await confirm(`Delete the custom Form View "${view.label}"? This cannot be undone.`, {
      title: "Delete Form View",
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    setBusyViewId(`custom:${view.id}`);
    setError(null);
    const startScopeKey = controller.getScopeKey();
    try {
      await controller.deleteCustomView(view.id);
    } catch (e: unknown) {
      if (controller.getScopeKey() === startScopeKey) {
        setError(formatError(e));
      }
    } finally {
      setBusyViewId(null);
    }
  }

  function renderRow(view: ResolvedFormView, opts: { editable: boolean }) {
    const key = `${view.origin}:${view.id}`;
    const selected = isSelected(view, selectedView);
    const busy = busyViewId === key;
    const isRenaming = opts.editable && renamingId === view.id;
    // Plan.md section 6/12: nonblocking "derived from unavailable view" notice -- the view
    // itself stays fully selectable/usable either way (see `isCustomViewBaseUnavailable`'s doc
    // comment); this only decides whether to show the informational text below.
    const baseUnavailable = isCustomViewBaseUnavailable(view, availableViews);

    return (
      <li key={key} className={styles.row} data-selected={selected}>
        <div className={styles.rowMain}>
          {isRenaming ? (
            <div className={styles.renameRow}>
              <input
                className={styles.renameInput}
                value={renameDraft}
                onChange={(e) => setRenameDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void commitRename(view.id);
                  if (e.key === "Escape") setRenamingId(null);
                }}
                autoFocus
                aria-label={`Rename ${view.label}`}
              />
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => void commitRename(view.id)}
                aria-label="Confirm rename"
                disabled={busy}
              >
                <Check size={13} />
              </button>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => setRenamingId(null)}
                aria-label="Cancel rename"
                disabled={busy}
              >
                <X size={13} />
              </button>
            </div>
          ) : (
            <>
              <span className={styles.rowLabel}>
                {view.label}
                {view.recommended ? <span className={styles.badge}>Recommended</span> : null}
                {selected ? <span className={styles.badge}>Selected</span> : null}
              </span>
              {view.description && <span className={styles.rowDescription}>{view.description}</span>}
              <span className={styles.rowSource}>{sourceText(view)}</span>
              {baseUnavailable && (
                <span className={styles.rowBaseWarning}>
                  Derived from a view that&apos;s no longer available -- this custom view is
                  unaffected and remains fully usable.
                </span>
              )}
            </>
          )}
        </div>

        {!isRenaming && (
          <div className={styles.rowActions}>
            <button
              type="button"
              className={styles.rowActionBtn}
              onClick={() => requestSwitch({ origin: view.origin, id: view.id })}
              disabled={selected}
            >
              Use this view
            </button>
            <button
              type="button"
              className={styles.rowActionBtn}
              onClick={() => void handleDuplicate(view)}
              disabled={busy}
            >
              Duplicate
            </button>
            {opts.editable && (
              <>
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => startRename(view)}
                  aria-label={`Rename ${view.label}`}
                  disabled={busy}
                >
                  <Pencil size={13} />
                </button>
                <button
                  type="button"
                  className={styles.rowActionBtnDanger}
                  onClick={() => void handleDelete(view)}
                  disabled={busy}
                >
                  Delete
                </button>
              </>
            )}
          </div>
        )}
      </li>
    );
  }

  return (
    <div className={styles.overlay} role="dialog" aria-modal="true" aria-label="Manage Form Views">
      <div className={styles.panel} ref={containerRef}>
        <div className={styles.header}>
          <span className={styles.title}>Manage Form Views</span>
          <button className={styles.closeBtn} onClick={requestClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          {controller.customViewsError && (
            <div className={styles.storeErrorBanner} role="alert">
              <p className={styles.storeErrorText}>
                Couldn&apos;t load your custom Form Views: {controller.customViewsError}. Your
                current view selection has been left untouched.
              </p>
              <div className={styles.storeErrorActions}>
                <button
                  type="button"
                  className={styles.rowActionBtn}
                  onClick={() => void handleRetryLoad()}
                  disabled={storeRecoveryBusy}
                >
                  {storeRecoveryBusy ? "Retrying…" : "Retry"}
                </button>
                <button
                  type="button"
                  className={styles.rowActionBtnDanger}
                  onClick={() => void handleResetStore()}
                  disabled={storeRecoveryBusy}
                >
                  Reset store
                </button>
              </div>
              {storeRecoveryMessage && (
                <p className={styles.storeErrorText}>{storeRecoveryMessage}</p>
              )}
            </div>
          )}
          {controller.customViewsWarning && (
            <p className={styles.warningBanner}>{controller.customViewsWarning.message}</p>
          )}
          {controller.persistWarning && (
            <p className={styles.errorBanner} role="alert">
              {controller.persistWarning}
            </p>
          )}
          {error && <p className={styles.errorBanner}>{error}</p>}

          <div className={styles.createRow}>
            <input
              className={styles.createInput}
              placeholder="New custom view name"
              value={newViewName}
              onChange={(e) => setNewViewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleCreate();
              }}
              aria-label="New custom view name"
            />
            <button
              type="button"
              className={styles.createBtn}
              onClick={() => void handleCreate()}
              disabled={creating || !newViewName.trim()}
            >
              {creating ? "Creating…" : "Create"}
            </button>
          </div>

          <ul className={styles.list} aria-label="Available form views">
            {defaultView && renderRow(defaultView, { editable: false })}
            {schemaViews.map((v) => renderRow(v, { editable: false }))}
            {customViews.map((v) => renderRow(v, { editable: true }))}
          </ul>

          {fieldChecklistTarget ? (
            <FormViewFieldChecklist
              controller={controller}
              def={fieldChecklistTarget.def}
              defSchema={fieldChecklistTarget.defSchema}
              catalog={fieldChecklistTarget.catalog}
            />
          ) : (
            <div className={styles.customizePlaceholder}>
              Field-by-field visibility customization isn&apos;t available for the current
              selection -- use Duplicate/Create above to prepare a custom view instead.
            </div>
          )}
        </div>

        <div className={styles.footer}>
          <span className={styles.spacer} />
          <button className={styles.closeAction} onClick={requestClose}>
            Close
          </button>
        </div>
      </div>

      {switchConfirmDialog}
      {closeConfirmPending && (
        <FormViewSwitchConfirmDialog
          hiddenCount={controller.hiddenCount}
          onCancel={() => setCloseConfirmPending(false)}
          onDiscardAndSwitch={() => {
            controller.resetOverride();
            setCloseConfirmPending(false);
            onClose();
          }}
          onSaveAsCustom={async (name) => {
            // Same staleness-guard idiom as `useViewSwitchConfirmation`'s own `onSaveAsCustom`
            // (see its doc comment): `saveOverrideAsCustomView` internally guards its own
            // auto-select side effect, but closing the dialog here is this component's OWN
            // chained side effect on top of that, so it needs its own scope check too. Checked
            // BEFORE touching any local state (not after clearing `closeConfirmPending`): this
            // component instance is not remounted on a Def switch, so a stale completion from a
            // save started under a DIFFERENT (now-abandoned) scope must never dismiss whatever
            // close-confirmation dialog belongs to the scope that's actually active now.
            const startScopeKey = controller.getScopeKey();
            await controller.saveOverrideAsCustomView(name);
            if (controller.getScopeKey() !== startScopeKey) return;
            setCloseConfirmPending(false);
            onClose();
          }}
        />
      )}
    </div>
  );
}
