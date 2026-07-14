// Wire types for the project-scoped custom Form View store (backend: `src-tauri/src/form_views/`).
// Mirrors Plan.md section 6's JSON shape. Schema-defined views (`SchemaFormView`) and the
// runtime `ResolvedFormView` union live in the schema-catalog / issue-05 resolver, not here --
// this feature only owns user-editable custom views and the store's own preferences.

export interface FormViewTarget {
  gameVersion: string;
  defType: string;
}

// Optional provenance recorded when a custom view was copied/saved from a schema-defined view.
// Purely informational: a missing/renamed base is surfaced as a nonblocking notice by a future
// resolution layer (issue 05+), never used here to recompute `hiddenFieldIds`.
export interface BaseSchemaViewReference {
  viewId: string;
  packId: string;
  packVersion: string;
  declaredOnDefType: string;
}

export interface CustomFormView {
  id: string;
  target: FormViewTarget;
  name: string;
  description: string | null;
  hiddenFieldIds: string[];
  baseSchemaView: BaseSchemaViewReference | null;
  createdAt: string;
  updatedAt: string;
}

export type FormViewOrigin = "default" | "schema" | "custom";

export interface SelectedFormViewRef {
  origin: FormViewOrigin;
  id: string;
}

export interface FormViewStoreWarning {
  code: string;
  message: string;
}

export interface ListCustomFormViewsResult {
  views: CustomFormView[];
  warning: FormViewStoreWarning | null;
}

export interface GetLastSelectedFormViewResult {
  selected: SelectedFormViewRef | null;
  warning: FormViewStoreWarning | null;
}

export interface DeleteCustomFormViewResult {
  deletedId: string;
}

export interface ResetCustomFormViewStoreResult {
  backupPath: string | null;
}

// Fields accepted by `updateCustomFormView`. Omitted fields are left unchanged.
//
// `description` is tri-state: omit the key entirely to leave it unchanged, pass `null` to
// explicitly clear it, or pass a string to set it. This mirrors the backend's
// `CustomFormViewUpdate.description: Option<Option<String>>` -- see
// `updateCustomFormView` in `../api/formViews.ts` for how that distinction survives the
// Tauri IPC boundary (a plain JSON `null` can't tell "omitted" from "explicitly null" apart).
export interface CustomFormViewUpdateInput {
  name?: string;
  hiddenFieldIds?: string[];
  description?: string | null;
}
