import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { EyeOff } from "lucide-react";
import type { DefEditorView } from "../../types/xmlDocument";
import type { XmlEditorSnapshot } from "../../types/editorSession";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import type {
  FormFieldModel,
  FormSectionDefaults,
  FormValue,
} from "../../types/editorForm";
import type { FormFieldStore } from "../../lib/formFieldStore";
import { useXmlEditorContext } from "../../context/XmlEditorContext";
import { FormFieldControl, fieldInputDomId } from "../FormFieldControl/FormFieldControl";
import { UnknownXmlFields } from "../UnknownXmlFields/UnknownXmlFields";
import { GraphicDataPreview } from "../GraphicDataPreview/GraphicDataPreview";
import type { UseFormViewsResult } from "../../../form-views/hooks/useFormViews";
import {
  FormViewSelector,
  FORM_VIEW_SELECTOR_SELECT_ID,
} from "../../../form-views/components/FormViewSelector/FormViewSelector";
import {
  FormViewManagerDialog,
  type FormViewFieldChecklistTarget,
} from "../../../form-views/components/FormViewManagerDialog/FormViewManagerDialog";
import { toggleHiddenFieldId } from "../../../form-views/lib/resolveFormViews";
import { computeHiddenFieldDiagnosticsSummary } from "../../../form-views/lib/hiddenFieldDiagnostics";
import styles from "./XmlFormEditor.module.css";

function formValueText(value: FormValue): string {
  switch (value.kind) {
    case "boolean":
      return value.value ? "true" : "false";
    case "enum":
    case "readonly":
    case "scalar":
      return value.value;
    case "list":
      return value.items.join("\n");
    case "flags":
      return [...value.selected, ...value.custom].join("\n");
    case "namedMap":
      return value.entries.map((e) => `${e.key}=${e.value}`).join("\n");
    case "typedReferenceList":
      return value.items.map((i) => `${i.defType}:${i.defName}`).join("\n");
    case "objectList":
      return `(${value.items.length} item${value.items.length === 1 ? "" : "s"})`;
  }
}

function getFieldSectionPath(model: FormFieldModel): string[] | null {
  const path = model.path;
  switch (path.kind) {
    case "nestedObjectField":
    case "nestedListItems":
    case "nestedAttribute":
      return path.objectPath;
    case "namedMap":
    case "objectList":
      return path.objectPath.length > 0 ? path.objectPath : null;
    default:
      return null;
  }
}

function getSectionMeta(
  model: FormFieldModel,
  sectionPath: string[],
): FormSectionDefaults | undefined {
  const key = sectionPath.join(".");
  return model.sectionDefaults.find((s) => s.path.join(".") === key);
}

interface Props {
  snapshot: XmlEditorSnapshot;
  selectedDefNodeId: number | null;
  onSelectDef: (nodeId: number | null) => Promise<void>;
  formApi: XmlFormApi;
  /** Form Views (issue 06) selection/resolution controller. `null`/`undefined` (or
   * `applicable: false`) renders no Form View controls at all -- Patch/About/raw editors never
   * pass one, and a Def with no resolvable schema resolves `applicable: false` on its own. */
  formViews?: UseFormViewsResult | null;
}

function formatSectionLabel(objectPath: string[]): string {
  return objectPath
    .map((part) =>
      part.replace(/([A-Z])/g, " $1").replace(/^[a-z]/, (c) => c.toUpperCase()),
    )
    .join(" / ");
}

function sectionStateKey(defNodeId: number, sectionPath: string[]): string {
  return `${defNodeId}:${sectionPath.join(".")}`;
}

/** Stable DOM id for a section's collapsible header button, derived from its `sectionStateKey`.
 * Shared by the header's own `id` and the issue 08 reveal-focus fallback below so the two never
 * drift apart. */
function sectionHeaderDomId(stateKey: string): string {
  return `section-header-${stateKey.replace(/[^a-z0-9]/gi, "-")}`;
}

/**
 * Finds a descendant of `container` with the exact given `id`, scoped strictly to that subtree.
 *
 * Deliberately uses an `[id="..."]` ATTRIBUTE selector rather than a `#id` ID selector. The two
 * are not interchangeable here: `id` selectors are specified/optimized around the assumption
 * that an id is unique in the whole document, and multiple engines (confirmed: jsdom's `nwsapi`,
 * used by this project's Vitest suite) take a `document`-wide `getElementById`-style fast path
 * for a SCOPED `element.querySelector('#id')` call whenever that id exists anywhere in the
 * document - silently returning null (or a WRONG element) when the actual descendant match isn't
 * the first same-id element in document order. An attribute selector has no such fast path and
 * always walks the real subtree, so it stays correct even when the same id legitimately appears
 * more than once in the document - exactly the situation this function exists to handle (see
 * `focusFieldInput`'s doc comment below).
 */
function findScopedById(container: HTMLElement, id: string): HTMLElement | null {
  return container.querySelector<HTMLElement>(`[id="${id}"]`);
}

/**
 * Focuses and scrolls to a field's primary input by canonical `FormFieldId`, if it is currently
 * mounted. Returns whether an element was found (issue 08's reveal-focus flow uses this to decide
 * whether to fall back to focusing the containing section's header instead).
 *
 * Deliberately scoped to `container` (this `XmlFormEditor` instance's own root DOM node) rather
 * than a global `document.getElementById` lookup: `EditorWorkspace` keeps every open tab's
 * `XmlEditorPane`/`XmlFormEditor` mounted (hidden, not unmounted) across tab switches (Plan.md
 * section 9), and the field DOM id derived from `FormFieldId` is NOT guaranteed globally unique
 * across panes - two open tabs on Defs that share a `defType`/`defName` (or both land on the same
 * node id) produce colliding ids. A global lookup would return whichever pane's element happens
 * to appear first in document order, regardless of which tab is actually active - silently
 * focusing a hidden, inactive tab's field instead of the one the user is looking at. Scoping the
 * lookup to this instance's own subtree (via `findScopedById`, not a raw `#id` selector) makes
 * that impossible even when ids collide elsewhere.
 */
function focusFieldInput(container: HTMLElement | null, fieldId: string): boolean {
  if (!container) return false;
  const el = findScopedById(container, fieldInputDomId(fieldId));
  if (!el) return false;
  el.focus();
  el.scrollIntoView?.({ block: "center" });
  return true;
}

/** Focuses and scrolls to a section header button by its `sectionStateKey`, if mounted - the
 * issue 08 reveal-focus fallback for when no focusable input exists for the revealed model even
 * after its section was force-expanded. Scoped to `container` for the same cross-pane reason as
 * `focusFieldInput` above. */
function focusSectionHeader(container: HTMLElement | null, stateKey: string): void {
  if (!container) return;
  const el = findScopedById(container, sectionHeaderDomId(stateKey));
  if (!el) return;
  el.focus();
  el.scrollIntoView?.({ block: "center" });
}

function computeInitialCollapsed(
  defaultCollapsed: boolean | undefined,
  hasData: boolean | undefined,
): boolean {
  if (defaultCollapsed !== undefined) return defaultCollapsed;
  // Only collapse when we explicitly know there is no data.
  // Undefined means metadata wasn't set (e.g. directly-constructed test fields) - treat as open.
  if (hasData === false) return true;
  return false;
}

/** Finds the value of a graphicData child field by subscribing to it from the store. */
function useGraphicDataFieldValue(
  store: FormFieldStore,
  models: FormFieldModel[],
  fieldName: string,
): string {
  const model = models.find(
    (m) =>
      m.path.kind === "nestedObjectField" &&
      m.path.objectPath.length === 1 &&
      m.path.objectPath[0] === "graphicData" &&
      m.path.fieldName === fieldName,
  );
  const id = model?.id ?? null;
  const subscribe = useCallback(
    (cb: () => void) => (id ? store.subscribeField(id, cb) : () => undefined),
    [store, id],
  );
  const getSnapshot = useCallback(
    () => (id ? store.getFieldState(id) : undefined),
    [store, id],
  );
  const field = useSyncExternalStore(subscribe, getSnapshot);
  return field ? formValueText(field.value) : "";
}

/** Live graphicData preview - re-renders only when its three source fields change. */
function GraphicDataPreviewConnected({
  store,
  models,
  projectId,
}: {
  store: FormFieldStore;
  models: FormFieldModel[];
  projectId?: string;
}) {
  const texPath = useGraphicDataFieldValue(store, models, "texPath");
  const graphicClass = useGraphicDataFieldValue(store, models, "graphicClass");
  const maskPath = useGraphicDataFieldValue(store, models, "maskPath");
  return (
    <GraphicDataPreview
      projectId={projectId}
      texPath={texPath}
      graphicClass={graphicClass}
      maskPath={maskPath || undefined}
    />
  );
}

export const XmlFormEditor = React.memo(function XmlFormEditor({
  snapshot,
  selectedDefNodeId,
  onSelectDef,
  formApi,
  formViews,
}: Props) {
  const { projectId, catalog } = useXmlEditorContext();
  const { parsed } = snapshot;
  const store = formApi.store;

  // Stable ordered model list - changes only on a structural rebuild, not on value edits.
  const models = useSyncExternalStore(
    store.subscribeStructure,
    store.getModels,
  );

  // Tracks explicit section toggle overrides, keyed by `{defNodeId}:{sectionPath}`.
  // Sections without an entry fall back to the computed default from schema/data metadata.
  const [explicitCollapsed, setExplicitCollapsed] = useState<
    Record<string, boolean>
  >({});

  // Form Views (issue 06): "Customize view" opens this manager overlay. Local to the form
  // editor, not the controller -- it's pure UI open/closed state, not selection/override state.
  const [managerOpen, setManagerOpen] = useState(false);

  // Hoisted above the `!parsed`/no-Def early return below (rather than after it, as the rest of
  // this function's plain derived values are) because issue 08's hooks (`useMemo`/`useEffect`
  // just below) need it, and every hook in this component must run unconditionally on every
  // render regardless of which branch `parsed`/`selectedDefNodeId` puts this render in -
  // otherwise a render that flips between "has a Def" and "has none" for the SAME mounted
  // instance would change the hook count/order, which React disallows. `selectedDef` stays a
  // plain (non-hook) `const` computation; TypeScript still narrows it to non-null after the
  // `!selectedDef` check below since it is never reassigned.
  const selectedDef: DefEditorView | null =
    parsed && parsed.defs.length > 0
      ? (parsed.defs.find((d) => d.nodeId === selectedDefNodeId) ?? parsed.defs[0])
      : null;
  // Resolved alongside `selectedDef` (rather than after the `!selectedDef` early return below,
  // as the rest of this function's plain derived values are) because the reveal-focus effects
  // just below need a plain `number` to build a `sectionStateKey` - TypeScript does not carry
  // `selectedDef`'s narrowing into the nested `renderSectionContent` closure later in this
  // function either way (see that closure's own comment), so this single hoisted value serves
  // both. The `-1` fallback is never actually consulted: the effects below only reach a
  // `sectionStateKey` call after finding a matching model in `models`, which is only non-empty
  // for a real selected Def.
  const selectedDefId = selectedDef?.nodeId ?? -1;

  // Issue 08 (Plan.md section 8 "Hidden validation feedback"): validation diagnostics for the
  // selected Def that map to a currently-hidden top-level root. `null` whenever Form Views don't
  // apply at all (Patch/About/raw never pass a controller; a Def with no resolvable schema
  // resolves `applicable: false` on its own) or there is no selected Def yet.
  const hiddenIssues = useMemo(() => {
    if (!formViews?.applicable || !selectedDef) return null;
    return computeHiddenFieldDiagnosticsSummary({
      diagnostics: snapshot.validationDiagnostics,
      def: selectedDef,
      effectiveHidden: formViews.effectiveHidden,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [formViews?.applicable, formViews?.effectiveHidden, snapshot.validationDiagnostics, selectedDef]);

  // Focus-after-reveal (Plan.md section 8: "focuses/scrolls to the first rendered field"). Set
  // by `handleReveal` just below to the exact root ids it just unhid; consumed by the effect
  // beneath it the next time `models` gets a new structural identity (i.e. once the store has
  // actually rebuilt with those roots' fields present - `models` comes from
  // `store.subscribeStructure`/`getModels`, which only changes identity on a structural rebuild,
  // not a plain value edit). A ref (not state) because setting it must never itself trigger a
  // render - the render that matters is the one already coming from `setOverrideHiddenFieldIds`.
  const pendingFocusRootsRef = useRef<ReadonlySet<string> | null>(null);

  // Second phase of the same flow, needed when the revealed field lives inside an object-root
  // section that defaults to COLLAPSED (`computeInitialCollapsed`: an object root with no
  // existing data collapses by default - a very plausible case for a field the user never
  // populated, which is exactly why it had a "missing required field" diagnostic in the first
  // place). A collapsed section's inner inputs are never mounted (`{!collapsed && (...)}`
  // below), so attempting to focus them immediately after the `models` rebuild would silently
  // find nothing. `pendingFocusFieldIdRef`/`pendingFocusHeaderKeyRef` carry the still-pending
  // focus target across the `explicitCollapsed` state update the first effect issues to force
  // those sections open; the second effect (keyed on `explicitCollapsed`) runs once that state
  // change has actually re-rendered the section as expanded and the target input is mounted.
  const pendingFocusFieldIdRef = useRef<string | null>(null);
  const pendingFocusHeaderKeyRef = useRef<string | null>(null);

  // This instance's own root DOM node - every reveal-focus DOM lookup is scoped to it (never a
  // global `document.getElementById`/`querySelector`) so a colliding field/section DOM id in a
  // different, hidden-but-still-mounted tab (`EditorWorkspace` keeps every open tab's pane
  // mounted, per Plan.md section 9) can never steal focus away from the pane the user is
  // actually looking at. See `focusFieldInput`'s doc comment above for the full rationale.
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const targets = pendingFocusRootsRef.current;
    if (!targets || targets.size === 0) return;
    pendingFocusRootsRef.current = null;
    // First model (in existing schema/render order) whose top-level root is one of the just-
    // revealed roots - "the first rendered field", not necessarily the first field of whichever
    // root happened to be listed first in the affected set.
    const target = models.find((m) => targets.has(m.fieldPath[0]));
    if (!target) {
      // Reachable for a structurally EMPTY object field - zero schema-declared child fields and
      // no discriminator (e.g. `ThingDef.colorGenerator`/`colorGeneratorInTraderStock`, whose
      // `ColorGenerator` object-type schema has `"fields": {}` and no variants).
      // `buildFormDescriptors` produces zero models - not even a section header - for such a
      // field regardless of Form Views, so there is genuinely nothing in the DOM for THIS
      // field to focus. Fall back to the Form View selector control instead of silently doing
      // nothing - the same fallback issue 05/06 already established for "the field that had
      // focus just disappeared" (see `XmlEditorPane`'s `onFocusedFieldHidden`). Scoped to this
      // pane's own `containerRef` (via `findScopedById`, not a global lookup or a raw `#id`
      // selector) for the same cross-pane reason as `focusFieldInput` above -
      // `FORM_VIEW_SELECTOR_SELECT_ID` is a fixed constant rendered once per mounted pane, so an
      // unscoped (or `#id`-selector-optimized) lookup could equally focus a different tab's
      // selector.
      if (containerRef.current) {
        findScopedById(containerRef.current, FORM_VIEW_SELECTOR_SELECT_ID)?.focus();
      }
      return;
    }

    const sectionPath = getFieldSectionPath(target);
    if (sectionPath && sectionPath.length > 0) {
      // Force every ancestor section level open (not just the top-level root) so a field nested
      // inside a collapsed sub-subsection is reachable too, e.g. `graphicData.shadowData.volume`
      // when `shadowData` itself defaults to collapsed. Overwrites any prior explicit collapse
      // choice for these exact sections - a deliberate consequence of "reveal and focus", not a
      // side effect to work around.
      const updates: Record<string, boolean> = {};
      for (let depth = 1; depth <= sectionPath.length; depth++) {
        updates[sectionStateKey(selectedDefId, sectionPath.slice(0, depth))] = false;
      }
      setExplicitCollapsed((prev) => ({ ...prev, ...updates }));
      pendingFocusFieldIdRef.current = target.id;
      pendingFocusHeaderKeyRef.current = sectionStateKey(selectedDefId, sectionPath.slice(0, 1));
      return; // Focus happens in the follow-up effect once the section(s) render open.
    }

    // Not nested in any section (a plain top-level scalar/list/map/objectList root) - nothing
    // can be collapsed, so the field's DOM anchor (a real input for a scalar control, or the
    // control's own root container id for a row-based editor like `ObjectListEditor`/
    // `ListEditor`/`NamedMapEditor`/`ReferenceListEditor`/`TypedReferenceListEditor` when it has
    // zero rows - see each of those components' own `containerId`/`id` doc comments) is already
    // mounted right now.
    if (!focusFieldInput(containerRef.current, target.id) && containerRef.current) {
      // Defensive fallback for any remaining control kind that renders no focusable
      // `field-${id}` element at all (e.g. a genuinely-unresolvable "object" placeholder with no
      // schema-backed control) - same Form View selector fallback as the "no model at all"
      // branch above, so reveal never leaves focus stuck with nowhere to go.
      findScopedById(containerRef.current, FORM_VIEW_SELECTOR_SELECT_ID)?.focus();
    }
  }, [models]);

  useEffect(() => {
    const fieldId = pendingFocusFieldIdRef.current;
    const headerKey = pendingFocusHeaderKeyRef.current;
    if (!fieldId && !headerKey) return;
    pendingFocusFieldIdRef.current = null;
    pendingFocusHeaderKeyRef.current = null;

    if (fieldId && focusFieldInput(containerRef.current, fieldId)) return;
    // Fallback: no focusable input was found for this exact model even after expanding its
    // section (e.g. an object shape with no schema-backed input at all for that field) - focus/
    // scroll to the section header instead of silently doing nothing, so the user still lands
    // somewhere meaningful.
    if (headerKey) focusSectionHeader(containerRef.current, headerKey);
  }, [explicitCollapsed]);

  const handleReveal = useCallback(() => {
    if (!formViews || !hiddenIssues || hiddenIssues.affectedRootIds.size === 0) return;
    // Plan.md section 8: "adds a temporary override that unhides only affected top-level roots,
    // leaves all other view rules intact" - computed as the CURRENT effective hidden set minus
    // only the affected roots, then applied through the exact same override mechanism issue 07's
    // checkboxes use (`setOverrideHiddenFieldIds`). Never touches Custom View storage, never
    // changes the selected view, never triggers the switch-confirmation prompt - identical
    // semantics to an unhide checkbox toggle.
    const nextHidden = new Set(formViews.effectiveHidden);
    for (const rootId of hiddenIssues.affectedRootIds) nextHidden.delete(rootId);
    pendingFocusRootsRef.current = hiddenIssues.affectedRootIds;
    formViews.setOverrideHiddenFieldIds(nextHidden);
  }, [formViews, hiddenIssues]);

  if (!parsed || !selectedDef) {
    return (
      <div className={styles.empty}>
        <p>No Def found in this file.</p>
      </div>
    );
  }

  const knownModels = models.filter((m) => m.control !== "readonlyUnknown");
  const unknownModels = models.filter((m) => m.control === "readonlyUnknown");
  const unknownFields = unknownModels
    .map((m) => store.getFieldState(m.id))
    .filter((f): f is NonNullable<typeof f> => !!f);

  // Form Views (issue 07): the checklist inside `FormViewManagerDialog` needs the FULL
  // canonical top-level schema field universe, not `models` -- a hidden field's models never
  // reach this component at all (issue 05 skips them upstream), so `fieldChecklistTarget` gives
  // the dialog everything it needs (schema, catalog, live XML) to build that list itself.
  const defSchema = catalog?.defTypes[selectedDef.defType] ?? null;
  const fieldChecklistTarget: FormViewFieldChecklistTarget | null =
    catalog && defSchema ? { def: selectedDef, defSchema, catalog } : null;

  // "Customize mode" (Plan.md section 8: "Hide/show affordances in the form itself appear only
  // in customize mode") is exactly "the manager dialog is open" -- entering customize mode opens
  // the dialog, per Plan.md section 8's lifecycle, so there is no separate mode flag to track.
  const customizeModeActive = managerOpen && !!formViews?.applicable;

  function toggleFieldHidden(fieldId: string) {
    if (!formViews) return;
    // Same shared pure primitive `FormViewFieldChecklist`'s checkboxes use, applied to the same
    // `effectiveHidden` baseline -- one toggle mechanism, not two that could drift apart.
    formViews.setOverrideHiddenFieldIds(toggleHiddenFieldId(formViews.effectiveHidden, fieldId));
  }

  function isSectionCollapsedForPath(
    stateKey: string,
    meta: FormSectionDefaults | undefined,
  ): boolean {
    if (Object.prototype.hasOwnProperty.call(explicitCollapsed, stateKey)) {
      return explicitCollapsed[stateKey];
    }
    return computeInitialCollapsed(meta?.defaultCollapsed, meta?.hasData);
  }

  function toggleSection(key: string, currentlyCollapsed: boolean) {
    setExplicitCollapsed((prev) => ({ ...prev, [key]: !currentlyCollapsed }));
  }

  function renderSectionContent(
    sectionModels: FormFieldModel[],
    parentPath: string[],
  ): React.ReactNode[] {
    const nodes: React.ReactNode[] = [];
    const depth = parentPath.length;
    let i = 0;

    while (i < sectionModels.length) {
      const model = sectionModels[i];
      const sectionPath = getFieldSectionPath(model);

      if (!sectionPath || sectionPath.length <= depth) {
        // Inline hide affordance (issue 07, Plan.md section 8: "on top-level scalar labels...")
        // -- only for a genuinely top-level field (`depth === 0`, single-segment `fieldPath`),
        // and only while customize mode is active, so ordinary form rendering is byte-identical
        // to before this issue.
        const topLevelFieldId =
          depth === 0 && model.fieldPath.length === 1 ? model.fieldPath[0] : null;
        const control = (
          <FormFieldControl
            key={model.id}
            fieldId={model.id}
            store={store}
            formApi={formApi}
            nestedDepth={depth}
          />
        );
        if (customizeModeActive && topLevelFieldId) {
          nodes.push(
            <div key={`customize-${model.id}`} className={styles.customizeFieldRow}>
              <div className={styles.customizeFieldControl}>{control}</div>
              <button
                type="button"
                className={styles.hideFieldBtn}
                onClick={() => toggleFieldHidden(topLevelFieldId)}
                aria-label={`Hide ${model.label}`}
                title="Hide this field (Customize view)"
              >
                <EyeOff size={12} />
              </button>
            </div>,
          );
        } else {
          nodes.push(control);
        }
        i++;
      } else {
        const subPath = sectionPath.slice(0, depth + 1);
        const subPathKey = subPath.join(".");

        const subModels: FormFieldModel[] = [];
        while (i < sectionModels.length) {
          const m = sectionModels[i];
          const fp = getFieldSectionPath(m);
          if (
            fp &&
            fp.length > depth &&
            fp.slice(0, depth + 1).join(".") === subPathKey
          ) {
            subModels.push(m);
            i++;
          } else {
            break;
          }
        }

        const stateKey = sectionStateKey(selectedDefId, subPath);
        const meta = getSectionMeta(subModels[0], subPath);
        const collapsed = isSectionCollapsedForPath(stateKey, meta);
        const safeKey = stateKey.replace(/[^a-z0-9]/gi, "-");
        const headerId = sectionHeaderDomId(stateKey);
        const sectionId = `section-content-${safeKey}`;
        // Inline hide affordance for object-section headers (issue 07, Plan.md section 8:
        // "...and object-section headers" / "object section header controls operate on its root
        // ID") -- only the section's TOP-LEVEL root (`subPath.length === 1`) is hideable; a
        // nested subsection inside an object is not independently hideable (non-goal: nested
        // member/list-row hiding).
        const topLevelSectionFieldId = subPath.length === 1 ? subPath[0] : null;

        const headerButton = (
          <button
            key={`header-${stateKey}`}
            id={headerId}
            className={styles.nestedSectionHeader}
            onClick={() => toggleSection(stateKey, collapsed)}
            aria-expanded={!collapsed}
            aria-controls={sectionId}
          >
            <span aria-hidden="true" className={styles.sectionToggleIcon}>
              {collapsed ? "▶" : "▼"}
            </span>
            <span>{formatSectionLabel(subPath)}</span>
          </button>
        );

        if (customizeModeActive && topLevelSectionFieldId) {
          nodes.push(
            <div key={`header-wrap-${stateKey}`} className={styles.sectionHeaderRow}>
              {headerButton}
              <button
                type="button"
                className={styles.hideFieldBtn}
                onClick={() => toggleFieldHidden(topLevelSectionFieldId)}
                aria-label={`Hide ${formatSectionLabel(subPath)}`}
                title="Hide this section (Customize view)"
              >
                <EyeOff size={12} />
              </button>
            </div>,
          );
        } else {
          nodes.push(headerButton);
        }

        nodes.push(
          <div key={`content-${stateKey}`} id={sectionId}>
            {!collapsed && (
              <>
                {subPathKey === "graphicData" && (
                  <GraphicDataPreviewConnected
                    store={store}
                    models={models}
                    projectId={projectId}
                  />
                )}
                {renderSectionContent(subModels, subPath)}
              </>
            )}
          </div>,
        );
      }
    }

    return nodes;
  }

  const fieldNodes = renderSectionContent(knownModels, []);

  return (
    <div className={styles.root} ref={containerRef}>
      {parsed.defs.length > 1 && (
        <div className={styles.defSelector}>
          <label htmlFor="def-selector" className={styles.defSelectorLabel}>
            Def
          </label>
          <select
            id="def-selector"
            className={styles.defSelectorInput}
            value={selectedDef.nodeId}
            onChange={(e) => void onSelectDef(Number(e.target.value))}
          >
            {parsed.defs.map((d) => (
              <option key={d.nodeId} value={d.nodeId}>
                {d.defName ?? d.defType} ({d.defType})
              </option>
            ))}
          </select>
        </div>
      )}

      {formViews?.applicable && (
        <FormViewSelector
          controller={formViews}
          onOpenManager={() => setManagerOpen(true)}
          hiddenIssues={hiddenIssues}
          onReveal={handleReveal}
        />
      )}

      <div className={styles.fields}>
        {formApi.formError && (
          <p className={styles.formError}>{formApi.formError}</p>
        )}

        {knownModels.length === 0 && unknownModels.length === 0 && (
          <p className={styles.noFields}>
            No schema available for <strong>{selectedDef.defType}</strong>.
          </p>
        )}

        {fieldNodes}

        <UnknownXmlFields fields={unknownFields} />
      </div>

      {managerOpen && formViews?.applicable && (
        <FormViewManagerDialog
          controller={formViews}
          onClose={() => setManagerOpen(false)}
          fieldChecklistTarget={fieldChecklistTarget}
        />
      )}
    </div>
  );
});
