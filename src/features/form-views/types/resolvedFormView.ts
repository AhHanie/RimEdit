// Runtime resolved Form View shape used by the selector/manager UI and the `useFormViews`
// controller. Reuses the existing wire shapes already established elsewhere:
// `SchemaFormView` (schema-catalog/types.ts) for schema provenance and
// `BaseSchemaViewReference` (./formViews.ts) for a custom view's optional schema base --
// rather than inventing a third, incompatible provenance shape.
import type { FormViewOrigin, BaseSchemaViewReference } from "./formViews";
import type { FormViewSource } from "../../schema-catalog";

/** Reserved id for the synthetic Default View -- never a real schema/custom view id. */
export const DEFAULT_FORM_VIEW_ID = "default";

/**
 * One selectable entry in the combined `[Default, ...schema views, ...custom views]` list.
 * Regardless of `origin`, `hiddenFieldIds` is always the
 * *materialized* set of canonical top-level Def schema field ids this view hides -- resolution
 * of inherited schema-view deltas happens in Rust/the catalog; this type only
 * carries the already-resolved result across the selector/resolver boundary.
 */
export interface ResolvedFormView {
  id: string;
  targetDefType: string;
  label: string;
  description?: string;
  icon?: string;
  order: number;
  origin: FormViewOrigin;
  hiddenFieldIds: readonly string[];
  recommended: boolean;
  /** Schema provenance (origin === "schema" only): the concrete Def type whose declaration is
   * the winning source for this resolved view -- shown as read-only source text in the UI. */
  declaredOnDefType?: string;
  /** Schema provenance (origin === "schema" only): winning pack id/version. */
  source?: FormViewSource;
  /** Custom provenance (origin === "custom" only): the schema view this custom view was
   * originally duplicated/saved from, if any -- purely informational. */
  baseSchemaView?: BaseSchemaViewReference;
}

/**
 * An unsaved, tab-local deviation from the selected view's materialized hidden set. Never
 * persisted and never written through the custom-view store -- see
 * `useFormViews`'s doc comment for the full lifecycle. The coarse constructor
 * (`useFormViews().setOverrideHiddenFieldIds`) backs the selector's indicator/reset/discard
 * contract; `FormViewFieldChecklist` and the in-form inline hide affordances are the per-field
 * checkbox UI that actually produces overrides via user interaction.
 */
export interface FieldVisibilityOverride {
  hiddenFieldIds: ReadonlySet<string>;
  /** Whether `hiddenFieldIds` differs from the selected view's resolved hidden set. */
  isDirty: boolean;
}
