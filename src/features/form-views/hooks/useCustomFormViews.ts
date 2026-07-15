import { useCallback, useEffect, useRef, useState } from "react";
import { formatError } from "../../../lib/formatError";
import { noActiveProjectError } from "../lib/formViewErrors";
import {
  createCustomFormView,
  deleteCustomFormView,
  listCustomFormViews,
  resetCustomFormViewStore,
  updateCustomFormView,
} from "../api/formViews";
import type {
  BaseSchemaViewReference,
  CustomFormView,
  CustomFormViewUpdateInput,
  FormViewStoreWarning,
  ResetCustomFormViewStoreResult,
} from "../types/formViews";

export interface UseCustomFormViewsResult {
  views: CustomFormView[];
  warning: FormViewStoreWarning | null;
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
  createView: (
    name: string,
    hiddenFieldIds: string[],
    description?: string | null,
    baseSchemaView?: BaseSchemaViewReference | null,
  ) => Promise<CustomFormView>;
  updateView: (viewId: string, updates: CustomFormViewUpdateInput) => Promise<CustomFormView>;
  deleteView: (viewId: string) => Promise<void>;
  // Corruption/incompatible-version recovery: backs up (never deletes) any existing store file
  // and starts fresh. Surfaced here so a UI that shows `error`/`warning` for this scope also has
  // an obvious recovery action available, without needing its own `useCustomFormViews` instance.
  resetStore: () => Promise<ResetCustomFormViewStoreResult>;
}

/** Normalizes a caught error (a structured Tauri `AppError` or a plain thrown `Error`) to
 * English UI text through the shared diagnostic renderer, rather than reading `.message`/
 * `String(e)` directly and discarding a command rejection's structured `code`/`args`. */
function errorMessage(e: unknown): string {
  return formatError(e);
}

/// Thin data-fetching hook over the custom Form View store: lists a project's custom views for
/// a `{gameVersion, defType}` scope and exposes CRUD that reloads the list afterward. This is
/// deliberately minimal -- the actual selection/override/resolution logic (which view is
/// active, temporary field-visibility overrides, schema-view merging) belongs to
/// `features/form-views`'s later `useFormViews` (issue 06), not here.
export function useCustomFormViews(
  projectId: string | null,
  gameVersion: string,
  defType: string,
): UseCustomFormViewsResult {
  const [views, setViews] = useState<CustomFormView[]>([]);
  const [warning, setWarning] = useState<FormViewStoreWarning | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Two DIFFERENT concepts, deliberately kept separate (an earlier version conflated them into
  // one shared counter, which broke same-scope concurrent mutations -- see below):
  //
  // 1. `scopeEpochRef` -- a pure SCOPE-identity guard, bumped ONLY when the `{project,
  //    gameVersion, defType}` scope itself changes (a game-version/project switch, or a Def-type
  //    change), via the `scopeKeyRef` comparison below, updated unconditionally during render
  //    (not in a `useEffect`) so it can never lag behind. Every CRUD mutation captures this at
  //    its own entry point and, once its own backend call resolves, chains its `reload()` call
  //    if and ONLY IF the scope is still the same one it started in -- this is what stops a
  //    stale mutation from a since-abandoned scope from fetching (and applying) data for a scope
  //    nobody is looking at anymore.
  //
  // 2. `reloadSequenceRef` -- purely for ordering multiple *list fetches* against each other
  //    (whichever one started most recently wins if they resolve out of order), independent of
  //    scope. `reload()` bumps and captures this every time it runs; it does NOT gate whether a
  //    CRUD mutation chains a reload at all.
  //
  // Why the split matters: two CRUD mutations in the SAME scope (e.g. two Duplicate clicks in a
  // row) each capture `scopeEpochRef`'s value at entry (unchanged by each other, since neither
  // bumps it -- only a scope change does). Each one's own reload() call, once its own backend
  // write resolves, is therefore never skipped just because the OTHER mutation's reload also
  // ran in the meantime and bumped `reloadSequenceRef` -- both mutations' writes get their own
  // chance to be reflected via their own reload, and the (separate) `reloadSequenceRef` check
  // inside `reload()` only decides which of those *list fetches'* results wins if they resolve
  // out of order, never whether a reload should be skipped/attempted in the first place. A
  // single shared counter used for both purposes previously caused the second mutation's own
  // reload to "consume" the counter, making the first (slower) mutation wrongly conclude
  // something newer had superseded it and skip its own reload -- silently leaving its
  // successfully created/updated/deleted view missing from the UI until an unrelated future
  // reload happened to pick it up.
  const scopeEpochRef = useRef(0);
  const scopeKeyRef = useRef<string | null>(null);
  const currentScopeKey = `${projectId ?? ""}::${gameVersion}::${defType}`;
  if (scopeKeyRef.current !== currentScopeKey) {
    scopeKeyRef.current = currentScopeKey;
    scopeEpochRef.current += 1;
  }
  const reloadSequenceRef = useRef(0);

  const reload = useCallback(async () => {
    const mySequence = ++reloadSequenceRef.current;
    const myScopeEpoch = scopeEpochRef.current;
    if (!projectId) {
      setViews([]);
      setWarning(null);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await listCustomFormViews(projectId, gameVersion, defType);
      // Discard if the scope has moved on entirely, OR if a newer list fetch (for this scope or
      // any other) has already started and should win instead.
      if (scopeEpochRef.current !== myScopeEpoch) return;
      if (reloadSequenceRef.current !== mySequence) return;
      setViews(result.views);
      setWarning(result.warning);
    } catch (e) {
      if (scopeEpochRef.current !== myScopeEpoch) return;
      if (reloadSequenceRef.current !== mySequence) return;
      setError(errorMessage(e));
    } finally {
      if (scopeEpochRef.current === myScopeEpoch && reloadSequenceRef.current === mySequence) {
        setLoading(false);
      }
    }
  }, [projectId, gameVersion, defType]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const createView = useCallback(
    async (
      name: string,
      hiddenFieldIds: string[],
      description?: string | null,
      baseSchemaView?: BaseSchemaViewReference | null,
    ) => {
      if (!projectId) {
        throw noActiveProjectError("No active project to save a custom Form View to.");
      }
      const myScopeEpoch = scopeEpochRef.current;
      const created = await createCustomFormView(
        projectId,
        gameVersion,
        defType,
        name,
        hiddenFieldIds,
        description,
        baseSchemaView,
      );
      // Refresh the list as long as the SCOPE hasn't moved on -- unconditionally, regardless of
      // whether some other same-scope mutation/reload has ALSO happened meanwhile. Skipping this
      // reload only when the scope itself changed (not merely because `reloadSequenceRef` also
      // advanced) is what lets two overlapping same-scope mutations both eventually surface in
      // the UI, rather than the slower one's own reload being wrongly treated as superseded.
      if (scopeEpochRef.current === myScopeEpoch) {
        await reload();
      }
      return created;
    },
    [projectId, gameVersion, defType, reload],
  );

  const updateView = useCallback(
    async (viewId: string, updates: CustomFormViewUpdateInput) => {
      if (!projectId) {
        throw noActiveProjectError("No active project to update a custom Form View in.");
      }
      const myScopeEpoch = scopeEpochRef.current;
      const updated = await updateCustomFormView(projectId, viewId, updates);
      if (scopeEpochRef.current === myScopeEpoch) {
        await reload();
      }
      return updated;
    },
    [projectId, reload],
  );

  const deleteView = useCallback(
    async (viewId: string) => {
      if (!projectId) {
        throw noActiveProjectError("No active project to delete a custom Form View from.");
      }
      const myScopeEpoch = scopeEpochRef.current;
      await deleteCustomFormView(projectId, viewId);
      if (scopeEpochRef.current === myScopeEpoch) {
        await reload();
      }
    },
    [projectId, reload],
  );

  const resetStore = useCallback(async () => {
    if (!projectId) {
      throw noActiveProjectError("No active project to reset the custom Form View store for.");
    }
    const myScopeEpoch = scopeEpochRef.current;
    const result = await resetCustomFormViewStore(projectId);
    if (scopeEpochRef.current === myScopeEpoch) {
      await reload();
    }
    return result;
  }, [projectId, reload]);

  return {
    views,
    warning,
    loading,
    error,
    reload,
    createView,
    updateView,
    deleteView,
    resetStore,
  };
}
