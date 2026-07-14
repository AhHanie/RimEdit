// Pure Form View resolution logic (Plan.md section 7's `available`/`selected`/`effectiveHidden`
// algorithm). No React, no Tauri `invoke`, no XML/session state -- `useFormViews` is the only
// caller that wires this to live catalog/store/session data. Kept pure and colocated with its
// own test file so the resolver contract (ordering, fallback, visibility intersection) is
// independently verifiable without mounting hooks/components.
import type { SchemaFormView } from "../../schema-catalog";
import type { CustomFormView, SelectedFormViewRef } from "../types/formViews";
import {
  DEFAULT_FORM_VIEW_ID,
  type FieldVisibilityOverride,
  type ResolvedFormView,
} from "../types/resolvedFormView";

/** The immutable, always-selectable full form. Empty hidden set by construction (Plan.md
 * section 7: "Default View has an empty hidden set"). */
export function buildDefaultFormView(targetDefType: string): ResolvedFormView {
  return {
    id: DEFAULT_FORM_VIEW_ID,
    targetDefType,
    label: "Default View",
    order: Number.NEGATIVE_INFINITY,
    origin: "default",
    hiddenFieldIds: [],
    recommended: false,
  };
}

/**
 * Combines `[Default View, resolved schema views for defType, custom views for
 * project/game/defType]` into one ordered selectable list (Plan.md section 7/8): Default first,
 * then schema views ordered by `order` (ties broken by `recommended` then label), then custom
 * views ordered by creation time (Plan.md section 6: "Order is creation/updated time initially").
 */
export function buildAvailableFormViews(
  targetDefType: string,
  schemaFormViews: Record<string, SchemaFormView> | undefined,
  customViews: readonly CustomFormView[],
): ResolvedFormView[] {
  const schemaViews: ResolvedFormView[] = Object.values(schemaFormViews ?? {})
    .slice()
    .sort((a, b) => {
      if (a.order !== b.order) return a.order - b.order;
      if (a.recommended !== b.recommended) return a.recommended ? -1 : 1;
      return a.label.localeCompare(b.label);
    })
    .map((v) => ({
      id: v.id,
      targetDefType,
      label: v.label,
      description: v.description,
      icon: v.icon,
      order: v.order,
      origin: "schema",
      hiddenFieldIds: v.hiddenFieldIds,
      recommended: v.recommended,
      declaredOnDefType: v.declaredOnDefType,
      source: v.source,
    }));

  const customSorted: ResolvedFormView[] = customViews
    .slice()
    .sort((a, b) => a.createdAt.localeCompare(b.createdAt))
    .map((v, index) => ({
      id: v.id,
      targetDefType,
      label: v.name,
      description: v.description ?? undefined,
      order: schemaViews.length + index,
      origin: "custom",
      hiddenFieldIds: v.hiddenFieldIds,
      recommended: false,
      baseSchemaView: v.baseSchemaView ?? undefined,
    }));

  return [buildDefaultFormView(targetDefType), ...schemaViews, ...customSorted];
}

/**
 * Plan.md section 7: `selected = valid persisted/current selection ? it : recommended schema
 * view ?? Default`. `available` is assumed to always contain the Default View (guaranteed by
 * `buildAvailableFormViews`); the final `available[0]` fallback only guards a malformed caller.
 */
export function resolveSelectedFormView(
  available: readonly ResolvedFormView[],
  persisted: SelectedFormViewRef | null,
): ResolvedFormView {
  if (persisted) {
    const byRef = available.find(
      (v) => v.origin === persisted.origin && v.id === persisted.id,
    );
    if (byRef) return byRef;
  }
  const recommended = available.find((v) => v.origin === "schema" && v.recommended);
  if (recommended) return recommended;
  const defaultView = available.find((v) => v.origin === "default");
  return defaultView ?? available[0];
}

/**
 * Plan.md section 7's `effectiveHidden`/`visibleTopLevel` computation. `knownTopLevel` must be
 * the same canonical top-level field universe `buildFormDescriptors` renders from
 * (`collectEffectiveTopLevelDefFields`), so a stale/removed field reference in a persisted
 * custom view or a schema view amendment can never leak into the live visible set.
 *
 * Returns `visibleTopLevelFieldIds: null` whenever the effective hidden set is empty, matching
 * `useXmlFormController`'s "undefined/null means no filter, byte-identical to the full form"
 * contract (issue 05) exactly -- selecting Default View with no override must behave identically
 * to today's unfiltered form, not merely compute an equivalent "hide nothing" Set.
 */
export function computeEffectiveVisibility(args: {
  selected: ResolvedFormView;
  override: FieldVisibilityOverride | null;
  knownTopLevel: ReadonlySet<string>;
}): { effectiveHidden: Set<string>; visibleTopLevelFieldIds: Set<string> | null } {
  const { selected, override, knownTopLevel } = args;
  const baseHidden = override ? override.hiddenFieldIds : selected.hiddenFieldIds;

  const effectiveHidden = new Set<string>();
  for (const id of baseHidden) {
    if (knownTopLevel.has(id)) effectiveHidden.add(id);
  }

  if (effectiveHidden.size === 0) {
    return { effectiveHidden, visibleTopLevelFieldIds: null };
  }

  const visibleTopLevelFieldIds = new Set<string>();
  for (const id of knownTopLevel) {
    if (!effectiveHidden.has(id)) visibleTopLevelFieldIds.add(id);
  }
  return { effectiveHidden, visibleTopLevelFieldIds };
}

/**
 * Issue 07's single toggle primitive: flips one field id's membership in a hidden-id set,
 * returning a fresh `Set` (never mutates `hidden` in place). Both `FormViewFieldChecklist`'s
 * per-row checkboxes AND `XmlFormEditor`'s inline in-form hide buttons call this exact function
 * against `controller.effectiveHidden` before handing the result to
 * `setOverrideHiddenFieldIds` -- one shared pure primitive, not two divergent toggle
 * implementations that could drift apart (Plan.md section 8: the inline affordances and the
 * checklist both mutate the same `FieldVisibilityOverride`).
 */
export function toggleHiddenFieldId(
  hidden: ReadonlySet<string>,
  fieldId: string,
): Set<string> {
  const next = new Set(hidden);
  if (next.has(fieldId)) {
    next.delete(fieldId);
  } else {
    next.add(fieldId);
  }
  return next;
}

/** Whether `hidden` differs from `selected`'s own resolved hidden set -- the `isDirty` flag on
 * `FieldVisibilityOverride` (Plan.md section 3). */
export function isHiddenSetDirty(
  hidden: ReadonlySet<string>,
  selected: ResolvedFormView,
): boolean {
  const base = selected.hiddenFieldIds;
  if (base.length !== hidden.size) return true;
  for (const id of base) {
    if (!hidden.has(id)) return true;
  }
  return false;
}

/**
 * Plan.md section 6/12: "A missing/renamed base becomes a nonblocking 'derived from
 * unavailable view' notice, not a broken view" / "Show unavailable-base/missing-field notices
 * in manager." A custom view's `baseSchemaView` is pure, never-re-derived provenance (Plan.md
 * section 6) -- it is never used to recompute `hiddenFieldIds`, so this check exists purely to
 * decide whether to show an informational notice, never to alter the view's own resolved
 * visibility (which stays exactly as materialized, per `computeEffectiveVisibility`, regardless
 * of the answer here).
 *
 * "Unavailable" means the base view id this custom view was originally duplicated/saved from no
 * longer resolves as a schema-defined view for the same target Def type -- covering both a
 * renamed id (the old id is simply gone) and a fully removed view. `availableViews` must be the
 * SAME list `resolveSelectedFormView` resolves against (i.e. `buildAvailableFormViews`'s output
 * for the view's own `targetDefType`), so this can never disagree with what the selector/manager
 * actually lists as selectable schema views.
 *
 * Returns `false` (never "unavailable") for a Default/schema view, or a custom view with no
 * recorded base at all -- there is nothing to have gone missing in either case.
 */
export function isCustomViewBaseUnavailable(
  view: ResolvedFormView,
  availableViews: readonly ResolvedFormView[],
): boolean {
  if (view.origin !== "custom" || !view.baseSchemaView) return false;
  const baseViewId = view.baseSchemaView.viewId;
  return !availableViews.some((v) => v.origin === "schema" && v.id === baseViewId);
}

/**
 * Stable per-Def-instance key so `useFormViews` can keep override/selection state isolated
 * across multiple Defs of the same type opened within one file/pane (Plan.md section 9).
 *
 * Includes `projectId`/`gameVersion`, not just `defType`/`ordinal`: custom views and the
 * persisted "last selected" preference are both scoped by `{project, gameVersion, defType}`
 * (Plan.md section 3/6). Omitting them here would let a game-version (or project) change reuse
 * a `loaded: true` cache entry from the OLD scope's `get_last_selected_form_view` fetch, so the
 * hook would never re-fetch the NEW scope's real preference before its fallback-reconciliation
 * logic ran -- misreading "haven't checked yet" as "checked, and it's gone" and re-persisting a
 * fallback into a scope whose real selection was never actually loaded.
 */
export function formViewsStateKey(
  projectId: string | null,
  gameVersion: string | null,
  defType: string,
  ordinal: number,
): string {
  return `${projectId ?? ""}::${gameVersion ?? ""}::${defType}::${ordinal}`;
}
