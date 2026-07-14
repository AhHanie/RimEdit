import { invoke } from "@tauri-apps/api/core";
import type {
  BaseSchemaViewReference,
  CustomFormView,
  CustomFormViewUpdateInput,
  DeleteCustomFormViewResult,
  FormViewOrigin,
  GetLastSelectedFormViewResult,
  ListCustomFormViewsResult,
  ResetCustomFormViewStoreResult,
} from "../types/formViews";

// Read-only: does not require a writable project (a read-only source tab can still list and
// select a project's custom views, per Plan.md section 6).
export function listCustomFormViews(
  projectId: string,
  gameVersion?: string,
  defType?: string,
): Promise<ListCustomFormViewsResult> {
  return invoke("list_custom_form_views", {
    projectId,
    gameVersion: gameVersion ?? null,
    defType: defType ?? null,
  });
}

export function createCustomFormView(
  projectId: string,
  gameVersion: string,
  defType: string,
  name: string,
  hiddenFieldIds: string[],
  description?: string | null,
  baseSchemaView?: BaseSchemaViewReference | null,
): Promise<CustomFormView> {
  return invoke("create_custom_form_view", {
    projectId,
    gameVersion,
    defType,
    name,
    hiddenFieldIds,
    description: description ?? null,
    baseSchemaView: baseSchemaView ?? null,
  });
}

// `description` is tri-state (see `CustomFormViewUpdateInput`): omitting the key from `updates`
// leaves it unchanged, `null` clears it, a string sets it. The backend command takes two plain
// parameters (`description` + `clearDescription`) rather than a doubly-nested optional, since a
// JSON `null` can't otherwise distinguish "not provided" from "explicitly cleared" over IPC --
// so this wrapper is what actually preserves that distinction, by only setting `clearDescription`
// when the caller's `updates` object has an own `description` key at all.
export function updateCustomFormView(
  projectId: string,
  viewId: string,
  updates: CustomFormViewUpdateInput,
): Promise<CustomFormView> {
  const descriptionProvided = Object.prototype.hasOwnProperty.call(updates, "description");
  const clearDescription = descriptionProvided && updates.description === null;
  return invoke("update_custom_form_view", {
    projectId,
    viewId,
    name: updates.name ?? null,
    hiddenFieldIds: updates.hiddenFieldIds ?? null,
    description: clearDescription ? null : (updates.description ?? null),
    clearDescription,
  });
}

export function deleteCustomFormView(
  projectId: string,
  viewId: string,
): Promise<DeleteCustomFormViewResult> {
  return invoke("delete_custom_form_view", { projectId, viewId });
}

// Corruption/incompatible-version recovery: backs up (never deletes) any existing store file and
// starts a fresh empty store. Requires a writable project -- this is a destructive disk write.
export function resetCustomFormViewStore(
  projectId: string,
): Promise<ResetCustomFormViewStoreResult> {
  return invoke("reset_custom_form_view_store", { projectId });
}

// Persists the last clean (non-overridden) view selection for a `{gameVersion, defType}` scope.
// Requires a writable project (it is a real disk write); see `getLastSelectedFormView` below for
// the read-only counterpart.
export function setLastSelectedFormView(
  projectId: string,
  gameVersion: string,
  defType: string,
  origin: FormViewOrigin,
  id: string,
): Promise<void> {
  return invoke("set_last_selected_form_view", {
    projectId,
    gameVersion,
    defType,
    origin,
    id,
  });
}

// Read-only: does not require a writable project, mirroring `listCustomFormViews`.
export function getLastSelectedFormView(
  projectId: string,
  gameVersion: string,
  defType: string,
): Promise<GetLastSelectedFormViewResult> {
  return invoke("get_last_selected_form_view", { projectId, gameVersion, defType });
}
