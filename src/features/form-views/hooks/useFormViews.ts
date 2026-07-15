// The Form View selection/resolution controller (issue 06, Plan.md section 9). Owns:
//   - the combined selectable list (Default + resolved schema views + project custom views);
//   - the resolved current selection, with safe fallback-to-Default and re-persistence;
//   - a tab-local, unsaved `FieldVisibilityOverride` slot (issue 07 is the sole owner of the
//     per-field checkbox UI that actually produces one; this issue only needs the state slot and
//     a coarse constructor to prove the selector's indicator/reset/discard contract);
//   - the resulting `effectiveHidden`/`visibleTopLevelFieldIds`, ready to hand straight to
//     `useXmlFormController`'s `visibleTopLevelFieldIds` (issue 05's contract).
//
// Instantiated once per mounted `XmlEditorPane` (Plan.md section 9: "`useFormViews` in
// `XmlEditorPane`"). Override/selection-in-progress state is keyed by
// `{projectId, gameVersion, defType, ordinal}` *within* this hook instance so two Defs of the
// same type opened in one multi-Def file don't share state, AND so a game-version (or project)
// change is treated as an entirely new scope rather than reusing a stale "already loaded"
// selection fetched under the OLD scope (see `formViewsStateKey`'s doc comment). Two different
// panes never share state in the first place because each pane owns its own hook call
// (`EditorWorkspace` keeps every pane's component instance mounted across tab switches -- see
// `EditorWorkspace.tsx` -- so this per-instance state survives a tab switch and is torn down on
// tab close exactly like the rest of that pane's local state).
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SchemaCatalog } from "../../schema-catalog";
import { collectEffectiveTopLevelDefFields } from "../../xml-editor/lib/formDescriptors";
import { getLastSelectedFormView, setLastSelectedFormView } from "../api/formViews";
import { useCustomFormViews } from "./useCustomFormViews";
import { noActiveProjectError, noUnsavedChangesError } from "../lib/formViewErrors";
import {
  buildAvailableFormViews,
  buildDefaultFormView,
  computeEffectiveVisibility,
  formViewsStateKey,
  isHiddenSetDirty,
  resolveSelectedFormView,
} from "../lib/resolveFormViews";
import { DEFAULT_FORM_VIEW_ID, type FieldVisibilityOverride, type ResolvedFormView } from "../types/resolvedFormView";
import type {
  BaseSchemaViewReference,
  CustomFormView,
  CustomFormViewUpdateInput,
  FormViewStoreWarning,
  ResetCustomFormViewStoreResult,
  SelectedFormViewRef,
} from "../types/formViews";

export interface FormViewsPaneIdentity {
  locationId: string;
  relativePath: string;
  sourceKind: "project" | "source";
}

export interface FormViewsSelectedDef {
  defType: string;
  /** Zero-based position among this file's own top-level Defs (matches the ordinal used
   * elsewhere for the same purpose, e.g. `PatchPreviewTarget`) -- disambiguates two Defs of the
   * same type opened in one multi-Def file so their overrides/selections stay independent. */
  ordinal: number;
}

export interface UseFormViewsArgs {
  projectId: string | null | undefined;
  gameVersion: string | null | undefined;
  catalog: SchemaCatalog | null;
  /** Reserved for future per-pane diagnostics/telemetry; not read by this issue. Kept as an
   * explicit parameter slot (rather than added later) per Plan.md section 9's stated inputs. */
  pane: FormViewsPaneIdentity | null;
  selectedDef: FormViewsSelectedDef | null;
}

export interface UseFormViewsResult {
  /** Whether Form View controls should render at all -- `profile === "defs"` with a selected Def
   * that has a resolvable schema (Plan.md section 11). False for Patch/About/raw and for a Def
   * type with no schema entry. */
  applicable: boolean;
  availableViews: ResolvedFormView[];
  selectedView: ResolvedFormView;
  effectiveHidden: ReadonlySet<string>;
  hiddenCount: number;
  /** Ready to pass straight to `useXmlFormController`'s `visibleTopLevelFieldIds` (issue 05). */
  visibleTopLevelFieldIds: ReadonlySet<string> | null;
  override: FieldVisibilityOverride | null;
  hasDirtyOverride: boolean;

  /** Clean selection change: clears any override and persists the new preference (Plan.md
   * section 9/12 -- "persist a clean selection preference"; never called while an override is
   * dirty without the caller having already resolved that via the switch-confirmation flow). */
  selectView: (ref: SelectedFormViewRef) => void;
  /** Convenience for the persistent "Show full form" action (Plan.md section 8). */
  selectDefaultView: () => void;
  /** "Reset to selected view" / "Discard override" (Plan.md section 8 step 4) -- same state
   * action, different call-site copy. */
  resetOverride: () => void;
  /** Coarse override constructor. Issue 06 has no field-checkbox UI to drive this from -- it
   * exists so tests (and issue 07) can exercise the indicator/reset/discard/save-as-custom
   * contract against a synthetic override. Replaces the override wholesale; `isDirty` is
   * computed against the *currently selected* view. */
  setOverrideHiddenFieldIds: (hidden: ReadonlySet<string>) => void;
  /** Materializes the current override's hidden set as a new custom view, selects it, and
   * clears the override (Plan.md section 8 step 3/6: "Save as custom view"). Throws if there is
   * no active project/Def type or no override to save. */
  saveOverrideAsCustomView: (
    name: string,
    description?: string | null,
  ) => Promise<CustomFormView>;
  /** Creates a new custom view copying `view`'s hidden set (Default/schema/custom all valid
   * sources -- Plan.md section 8 step 5/17: "schema-defined views ... can be duplicated"). Does
   * not change the current selection. */
  duplicateAsCustomView: (view: ResolvedFormView, name?: string) => Promise<CustomFormView>;
  /** Creates a brand-new custom view with an empty hidden set (hides nothing, same effective
   * visibility as Default until issue 07's checkbox UI customizes it). Does NOT change the
   * current selection -- creating a view is not itself a switch, so it must not silently clear
   * a dirty override. Callers that want to auto-select the freshly created view should route
   * through the same `useViewSwitchConfirmation`/`selectView` gate as any other switch, e.g.
   * `requestSwitch({ origin: "custom", id: created.id })`, so a dirty override still prompts. */
  createCustomView: (name: string, description?: string | null) => Promise<CustomFormView>;
  renameCustomView: (viewId: string, name: string) => Promise<CustomFormView>;
  updateCustomView: (
    viewId: string,
    updates: CustomFormViewUpdateInput,
  ) => Promise<CustomFormView>;
  /** Deletes a custom view; if it was the current selection, immediately selects Default
   * (Plan.md section 12 edge case). */
  deleteCustomView: (viewId: string) => Promise<void>;

  /**
   * Reads the CURRENT `{project, gameVersion, defType, ordinal}` scope identity fresh, every
   * call -- unlike every other value on this object, which is a snapshot frozen into whichever
   * render produced it. Exists specifically for UI components with an async handler that awaits
   * a `useFormViews`/`useCustomFormViews` call and then chains a follow-up `selectView`/
   * `requestSwitch` call (e.g. `FormViewManagerDialog`'s "Create" button, or
   * `useViewSwitchConfirmation`'s "Save as custom view" flow): capture `getScopeKey()` before
   * starting the async call, and skip the chained selection entirely if `getScopeKey()` no
   * longer matches by the time it resolves. A component's own `controller` prop is itself a
   * snapshot -- if the scope changes (a Def/tab/game-version switch) while that async call is in
   * flight, the component's already-running handler keeps executing with its ORIGINAL, now-stale
   * `controller` closure, and reading a plain value off it again after the `await` would just
   * return the same frozen snapshot, not the truth. This function's identity never changes
   * across renders (so a stale closure holding a reference to it still calls the SAME function),
   * and it always reads the live ref underneath, so it always reports the truth regardless of
   * which render's closure invokes it. Relying on this is required in addition to (not instead
   * of) `selectView`'s own internal key-mismatch guard: that guard stops a stale call from
   * corrupting the new scope's in-memory selection, but it does not stop the call from firing at
   * all -- and firing still issues a real `setLastSelectedFormView` persist attempt whose
   * success/failure would otherwise be misattributed to whatever scope happens to be active by
   * the time it resolves.
   */
  getScopeKey: () => string | null;

  customViews: CustomFormView[];
  customViewsLoading: boolean;
  customViewsWarning: FormViewStoreWarning | null;
  /** A failed `list_custom_form_views` fetch (e.g. a transiently unreadable or corrupt on-disk
   * custom-view store). While set, reconciliation/fallback is deliberately suspended (see the
   * reconciliation effect) so a load failure can never overwrite a real persisted selection --
   * but that means the UI must surface this and offer a way out, not leave the user silently
   * stuck on a fallback view. Use `reloadCustomViews`/`resetCustomViewStore` to recover. */
  customViewsError: string | null;
  /** Re-runs the `list_custom_form_views` fetch for the current scope -- the retry action for
   * `customViewsError`. */
  reloadCustomViews: () => Promise<void>;
  /** Corruption/incompatible-version recovery: backs up (never deletes) the existing on-disk
   * custom-view store and starts a fresh empty one, then reloads. Requires a writable project. */
  resetCustomViewStore: () => Promise<ResetCustomFormViewStoreResult>;
  /** Set when persisting a selection/fallback preference failed -- non-blocking, informational.
   * Cleared automatically the next time a persist succeeds. Rendered by `FormViewSelector`
   * (visible even when the manager dialog is closed, since a failure can happen from the plain
   * selector/"Show full form" action) and by `FormViewManagerDialog` alongside its other
   * warning/error banners. Without a visible surface, the user sees their new selection take
   * effect in memory with no indication it failed to persist, and it silently reverts after an
   * app restart with no explanation. */
  persistWarning: string | null;
}

interface PerDefState {
  key: string;
  /** The raw selection reference as last known-persisted or explicitly chosen. `null` means
   * "no explicit selection yet" (either still loading the preference, or none was ever saved) --
   * `resolveSelectedFormView` treats that the same as an unresolvable reference and falls back
   * to the recommended/Default view, but a `null` ref is deliberately never auto-persisted
   * (only a real broken/missing reference is -- see the reconciliation effect below). */
  selectionRef: SelectedFormViewRef | null;
  /** Whether the initial `getLastSelectedFormView` fetch for this key has completed. Guards the
   * fallback-reconciliation effect so it never fires before the real preference has been read
   * (which would otherwise overwrite a valid stored preference with Default). */
  loaded: boolean;
  /**
   * Monotonic counter bumped every time `selectionRef` is set by something OTHER than the
   * initial-preference fetch itself -- an explicit `selectView` call, or the
   * fallback-reconciliation effect applying a fallback. The initial-fetch effect captures this
   * value when it STARTS, and only applies its (by-then possibly stale) result if the value is
   * still unchanged when the fetch RESOLVES. Without this, a manual selection made while that
   * fetch is still in flight would be silently reverted the moment the stale fetch completes
   * (and could then trigger an incorrect reconciliation-fallback persist on top of that).
   */
  selectionGeneration: number;
  override: FieldVisibilityOverride | null;
}

function freshState(key: string): PerDefState {
  return { key, selectionRef: null, loaded: false, selectionGeneration: 0, override: null };
}

export function useFormViews({
  projectId,
  gameVersion,
  catalog,
  selectedDef,
}: UseFormViewsArgs): UseFormViewsResult {
  const { t, i18n } = useTranslation("editor");
  const defType = selectedDef?.defType ?? null;
  const defSchema = defType && catalog ? (catalog.defTypes[defType] ?? null) : null;
  const applicable = !!selectedDef && !!defSchema;

  const normalizedGameVersion = gameVersion ?? "";
  // Custom views are scoped by {project, gameVersion, defType} (Plan.md section 3). Only ask
  // the store for real data once we actually have all three -- an empty defType/gameVersion
  // would otherwise be a meaningless (and misleading) list call.
  const customViewsProjectId = projectId && normalizedGameVersion && defType ? projectId : null;
  const customFormViews = useCustomFormViews(
    customViewsProjectId,
    normalizedGameVersion,
    defType ?? "",
  );

  const knownTopLevel = useMemo(() => {
    if (!defSchema || !catalog) return new Set<string>();
    return new Set(collectEffectiveTopLevelDefFields(defSchema, catalog));
  }, [defSchema, catalog]);

  const availableViews = useMemo(() => {
    if (!defType) return [];
    return buildAvailableFormViews(defType, defSchema?.formViews, customFormViews.views, i18n.language);
  }, [defType, defSchema, customFormViews.views, i18n.language]);

  // Per-{defType,ordinal} state, stashed across switches within this single pane instance (see
  // module doc comment). Mirrors the stash-ref pattern `useXmlFormController` already uses for
  // hidden-field drafts. Depends on the `defType`/`ordinal` primitives (not the `selectedDef`
  // object) so a caller that reconstructs that object on every render (as `XmlEditorPane` does)
  // doesn't churn this effect on every keystroke elsewhere in the form.
  const stashRef = useRef<Map<string, PerDefState>>(new Map());
  const activeKeyRef = useRef<string | null>(null);
  const [active, setActive] = useState<PerDefState | null>(null);
  const activeRef = useRef<PerDefState | null>(null);
  activeRef.current = active;

  const selectedDefOrdinal = selectedDef?.ordinal ?? null;

  // Single shared "scope generation" every async completion in this hook that performs a
  // CHAINED side effect (persisting a selection, or auto-selecting after a CRUD mutation)
  // checks against before applying that side effect -- analogous to (and coordinating with, via
  // `customFormViews`'s own instance of the same pattern) `useCustomFormViews`'s
  // `generationRef`. Bumped exactly once whenever the full `{project, gameVersion, defType,
  // ordinal}` scope changes; updated unconditionally during render (not in an effect) so it can
  // never lag behind a scope change regardless of which render's closure ends up executing a
  // stale completion. `active.selectionGeneration` (on `PerDefState`) is a DIFFERENT, narrower
  // concept -- it only guards the initial-preference fetch against a manual selection made
  // *within the same scope* -- this ref is what guards every chained side effect against the
  // scope itself having moved on entirely.
  const scopeGenerationRef = useRef(0);
  const scopeKeyRef = useRef<string | null>(null);
  const currentScopeKey =
    defType !== null && selectedDefOrdinal !== null
      ? formViewsStateKey(projectId ?? null, gameVersion ?? null, defType, selectedDefOrdinal)
      : null;
  if (scopeKeyRef.current !== currentScopeKey) {
    scopeKeyRef.current = currentScopeKey;
    scopeGenerationRef.current += 1;
  }

  // Stable identity across every render (empty deps) -- see the `getScopeKey` doc comment on
  // `UseFormViewsResult` for why UI components need this rather than a plain snapshot value.
  const getScopeKey = useCallback(() => scopeKeyRef.current, []);

  // Same-scope persist-attempt ordering, a SEPARATE concern from `scopeGenerationRef` above.
  // `scopeGenerationRef` only tells apart "this scope" from "a different scope"; it does nothing
  // to order two persist attempts that both belong to the SAME scope (e.g. selecting view A,
  // then promptly selecting Default before A's persist call has resolved). Bumped on every
  // `selectView` call (and by the reconciliation effect applying a fallback, which is also a new
  // "attempt" at establishing what the clean selection is) regardless of scope; a persist
  // completion checks it alongside the scope generation and is a no-op (does not touch
  // `persistWarning` either way, success or failure) if a newer attempt has since started --
  // otherwise an older, already-superseded attempt's outcome could display a false failure
  // warning against a newer attempt that actually saved successfully, or wrongly clear a real
  // warning belonging to that newer attempt.
  const selectAttemptRef = useRef(0);

  useEffect(() => {
    // Includes `projectId`/`gameVersion` so a game-version (or project) change is treated as a
    // brand-new scope with its own `loaded: false` state, not a reuse of whatever was already
    // loaded for the OLD scope's `{project, gameVersion, defType}` custom-view/preference
    // namespace (Plan.md section 3/6). Without this, switching game version would keep
    // `active.loaded === true` and the stale OLD-scope `selectionRef`, letting the
    // fallback-reconciliation effect below wrongly conclude the NEW scope's real preference
    // (never actually fetched) doesn't resolve, and re-persist a fallback over it.
    const key =
      defType !== null && selectedDefOrdinal !== null
        ? formViewsStateKey(projectId ?? null, gameVersion ?? null, defType, selectedDefOrdinal)
        : null;
    if (activeKeyRef.current === key) return;

    const prevKey = activeKeyRef.current;
    const prevState = activeRef.current;
    if (prevKey !== null && prevState && prevState.key === prevKey) {
      stashRef.current.set(prevKey, prevState);
    }
    activeKeyRef.current = key;
    setActive(key === null ? null : (stashRef.current.get(key) ?? freshState(key)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, gameVersion, defType, selectedDefOrdinal]);

  const [persistWarning, setPersistWarning] = useState<string | null>(null);

  // Fetch the persisted preference once per key.
  useEffect(() => {
    if (!active || active.loaded) return;
    if (!projectId || !gameVersion || !defType) return;
    let cancelled = false;
    const key = active.key;
    // Captured at fetch-start time: if a manual `selectView` call (or a reconciliation fallback)
    // bumps this before the fetch resolves, `prev.selectionGeneration` below will no longer
    // match, and the stale result is discarded instead of clobbering the newer selection.
    const fetchGeneration = active.selectionGeneration;
    getLastSelectedFormView(projectId, gameVersion, defType)
      .then((result) => {
        if (cancelled) return;
        setActive((prev) =>
          prev && prev.key === key && prev.selectionGeneration === fetchGeneration
            ? { ...prev, selectionRef: result.selected, loaded: true }
            : prev,
        );
      })
      .catch(() => {
        if (cancelled) return;
        // No `selectionRef`/generation implications on failure -- just unblock `loaded` so the
        // resolver can fall back to recommended/Default (Plan.md section 6/12) without waiting
        // forever. Harmless (idempotent) even if a manual selection already flipped `loaded` to
        // `true` in the meantime.
        setActive((prev) => (prev && prev.key === key ? { ...prev, loaded: true } : prev));
      });
    return () => {
      cancelled = true;
    };
    // `active` is read via `active.loaded`/`active.key` above; re-running per key change (not
    // per every `active` mutation, e.g. an override edit) is intentional -- see the `key`/
    // `loaded` deps below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active?.key, active?.loaded, projectId, gameVersion, defType]);

  const selectedView = useMemo(() => {
    const view = resolveSelectedFormView(availableViews, active?.selectionRef ?? null);
    return view ?? buildFallbackDefault(defType ?? "");
  }, [availableViews, active?.selectionRef, defType]);

  // Fallback reconciliation (Plan.md section 6/12, issue instruction: "correctly re-persist the
  // fallback as the new clean selection"). Only fires for a *real* stored reference that no
  // longer resolves (a deleted custom view, a schema view/pack that disappeared) -- never for
  // "no stored reference yet", which is not a fallback, just an unset preference.
  useEffect(() => {
    if (!active || !active.loaded) return;
    if (!active.selectionRef) return;
    if (!projectId || !gameVersion || !defType) return;
    // Require the catalog to have actually resolved this Def type (and the custom-view list to
    // have finished its own initial load, WITHOUT error) before treating a non-matching
    // reference as a genuine fallback. `customFormViews.error` is critical here, not just
    // `loading`: `useCustomFormViews` reports `loading: false` with an *empty* `views` array
    // both when there really are zero custom views AND when the last list fetch failed (e.g. a
    // transiently unreadable or corrupt on-disk store) -- those two states are otherwise
    // indistinguishable from here. Treating a load failure as "confirmed gone" would silently
    // overwrite a real persisted custom-view selection with Default/recommended the moment a
    // transient read error occurs, contradicting Plan.md section 12's "surface diagnostic, do
    // not overwrite" rule for custom-view store problems. While there's a load error, leave the
    // current selection exactly as-is (do not reconcile, do not persist) until a subsequent
    // successful reload proves one way or the other.
    if (!defSchema || customFormViews.loading || customFormViews.error) return;
    const stillResolves = availableViews.some(
      (v) => v.origin === active.selectionRef!.origin && v.id === active.selectionRef!.id,
    );
    if (stillResolves) return;
    const fallback = selectedView;
    const key = active.key;
    setActive((prev) =>
      prev && prev.key === key
        ? {
            ...prev,
            selectionRef: { origin: fallback.origin, id: fallback.id },
            // Bumped so a still-in-flight initial-preference fetch (started before `loaded`
            // became true via a manual `selectView`, then raced by this reconciliation) can't
            // later overwrite this fallback with its own stale result -- see the initial-fetch
            // effect's `selectionGeneration` guard above.
            selectionGeneration: prev.selectionGeneration + 1,
          }
        : prev,
    );
    const myScopeGeneration = scopeGenerationRef.current;
    const myAttempt = ++selectAttemptRef.current;
    void setLastSelectedFormView(projectId, gameVersion, defType, fallback.origin, fallback.id)
      .then(() => {
        if (scopeGenerationRef.current !== myScopeGeneration) return;
        if (selectAttemptRef.current !== myAttempt) return;
        setPersistWarning(null);
      })
      .catch(() => {
        if (scopeGenerationRef.current !== myScopeGeneration) return;
        if (selectAttemptRef.current !== myAttempt) return;
        setPersistWarning(t("formViews.persistWarning.fallbackSaveFailed"));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    active,
    availableViews,
    selectedView,
    projectId,
    gameVersion,
    defType,
    defSchema,
    customFormViews.loading,
    customFormViews.error,
  ]);

  const { effectiveHidden, visibleTopLevelFieldIds } = useMemo(
    () =>
      computeEffectiveVisibility({
        selected: selectedView,
        override: active?.override ?? null,
        knownTopLevel,
      }),
    [selectedView, active?.override, knownTopLevel],
  );

  const selectView = useCallback(
    (ref: SelectedFormViewRef) => {
      if (defType === null || selectedDefOrdinal === null) return;
      const key = formViewsStateKey(projectId ?? null, gameVersion ?? null, defType, selectedDefOrdinal);
      const myScopeGeneration = scopeGenerationRef.current;
      const myAttempt = ++selectAttemptRef.current;
      setActive((prev) => {
        if (prev && prev.key === key) {
          return {
            ...prev,
            selectionRef: ref,
            override: null,
            loaded: true,
            // Bumped so a still-in-flight initial-preference fetch (started before this
            // explicit choice) discards its stale result instead of reverting it once that
            // fetch finally resolves -- see the initial-fetch effect's guard above.
            selectionGeneration: prev.selectionGeneration + 1,
          };
        }
        if (prev) {
          // `prev` exists but for a DIFFERENT key than this call computed: this `selectView`
          // closure is stale (created for a scope/Def that is no longer active -- e.g. it was
          // captured by an async UI handler before a scope change, or is a chained call from a
          // CRUD mutation whose scope has since moved on). Never clobber the current, unrelated
          // active state with this stale key -- a plain no-op, not a silent overwrite.
          return prev;
        }
        // No active state at all yet (e.g. called before the key-tracking effect above has had
        // a chance to run) -- legitimate first-time initialization for this key.
        return { key, selectionRef: ref, override: null, loaded: true, selectionGeneration: 1 };
      });
      if (projectId && gameVersion) {
        void setLastSelectedFormView(
          projectId,
          gameVersion,
          defType,
          ref.origin,
          ref.id,
        )
          .then(() => {
            if (scopeGenerationRef.current !== myScopeGeneration) return;
            if (selectAttemptRef.current !== myAttempt) return;
            setPersistWarning(null);
          })
          .catch(() => {
            if (scopeGenerationRef.current !== myScopeGeneration) return;
            if (selectAttemptRef.current !== myAttempt) return;
            setPersistWarning(t("formViews.persistWarning.selectionSaveFailed"));
          });
      }
    },
    [projectId, gameVersion, defType, selectedDefOrdinal, t],
  );

  const selectDefaultView = useCallback(() => {
    selectView({ origin: "default", id: DEFAULT_FORM_VIEW_ID });
  }, [selectView]);

  const resetOverride = useCallback(() => {
    setActive((prev) => (prev ? { ...prev, override: null } : prev));
  }, []);

  const setOverrideHiddenFieldIds = useCallback(
    (hidden: ReadonlySet<string>) => {
      setActive((prev) => {
        if (!prev) return prev;
        const dirty = isHiddenSetDirty(hidden, selectedView);
        return { ...prev, override: { hiddenFieldIds: new Set(hidden), isDirty: dirty } };
      });
    },
    [selectedView],
  );

  const saveOverrideAsCustomView = useCallback(
    async (name: string, description?: string | null): Promise<CustomFormView> => {
      if (!active?.override) {
        throw noUnsavedChangesError("No unsaved Form View changes to save.");
      }
      if (!projectId || !gameVersion || defType === null) {
        throw noActiveProjectError("No active project to save a custom Form View to.");
      }
      const myScopeGeneration = scopeGenerationRef.current;
      const baseSchemaView = baseSchemaViewFor(selectedView);
      const created = await customFormViews.createView(
        name,
        [...active.override.hiddenFieldIds],
        description ?? null,
        baseSchemaView,
      );
      // Only auto-select the freshly created view if the scope hasn't moved on since this call
      // started -- otherwise `created` belongs to a scope nobody is looking at anymore, and
      // selecting it would replace the CURRENT (different) scope's active selection with one
      // that doesn't even apply there. (`selectView` itself also independently guards against
      // this via its own key check, but this explicit guard keeps the same "check the shared
      // scope generation before any side effect" contract other call sites in this hook follow.)
      if (scopeGenerationRef.current === myScopeGeneration) {
        selectView({ origin: "custom", id: created.id });
      }
      return created;
    },
    [active?.override, projectId, gameVersion, defType, selectedView, customFormViews, selectView],
  );

  const duplicateAsCustomView = useCallback(
    async (view: ResolvedFormView, name?: string): Promise<CustomFormView> => {
      if (!projectId || !gameVersion || defType === null) {
        throw noActiveProjectError("No active project to duplicate a Form View for.");
      }
      const label = name?.trim() || `${view.label} copy`;
      const baseSchemaView = baseSchemaViewFor(view);
      return customFormViews.createView(
        label,
        [...view.hiddenFieldIds],
        view.description ?? null,
        baseSchemaView,
      );
    },
    [projectId, gameVersion, defType, customFormViews],
  );

  const createCustomView = useCallback(
    async (name: string, description?: string | null): Promise<CustomFormView> => {
      if (!projectId || !gameVersion || defType === null) {
        throw noActiveProjectError("No active project to create a custom Form View for.");
      }
      // Intentionally does not select the new view -- see the interface doc comment. Creating a
      // view must never bypass the dirty-override switch confirmation.
      return customFormViews.createView(name, [], description ?? null, null);
    },
    [projectId, gameVersion, defType, customFormViews],
  );

  const updateCustomView = useCallback(
    (viewId: string, updates: CustomFormViewUpdateInput) => customFormViews.updateView(viewId, updates),
    [customFormViews],
  );

  const renameCustomView = useCallback(
    (viewId: string, name: string) => customFormViews.updateView(viewId, { name }),
    [customFormViews],
  );

  const deleteCustomView = useCallback(
    async (viewId: string) => {
      const myScopeGeneration = scopeGenerationRef.current;
      await customFormViews.deleteView(viewId);
      // Scope check first: if it's moved on, don't touch anything here at all -- not even read
      // `activeRef`, since whatever is now active belongs to a different Def/scope entirely and
      // has nothing to do with the view just deleted.
      if (scopeGenerationRef.current !== myScopeGeneration) return;
      // Read the LIVE state via `activeRef` (kept current every render), not `active` captured
      // by this callback's own closure -- if a stale instance of this exact function is what
      // ends up executing (e.g. invoked from an async UI handler whose own closure predates a
      // Def switch within the same scope), `active` here would still reflect whatever was active
      // when THIS closure was created, not the real current selection.
      const currentActive = activeRef.current;
      if (currentActive?.selectionRef?.origin === "custom" && currentActive.selectionRef.id === viewId) {
        selectDefaultView();
      }
    },
    [customFormViews, selectDefaultView],
  );

  return {
    applicable,
    availableViews,
    selectedView,
    effectiveHidden,
    hiddenCount: effectiveHidden.size,
    visibleTopLevelFieldIds,
    override: active?.override ?? null,
    hasDirtyOverride: !!active?.override?.isDirty,
    selectView,
    selectDefaultView,
    resetOverride,
    setOverrideHiddenFieldIds,
    saveOverrideAsCustomView,
    duplicateAsCustomView,
    createCustomView,
    renameCustomView,
    updateCustomView,
    deleteCustomView,
    getScopeKey,
    customViews: customFormViews.views,
    customViewsLoading: customFormViews.loading,
    customViewsWarning: customFormViews.warning,
    customViewsError: customFormViews.error,
    reloadCustomViews: customFormViews.reload,
    resetCustomViewStore: customFormViews.resetStore,
    persistWarning,
  };
}

function baseSchemaViewFor(view: ResolvedFormView): BaseSchemaViewReference | null {
  if (view.origin === "schema" && view.declaredOnDefType && view.source) {
    return {
      viewId: view.id,
      packId: view.source.packId,
      packVersion: view.source.packVersion,
      declaredOnDefType: view.declaredOnDefType,
    };
  }
  return null;
}

function buildFallbackDefault(defType: string): ResolvedFormView {
  // Identical shape to `buildAvailableFormViews`'s own Default View entry -- delegate instead
  // of duplicating the (translated) label literal here.
  return buildDefaultFormView(defType);
}
