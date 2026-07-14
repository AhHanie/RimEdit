// Issue 08 (Plan.md section 8 "Hidden validation feedback"): pure logic mapping full-document
// `ValidationDiagnostic`s onto the currently selected Def's canonical top-level Form View roots,
// so a Form View header can report "N hidden field issue(s)" and offer `Reveal fields with
// issues`. No React, no Tauri `invoke` -- `XmlFormEditor` is the sole caller that wires this to
// live session/controller data (mirrors `resolveFormViews.ts`'s pure-logic/thin-wiring split).
//
// Constraint (Plan.md, issue 08 non-goals): this module never changes what Rust validates or how
// severe a diagnostic is -- it only *reads* the existing `fieldPath`/`nodeId`/`blocking` fields
// already produced by `xml_document/validation/*` and decides whether a diagnostic is (a) inside
// the currently selected Def instance and (b) currently hidden by the active Form View.
import type { DefEditorView, ValidationDiagnostic } from "../../xml-editor/types/xmlDocument";
import type { XmlListItemView, XmlNestedChildView } from "../../xml-editor/types/xmlDocument";

/**
 * Maps a `ValidationDiagnostic.fieldPath` to its canonical top-level Form View root id (Plan.md
 * section 8: "the first dot segment and strips list/map suffixes (`foo[0]`, `foo[key]`) to root
 * `foo`").
 *
 * Examples:
 * - `"foo"` -> `"foo"` (top-level scalar)
 * - `"graphicData.texPath"` -> `"graphicData"` (nested object field)
 * - `"comps[0].class"` -> `"comps"` (list index, including a bracket on the first segment)
 * - `"statBases[WorkToMake]"` -> `"statBases"` (keyed map, no dot at all)
 * - `"comps[0].nested[1].deep"` -> `"comps"` (multiple nesting levels -- only the first dot
 *   segment's bracket matters)
 *
 * Returns `null` for `null`/`undefined`/empty/whitespace-only input and for a malformed path
 * whose first segment has no field name at all (e.g. `"[0]"`, `"."`) -- these are exactly the
 * "no path or an unmapped/Def-level path" diagnostics the issue requires to stay in the bottom
 * panel rather than be claimed as "revealed" by anything this module computes. This function does
 * NOT validate the root against the actual known schema field universe -- that intersection
 * happens in `computeHiddenFieldDiagnosticsSummary` below (via `effectiveHidden`, which is itself
 * already intersected with `collectEffectiveTopLevelDefFields`), so a syntactically valid but
 * unknown/removed field id is naturally excluded there, not here.
 */
export function mapFieldPathToTopLevelRoot(
  fieldPath: string | null | undefined,
): string | null {
  if (!fieldPath) return null;
  const firstDotSegment = fieldPath.split(".")[0];
  const root = firstDotSegment.split("[")[0].trim();
  return root.length > 0 ? root : null;
}

/** Anything shaped enough to walk for node-id containment -- both `XmlChildView` (a Def's direct
 * children) and `XmlNestedChildView` (everything beneath) carry the same relevant shape. */
interface WalkableXmlNode {
  nodeId: number;
  children?: XmlNestedChildView[];
  liObjectItems?: XmlNestedChildView[][];
  liItems?: XmlListItemView[];
}

function collectNodeIds(node: WalkableXmlNode, ids: Set<number>): void {
  ids.add(node.nodeId);
  for (const nested of node.children ?? []) collectNodeIds(nested, ids);
  for (const group of node.liObjectItems ?? []) {
    for (const nested of group) collectNodeIds(nested, ids);
  }
  for (const li of node.liItems ?? []) {
    ids.add(li.nodeId);
    for (const nested of li.children) collectNodeIds(nested, ids);
  }
}

/**
 * Every `XmlNodeId` belonging to `def`'s own subtree: its own root node id plus every descendant
 * (direct children, nested object fields, list/map items, at any depth). Used to scope
 * document-wide diagnostics down to "belongs to the currently selected Def instance" via node id
 * containment (issue 08 step 2: "Filter session validation diagnostics to selected Def via node
 * ID where present") -- a diagnostic's `nodeId` is the specific field/element node it was raised
 * against (see `xml_document/validation/fields.rs`'s `error_at_node`/`warning_at_node`), not
 * necessarily the Def's own root node id, so a plain `nodeId === def.nodeId` equality check would
 * wrongly exclude every diagnostic raised against an existing field value (it only happens to
 * equal the Def's own node id for Def-root-level diagnostics like a missing required field or a
 * duplicate defName -- see `validate_required_fields_present`/`validate_def_identity`).
 */
export function collectDefSubtreeNodeIds(def: DefEditorView): ReadonlySet<number> {
  const ids = new Set<number>();
  ids.add(def.nodeId);
  for (const child of def.children) collectNodeIds(child, ids);
  return ids;
}

export interface HiddenFieldDiagnosticsSummary {
  /** Canonical top-level root ids that have at least one diagnostic hidden behind them --
   * exactly the set `Reveal fields with issues` unhides (Plan.md: "unhides only affected
   * top-level roots"). */
  affectedRootIds: ReadonlySet<string>;
  /** Total diagnostic count mapped to a currently-hidden root, for the selected Def only. */
  totalCount: number;
  /** Subset of `totalCount` whose `blocking` flag is true (Plan.md section 2/8: blocking vs.
   * non-blocking is already a first-class diagnostic property; the header surfaces it
   * separately rather than conflating it with plain severity). */
  blockingCount: number;
}

const EMPTY_SUMMARY: HiddenFieldDiagnosticsSummary = {
  affectedRootIds: new Set(),
  totalCount: 0,
  blockingCount: 0,
};

/**
 * The full "hidden validation feedback" computation (Plan.md section 8 / issue 08 steps 2-3):
 * full-document diagnostics -> scoped to the selected Def instance (via node id containment) ->
 * mapped to a canonical top-level root -> intersected with the currently *hidden* set. Never
 * mutates anything and never itself changes `effectiveHidden` -- purely a read/count, matching
 * the issue's "no auto-reveal" requirement (only an explicit caller-driven override change, in
 * response to a user clicking `Reveal fields with issues`, can ever change visibility).
 *
 * A diagnostic contributes to the summary only when ALL of:
 * 1. it belongs to the selected Def instance (`nodeId` inside `collectDefSubtreeNodeIds(def)`) --
 *    a diagnostic for a *different* Def of the same type (or a different node entirely) is never
 *    counted here, even if its `fieldPath` happens to name a root that's hidden on this Def;
 * 2. its `fieldPath` maps to a non-null root (`mapFieldPathToTopLevelRoot`) -- a diagnostic with
 *    no path, or an unmapped/malformed one, is left for the bottom panel and never claimed as
 *    "revealed" by this module (issue 08 step 5);
 * 3. that root is currently hidden (`effectiveHidden.has(root)`) -- a diagnostic on an already
 *    visible field is not a "hidden field issue" at all.
 */
export function computeHiddenFieldDiagnosticsSummary(args: {
  diagnostics: readonly ValidationDiagnostic[];
  def: DefEditorView;
  effectiveHidden: ReadonlySet<string>;
}): HiddenFieldDiagnosticsSummary {
  const { diagnostics, def, effectiveHidden } = args;
  if (effectiveHidden.size === 0 || diagnostics.length === 0) return EMPTY_SUMMARY;

  const defNodeIds = collectDefSubtreeNodeIds(def);
  const affectedRootIds = new Set<string>();
  let totalCount = 0;
  let blockingCount = 0;

  for (const diagnostic of diagnostics) {
    if (diagnostic.nodeId === null || !defNodeIds.has(diagnostic.nodeId)) continue;
    const root = mapFieldPathToTopLevelRoot(diagnostic.fieldPath);
    if (root === null || !effectiveHidden.has(root)) continue;
    affectedRootIds.add(root);
    totalCount += 1;
    if (diagnostic.blocking) blockingCount += 1;
  }

  if (totalCount === 0) return EMPTY_SUMMARY;
  return { affectedRootIds, totalCount, blockingCount };
}
