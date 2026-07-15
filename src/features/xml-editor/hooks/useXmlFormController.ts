import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import { useTranslation } from "react-i18next";
import {
  buildFormDescriptors,
  buildFormFieldModels,
  formValueFromModel,
} from "../lib/formDescriptors";
import { FormFieldStore, type StoredFieldState } from "../lib/formFieldStore";
import {
  cloneFormValue,
  emptyFormValueForModel,
  fieldToXmlEdit,
  formValuesEqual,
  isStructuralEdit,
  validateFieldValue,
} from "../lib/formValues";
import type {
  FormFieldId,
  FormFieldModel,
  FormSnapshot,
  FormValue,
} from "../types/editorForm";
import type {
  DefTemplate,
  DefTypeSchema,
  FieldSchema,
  ObjectTypeSchema,
  SchemaCatalog,
  SchemaFormView,
  ValidationRule,
} from "../../schema-catalog";
import type { XmlEditorSnapshot } from "../types/editorSession";
import type { XmlEdit, XmlEditContext } from "../types/xmlDocument";
import {
  buildEffectiveFieldOrder,
  buildNestedFieldOrders,
} from "../lib/schemaFieldOrder";
import { formatError } from "../../../lib/formatError";
import { initI18n } from "../../../i18n";

// Re-export the pure form-value helpers so existing component imports keep working.
export {
  formValueToString,
  formValuesEqual,
  cloneFormValue,
  emptyFormValueForModel,
  scalarFormValue,
  listFormValue,
  booleanFormValue,
  enumFormValue,
  flagsFormValue,
  namedMapFormValue,
  typedReferenceListFormValue,
} from "../lib/formValues";

interface UseXmlFormControllerArgs {
  snapshot: XmlEditorSnapshot | null;
  catalog: SchemaCatalog | null;
  selectedDefNodeId: number | null;
  commitEdits: (
    edits: XmlEdit[],
    editContext?: XmlEditContext,
  ) => Promise<string>;
  clearPreview: () => void;
  /**
   * Form Views (issue 05, Plan.md section 7/9/10): canonical top-level Def schema field keys
   * (`descriptor.fieldPath[0]`, i.e. `DefTypeSchema.fields` keys) that should be rendered.
   * `undefined`/`null` means "no filter" - the existing unfiltered full form - and MUST
   * produce byte-identical behavior to omitting this argument entirely; no real caller
   * passes a value yet (issue 06's `useFormViews`/`XmlEditorPane` will own selection state
   * and supply this Set). Passed straight through to `buildFormDescriptors`/
   * `buildFormFieldModels`, which skip hidden top-level roots before any expensive nested
   * expansion. Changing this value is an explicit dependency of the descriptor/model
   * rebuild below (`resetKey`), so a visibility change rebuilds the store/models exactly
   * once from the *same* underlying XML snapshot - it never reparses XML, calls
   * `commitEdits`, or touches session/undo history.
   */
  visibleTopLevelFieldIds?: ReadonlySet<string> | null;
  /**
   * Form Views (issue 05, Plan.md section 7) focus-fallback hook. Called once per field id
   * that was focused immediately before a descriptor/model rebuild (e.g. a visibility
   * change hiding the focused field's top-level root) and is no longer present afterward.
   * This issue does not implement a fallback focus target itself - there is no selector/
   * customize control to focus yet - so it is exposed purely as a signal for the future
   * owner (issue 06/07's Form View selector/`FormViewManagerDialog`) to move DOM focus to
   * their own control when they wire this hook up.
   */
  onFocusedFieldHidden?: (fieldId: FormFieldId) => void;
}

export interface XmlFormActions {
  setFieldValue: (fieldId: FormFieldId, value: FormValue) => void;
  focusField: (fieldId: FormFieldId) => void;
  blurField: (fieldId: FormFieldId) => void;
  resetField: (fieldId: FormFieldId) => void;
  clearField: (fieldId: FormFieldId) => void;
  discardDrafts: () => void;
  flushField: (fieldId: FormFieldId) => Promise<string | null>;
  flushAll: () => Promise<string | null>;
}

export interface XmlFormApi {
  snapshot: FormSnapshot | null;
  /** Subscribable store for per-field rendering (each control subscribes to its own id). */
  store: FormFieldStore;
  actions: XmlFormActions;
  hasDraftChanges: boolean;
  hasPendingCommits: boolean;
  hasBlockingErrors: boolean;
  formError: string | null;
  setFieldValue: (fieldId: FormFieldId, value: FormValue) => void;
  focusField: (fieldId: FormFieldId) => void;
  blurField: (fieldId: FormFieldId) => void;
  resetField: (fieldId: FormFieldId) => void;
  clearField: (fieldId: FormFieldId) => void;
  discardDrafts: () => void;
  flushField: (fieldId: FormFieldId) => Promise<string | null>;
  flushAll: () => Promise<string | null>;
}

const emptyCatalog: SchemaCatalog = {
  formatVersion: 1,
  packs: [],
  defTypes: {},
  objectTypes: {},
};

// Short, content-stable id for a catalog, used in `resetKey` instead of the expensive
// per-render `JSON.stringify(catalog.objectTypes/fields)`.
//
// Correctness contract: the id must change when the schema content changes (so a reload /
// game-version switch rebuilds the form) and stay the same when it doesn't - even if the
// catalog *object* is re-created with identical content (which happens in tests and, per
// the findings, on some real render paths). Keying on bare object identity churned the key
// every render and looped the rebuild; keying on a content signature avoids that.
//
// Cost: cached by object identity, so the stable-identity case (the real app, where the
// catalog lives in useState) computes the signature exactly once and is O(1) thereafter.
// A churned-but-identical catalog recomputes the signature but maps to the same id.
const catalogIdByRef = new WeakMap<SchemaCatalog, number>();
const catalogIdBySignature = new Map<string, number>();
let nextCatalogId = 1;
function getCatalogId(catalog: SchemaCatalog): number {
  const cached = catalogIdByRef.get(catalog);
  if (cached !== undefined) return cached;
  const signature =
    JSON.stringify(catalog.defTypes) +
    "|" +
    JSON.stringify(catalog.objectTypes);
  let id = catalogIdBySignature.get(signature);
  if (id === undefined) {
    id = nextCatalogId++;
    catalogIdBySignature.set(signature, id);
  }
  catalogIdByRef.set(catalog, id);
  return id;
}

// `getCatalogId` above intentionally reflects the FULL catalog
// content, including `label`/`description`/`message` - that is exactly right for `models`/
// `descriptors` (they must re-resolve translated text on a locale switch)
// but wrong for `docKey`'s draft-preservation decision below. `load_schema_catalog` resolves
// `label`/`description`/`message` through the active locale's sidecar overlay server-side
// (see `src-tauri/src/schema_pack/merge.rs`'s "locale sidecar" handling) and issue 06 made the
// catalog re-fetch itself locale-aware, so a locale switch legitimately produces a brand-new
// `SchemaCatalog` object whose translated text differs while its field set/shape does not. If
// `docKey` used `getCatalogId` directly, that new-object-same-structure catalog would look like
// a genuine structural/document change and wipe any in-progress, not-yet-flushed draft field
// edit purely because its translated label text changed underneath it.
//
// This id strips exactly the `label`/`description`/`message` PROPERTIES the locale sidecar
// overlay rewrites (see `merge.rs`'s per-scalar sidecar-ownership tracking for `DefTypeSchema`/
// `FieldSchema`/`ObjectTypeSchema`/`DefTemplate`/`SchemaFormView`), so a locale-only catalog
// reload maps to the SAME structural id, while any change to the actual field set/types/shape/
// order/inheritance still changes it like before.
//
// Deliberately NOT a blind `JSON.stringify(value, replacer)` keyed on the literal strings
// "label"/"description"/"message": `fields`/`defTypes`/`templates`/etc. are all
// `Record<string, X>` dictionaries, so a REAL RimWorld Def field is very often itself named
// `description` (e.g. `ThingDef.description`) - a key-name-blind replacer would strip that
// entire field's schema (not just its `label`/`description` text) purely because its field NAME
// collides with the text-property name, silently collapsing a genuine structural difference
// (e.g. `description` becoming `required`) into "no change". Each `stripX` helper below instead
// destructures the exact, known `label`/`description`/`message` PROPERTY off a specific typed
// shape - never a dictionary key - so field/def-type/template/view names are never touched.
function stripFieldSchemaText(field: FieldSchema): Omit<FieldSchema, "label" | "description"> {
  const { label: _label, description: _description, ...rest } = field;
  return rest;
}
function stripDefTemplateText(template: DefTemplate): Omit<DefTemplate, "label" | "description"> {
  const { label: _label, description: _description, ...rest } = template;
  return rest;
}
function stripValidationRuleText(rule: ValidationRule): Omit<ValidationRule, "message"> {
  const { message: _message, ...rest } = rule;
  return rest;
}
function stripSchemaFormViewText(
  view: SchemaFormView,
): Omit<SchemaFormView, "label" | "description"> {
  const { label: _label, description: _description, ...rest } = view;
  return rest;
}
function mapRecordValues<T, R>(
  record: Record<string, T>,
  fn: (value: T) => R,
): Record<string, R> {
  const result: Record<string, R> = {};
  for (const key of Object.keys(record)) result[key] = fn(record[key]);
  return result;
}
function stripDefTypeSchemaText(defType: DefTypeSchema): unknown {
  const {
    label: _label,
    description: _description,
    fields,
    templates,
    validationRules,
    formViews,
    ...rest
  } = defType;
  return {
    ...rest,
    fields: mapRecordValues(fields, stripFieldSchemaText),
    templates: templates ? mapRecordValues(templates, stripDefTemplateText) : templates,
    validationRules: validationRules
      ? mapRecordValues(validationRules, stripValidationRuleText)
      : validationRules,
    formViews: formViews ? mapRecordValues(formViews, stripSchemaFormViewText) : formViews,
  };
}
function stripObjectTypeSchemaText(objectType: ObjectTypeSchema): unknown {
  const { label: _label, description: _description, fields, ...rest } = objectType;
  return { ...rest, fields: mapRecordValues(fields, stripFieldSchemaText) };
}
const structuralCatalogIdByRef = new WeakMap<SchemaCatalog, number>();
const structuralCatalogIdBySignature = new Map<string, number>();
let nextStructuralCatalogId = 1;
function getStructuralCatalogId(catalog: SchemaCatalog): number {
  const cached = structuralCatalogIdByRef.get(catalog);
  if (cached !== undefined) return cached;
  const structuralDefTypes = mapRecordValues(catalog.defTypes, stripDefTypeSchemaText);
  const structuralObjectTypes = mapRecordValues(catalog.objectTypes, stripObjectTypeSchemaText);
  const signature =
    JSON.stringify(structuralDefTypes) + "|" + JSON.stringify(structuralObjectTypes);
  let id = structuralCatalogIdBySignature.get(signature);
  if (id === undefined) {
    id = nextStructuralCatalogId++;
    structuralCatalogIdBySignature.set(signature, id);
  }
  structuralCatalogIdByRef.set(catalog, id);
  return id;
}

// Same content-stable-id pattern as `getCatalogId`, applied to the Form Views visibility
// set (issue 05). `0` is reserved for "no filter" (the argument omitted/null), so the
// resetKey suffix is a fixed constant for every caller that doesn't pass this yet - it can
// never change across renders and therefore never changes today's rebuild behavior.
const visibilityIdByRef = new WeakMap<ReadonlySet<string>, number>();
const visibilityIdBySignature = new Map<string, number>();
let nextVisibilityId = 1;
function getVisibilityId(
  visible: ReadonlySet<string> | null | undefined,
): number {
  if (!visible) return 0;
  const cached = visibilityIdByRef.get(visible);
  if (cached !== undefined) return cached;
  // `JSON.stringify` quotes/escapes each element, so e.g. a single field literally named
  // `"a,b"` and the two fields `"a"`/`"b"` serialize differently (`["a,b"]` vs `["a","b"]`).
  // A plain `.join(",")` would collide those two distinct Sets onto the same signature and
  // silently fail to rebuild on a real visibility change.
  const signature = JSON.stringify([...visible].sort());
  let id = visibilityIdBySignature.get(signature);
  if (id === undefined) {
    id = nextVisibilityId++;
    visibilityIdBySignature.set(signature, id);
  }
  visibilityIdByRef.set(visible, id);
  return id;
}

/**
 * Form ids that are currently focused in `store` but absent from `nextModels` - i.e. the
 * field's descriptor was dropped by the upcoming rebuild (most commonly: its top-level
 * root just became hidden by a Form View visibility change). Must be read *before*
 * `store.reset(...)` replaces the store's field set.
 */
function focusedFieldIdsGoneAfterReset(
  store: FormFieldStore,
  nextModels: FormFieldModel[],
): FormFieldId[] {
  const nextIds = new Set(nextModels.map((m) => m.id));
  return store
    .getAllStored()
    .filter((f) => f.focused && !nextIds.has(f.model.id))
    .map((f) => f.model.id);
}

/**
 * Form Views (issue 05, Plan.md section 7/9 - "no value is discarded"): every field in
 * `store` with an uncommitted draft (a dirty edit or an explicit clear request), keyed by
 * its canonical `FormFieldId`. Must be read *before* `store.reset(...)` replaces the field
 * map, since a plain rebuild otherwise silently drops in-memory drafts in favor of the
 * freshly-XML-derived (clean) value - which is correct when the underlying document/def/
 * catalog actually changed, but wrong for a rebuild caused purely by a visibility change.
 */
function captureDirtyDrafts(
  store: FormFieldStore,
): Map<FormFieldId, StoredFieldState> {
  const drafts = new Map<FormFieldId, StoredFieldState>();
  for (const stored of store.getAllStored()) {
    if (stored.dirty || stored.clearRequested) {
      drafts.set(stored.model.id, stored);
    }
  }
  return drafts;
}

/**
 * Re-applies captured dirty drafts onto a freshly-XML-derived field-state map (mutated in
 * place), for every id that still exists in it - i.e. the field is visible again after the
 * rebuild. A draft whose id is absent from `freshFields` (the field is still hidden) is left
 * untouched in `pendingDrafts` so a *later* rebuild, whenever that field becomes visible
 * again, can restore it - nothing is discarded merely because a field was hidden in between.
 * A draft is removed from `pendingDrafts` once it has been re-applied, since the live store
 * (not this map) becomes the authoritative source of truth for that field from then on.
 */
function applyDirtyDrafts(
  freshFields: Map<FormFieldId, StoredFieldState>,
  pendingDrafts: Map<FormFieldId, StoredFieldState>,
): void {
  for (const [id, draft] of pendingDrafts) {
    const fresh = freshFields.get(id);
    if (!fresh) continue; // Still hidden - keep stashed for a future rebuild.
    freshFields.set(id, {
      ...fresh,
      value: draft.value,
      dirty: draft.dirty,
      touched: draft.touched,
      pending: draft.pending,
      error: draft.error,
      cachedValidationErrors: draft.cachedValidationErrors,
      clearRequested: draft.clearRequested,
    });
  }
  for (const id of [...pendingDrafts.keys()]) {
    if (freshFields.has(id)) pendingDrafts.delete(id);
  }
}

export function useXmlFormController({
  snapshot,
  catalog,
  selectedDefNodeId,
  commitEdits,
  clearPreview,
  visibleTopLevelFieldIds,
  onFocusedFieldHidden,
}: UseXmlFormControllerArgs): XmlFormApi {
  // `i18n.language` is a reactive dependency of `models`/`descriptors`
  // below (schema catalog `label`/`description` text and this hook's own generated readOnlyReason/
  // summary strings -- see `formDescriptors.ts`/`objectDescriptors.ts` -- are resolved through the
  // active i18next language at build time, not stored translation-neutral). Without this, a locale
  // switch that produces a structurally-identical catalog (same fields/shape, same `catalogId` --
  // see `getCatalogId`'s doc comment) would never rebuild the form, leaving stale prior-locale text
  // on screen until something else forced a rebuild. `useTranslation()` (no namespace) subscribes
  // this hook to i18next's language-changed event so `i18n.language` is a genuinely reactive value,
  // not read once at mount.
  const { i18n } = useTranslation();

  const selectedDef = useMemo(() => {
    const parsed = snapshot?.parsed;
    if (!parsed || parsed.defs.length === 0) return null;
    return (
      parsed.defs.find((d) => d.nodeId === selectedDefNodeId) ?? parsed.defs[0]
    );
  }, [snapshot, selectedDefNodeId]);

  // Step 2: cheap, content-stable ids. Computed before `models`/`descriptors` below so those
  // memos can depend on stable primitives instead of the raw `catalog`/`visibleTopLevelFieldIds`
  // references: a caller re-creating a content-equal catalog or
  // Set on every render (e.g. `new Set(someComputedArray)` inline) must not force the
  // expensive descriptor rebuild/nested expansion this filtering exists to avoid.
  const catalogId = catalog ? getCatalogId(catalog) : 0;
  // Structural-only counterpart of `catalogId` (see
  // `getStructuralCatalogId`'s doc comment) - unchanged by a locale-only catalog reload, used
  // only by `docKey` below.
  const structuralCatalogId = catalog ? getStructuralCatalogId(catalog) : 0;
  // Form Views (issue 05): `0` when no visibility filter is supplied, so this is a fixed
  // constant (never changes across renders) until a real caller passes a Set.
  const visibilityId = getVisibilityId(visibleTopLevelFieldIds);

  const models = useMemo(() => {
    if (!selectedDef) return [];
    const activeCatalog = catalog ?? emptyCatalog;
    const defSchema = activeCatalog.defTypes[selectedDef.defType] ?? null;
    return buildFormFieldModels(
      selectedDef,
      defSchema,
      activeCatalog,
      visibleTopLevelFieldIds,
    );
    // `catalog`/`visibleTopLevelFieldIds` are intentionally read via closure rather than
    // listed here - `catalogId`/`visibilityId` are their content-stable proxies, so a
    // content-equal-but-new-reference catalog/Set correctly does NOT retrigger this.
    // `i18n.language` IS listed directly (see this function's top-of-hook comment): it is
    // already a stable primitive, so it needs no content-stable proxy of its own.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [catalogId, selectedDef, visibilityId, i18n.language]);

  const descriptors = useMemo(() => {
    if (!selectedDef) return [];
    const activeCatalog = catalog ?? emptyCatalog;
    const defSchema = activeCatalog.defTypes[selectedDef.defType] ?? null;
    return buildFormDescriptors(
      selectedDef,
      defSchema,
      activeCatalog,
      visibleTopLevelFieldIds,
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [catalogId, selectedDef, visibilityId, i18n.language]);

  // `docKey` identifies the underlying document/def/catalog identity, deliberately excluding
  // visibility AND locale. Comparing it across renders (see `lastDocKeyRef` below) is how the
  // rebuild effect tells "the document didn't change, only the visibility filter or locale did"
  // apart from every other rebuild cause - those two cases share the same draft-preservation
  // semantics (see `isPureDisplayOnlyChange` below), which differ from a real document change.
  //
  // Uses `structuralCatalogId`, NOT `catalogId`, precisely so a
  // locale-only catalog reload (new `SchemaCatalog` object, same field set/shape, only
  // translated `label`/`description`/`message` text differs) does not itself change `docKey` -
  // see `getStructuralCatalogId`'s doc comment. A genuine structural catalog change (different
  // fields/types/shape/order/inheritance - e.g. a game-version or schema-pack switch) still
  // changes `docKey` exactly as before.
  const docKey = useMemo(() => {
    if (!snapshot || !selectedDef) return null;
    return `${snapshot.rawXml}:${selectedDef.nodeId}:${selectedDef.defType}:${structuralCatalogId}`;
  }, [snapshot, selectedDef, structuralCatalogId]);
  // Locale is included here (not in `docKey`) for the same reason visibility is: a locale switch
  // must still trigger the rebuild effect below (so the store is rebuilt from freshly-translated
  // `models`/`descriptors`), but it is a display-only change, not a document change, so it must
  // NOT discard in-progress dirty drafts the way a real document/def/catalog change does.
  //
  // `catalogId` (the FULL, non-structural signature) is also included here directly - not just
  // via `docKey` - so that a catalog content change that is NOT a locale switch (e.g. a
  // non-locale label edit coming from a different schema pack/game version, where
  // `structuralCatalogId` may legitimately stay the same) still triggers this rebuild effect,
  // exactly as it did before `docKey` switched to the structural id.
  const resetKey =
    docKey === null
      ? "empty"
      : `${docKey}:${visibilityId}:${i18n.language}:${catalogId}`;

  // Stable callbacks/values read inside effects and async commit handlers.
  const clearPreviewRef = useRef(clearPreview);
  clearPreviewRef.current = clearPreview;
  const commitEditsRef = useRef(commitEdits);
  commitEditsRef.current = commitEdits;
  const selectedDefRef = useRef(selectedDef);
  selectedDefRef.current = selectedDef;
  const catalogRef = useRef(catalog);
  catalogRef.current = catalog;
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;
  const onFocusedFieldHiddenRef = useRef(onFocusedFieldHidden);
  onFocusedFieldHiddenRef.current = onFocusedFieldHidden;

  const draftVersionRef = useRef(0);
  const pendingCommitRef = useRef<Promise<string | null>>(
    Promise.resolve(null),
  );

  // Step 4: markers recorded when the form commits, so the reset path can recognise a
  // form-originated content change and skip the redundant whole-form rebuild.
  const lastCommittedRawXmlRef = useRef<string | null>(null);
  const lastCommitStructuralRef = useRef(false);
  const lastCommitPrevNodeCountRef = useRef<number | null>(null);
  const appliedResetKeyRef = useRef<string | null>(null);

  // Form Views (issue 05), extended to also cover locale, with the comparison itself later
  // refined - see `isPureDisplayOnlyChange`'s doc comment: the
  // `docKey`/`i18n.language` last applied, so the rebuild effect can tell a pure visibility-,
  // locale-, or locale-driven-catalog-metadata-only rebuild apart from every other rebuild cause
  // (raw edit, undo/redo, def switch, structural catalog reload, form commit). Only a pure
  // display-only change preserves uncommitted drafts (see `draftOverridesRef` below); every
  // other cause discards stale drafts exactly as it already did before this feature existed.
  const lastDocKeyRef = useRef<string | null>(null);
  const lastLanguageRef = useRef<string | null>(null);
  // Field state stashed for a currently-hidden field across a pure visibility rebuild, keyed
  // by canonical FormFieldId. Usually an uncommitted (dirty/clearRequested) draft; if that
  // field's own commit later lands while it's still hidden, `flushFields` updates the entry
  // in place to the just-committed clean value instead of deleting it,
  // so the field reflects the real committed value - not the stale pre-commit draft - the
  // moment it becomes visible again, without waiting for a fresh document snapshot to arrive.
  // An entry is applied and removed the moment its field becomes visible again. Cleared
  // entirely on any real (non-visibility) rebuild and on an explicit `discardDrafts()` -
  // never resurrected across an actual document change or an explicit user discard.
  const draftOverridesRef = useRef<Map<FormFieldId, StoredFieldState>>(new Map());

  // Lazy store init - populated synchronously on first render so the very first paint has
  // fields (no flash) and no subscribers exist yet, so `initialize` notifies no one.
  const storeRef = useRef<FormFieldStore | null>(null);
  if (storeRef.current === null) {
    const s = new FormFieldStore();
    s.initialize(models, buildInitialFieldState(models, descriptors));
    storeRef.current = s;
    appliedResetKeyRef.current = resetKey;
    lastDocKeyRef.current = docKey;
    lastLanguageRef.current = i18n.language;
  }
  const store = storeRef.current;

  // Re-render the host only when aggregates (dirty/pending/error/formError) or the form
  // structure change - not on every per-field value edit (those re-render just their own
  // control via the store subscription in FormFieldControl).
  //
  // Measurement note: steady-state typing is O(1) (only the edited control re-renders). The
  // *transition* keystrokes that flip an aggregate - e.g. the first edit (clean→dirty) or one
  // that introduces/clears a validation error - change `aggregates`, so `formApi` identity
  // changes and the memoized XmlFormEditor re-renders once, rebuilding its element tree
  // (O(N) element construction). That stays cheap because every child FormFieldControl is
  // memo-blocked on stable (fieldId, store, actions) and does not re-render. So the
  // dirty-toggle keystroke is not strictly O(1) - worth knowing when reading longtask traces.
  const aggregates = useSyncExternalStore(
    store.subscribeAggregates,
    store.getAggregates,
  );
  useSyncExternalStore(store.subscribeStructure, store.getStructureVersion);

  // Rebuild (or, per Step 4, deliberately skip) the field store when the reset key changes.
  // Runs as a layout effect so the rebuilt store is committed + subscribers notified before
  // paint, avoiding a visible stale frame.
  useLayoutEffect(() => {
    if (appliedResetKeyRef.current === resetKey) return;

    const newRawXml = snapshotRef.current?.rawXml ?? null;
    const newNodeCount = snapshotRef.current?.parsed?.nodeCount ?? null;

    // Step 4: a form-originated commit produced exactly this rawXml. If it changed no
    // document structure (no structural edits AND node count unchanged), every parse-order
    // node id maps to the same element and the post-flush field state is already correct -
    // so the full rebuild is redundant work and we skip it. Anything else (raw edit,
    // undo/redo, def insert, file load, confirm-save, catalog reload, or a structural form
    // commit) rebuilds.
    // A locale switch alone must never take the `structurallySkippable`
    // fast path below. That path's guarantee is "field VALUES are provably unchanged, so skip the
    // rebuild" (`storeValuesMatchFreshBuild` only compares values, never labels/descriptions/
    // readOnlyReason text) -- correct for a raw/undo/commit-driven rebuild, but wrong here, since a
    // locale switch is precisely a display-text-only change this rebuild exists to pick up.
    const localeChanged = lastLanguageRef.current !== i18n.language;

    const isFormOriginated =
      lastCommittedRawXmlRef.current !== null &&
      newRawXml !== null &&
      newRawXml === lastCommittedRawXmlRef.current;
    const structurallySkippable =
      isFormOriginated &&
      !lastCommitStructuralRef.current &&
      newNodeCount !== null &&
      lastCommitPrevNodeCountRef.current !== null &&
      newNodeCount === lastCommitPrevNodeCountRef.current &&
      !localeChanged;

    // Form Views (issue 05, Plan.md section 7/9 - "no value is discarded"), extended to
    // locale: this rebuild is caused *purely* by a display-only change
    // (the visibility filter, the active locale, or a locale-only
    // catalog re-fetch landing on a LATER render than the `i18n.language` flip that triggered it,
    // see `resetKey`'s `catalogId` term) exactly when the underlying document/def/catalog
    // STRUCTURAL identity (`docKey`, keyed on `structuralCatalogId`) is exactly what was last
    // applied. Mutually exclusive with `isFormOriginated`/`structurallySkippable`: a form commit
    // always changes `snapshot.rawXml`, which is part of `docKey`.
    //
    // Deliberately does NOT also require "visibility changed OR
    // locale changed on THIS render" (the original condition, which compared against a
    // now-removed `lastVisibilityIdRef`). `resetKey` can now change for a fourth reason beyond
    // docKey/visibility/locale - `catalogId` (the FULL, non-structural signature) - when a
    // locale-aware catalog re-fetch resolves on a render where `i18n.language` itself already
    // stabilized on a PRIOR render (see the test in `useXmlFormController.test.tsx` titled
    // "preserves an in-progress dirty draft when a locale switch delivers a new catalog
    // object..."). In that case neither `visibilityId` nor `localeChanged` changed on THIS
    // render, but `docKey` (structural) is still unchanged from last applied, so it alone is both
    // necessary and sufficient: whenever the effect body runs at all (past the early-return
    // above), at least one of docKey/visibilityId/i18n.language/catalogId must differ from what
    // was last applied - if docKey (structural) didn't, the underlying document/field-structure
    // genuinely didn't change, regardless of which of the other three caused this particular
    // render.
    const isPureDisplayOnlyChange =
      lastDocKeyRef.current !== null && lastDocKeyRef.current === docKey;

    appliedResetKeyRef.current = resetKey;
    lastDocKeyRef.current = docKey;
    lastLanguageRef.current = i18n.language;

    // `draftVersionRef` is the guard `flushFields`
    // uses, after awaiting `commitEdits`, to detect "the draft/document generation changed
    // while this commit was in flight" and treat a resolved commit as stale (throwing "Form
    // changed while edits were being applied"). Bumping it here is only correct when the
    // rebuild reflects a REAL document/def/catalog change - a pure visibility toggle alters
    // no field's value (see the draft-preservation branch below), so an in-flight flush from
    // before the toggle is still valid and must not be invalidated purely because the user
    // switched views mid-flight. The same reasoning applies to the Step 4 one-shot commit
    // markers: consuming them here for a rebuild that hasn't actually observed the commit's
    // rawXml yet would make the *next*, real rebuild wrongly pay for a full rebuild instead
    // of the Step 4 fast path.
    //
    // This used to RESET `draftVersionRef` to the fixed constant 0, which is
    // a genuine (pre-existing, not introduced by Form Views) race: an in-flight flush that
    // had captured version 1 before this reset, followed by exactly one fresh edit after the
    // rebuild (bumping the counter from 0 back to 1), would collide with the stale flush's
    // captured value and be wrongly accepted as still-current. Incrementing instead of
    // resetting makes this a genuinely monotonic generation counter - it can never loop back
    // to a value an in-flight flush already captured, however many edits/rebuilds occur.
    if (!isPureDisplayOnlyChange) {
      lastCommittedRawXmlRef.current = null;
      lastCommitStructuralRef.current = false;
      lastCommitPrevNodeCountRef.current = null;
      draftVersionRef.current += 1;
    }

    if (structurallySkippable) {
      // Correctness guard for the optimistic skip: trust the form's in-memory state only if
      // it actually matches the re-parsed document. The invariant we depend on is that the
      // backend does NOT rewrite field *values* for a non-structural, count-preserving edit
      // (formatting/ordering differences are fine - they don't change field values). If it
      // ever did (e.g. canonicalizing a number), the freshly-derived values would differ and
      // we fall back to a full rebuild rather than silently showing stale values.
      const fresh = buildInitialFieldState(models, descriptors);
      if (storeValuesMatchFreshBuild(store, fresh)) return;
      const goneFocusedIds = focusedFieldIdsGoneAfterReset(store, models);
      // A real document change (a form commit) - stale hidden-field drafts no longer apply.
      draftOverridesRef.current.clear();
      store.reset(models, fresh);
      for (const id of goneFocusedIds) onFocusedFieldHiddenRef.current?.(id);
      return;
    }

    // Form Views (issue 05): capture focused-but-about-to-disappear field ids *before*
    // `store.reset` replaces the field set, so the fallback hook can still be told which
    // field lost focus (e.g. its top-level root was just hidden by a visibility change).
    const goneFocusedIds = focusedFieldIdsGoneAfterReset(store, models);
    const fresh = buildInitialFieldState(models, descriptors);

    if (isPureDisplayOnlyChange) {
      // Stash every currently-dirty field's draft (not just ones whose own visibility just
      // changed - a full rebuild would otherwise also wipe an unrelated visible field's
      // in-progress edit), then re-apply whichever of those - old or newly stashed - now
      // have a home in `fresh`. Anything still hidden stays stashed for a later rebuild.
      for (const [id, draft] of captureDirtyDrafts(store)) {
        draftOverridesRef.current.set(id, draft);
      }
      applyDirtyDrafts(fresh, draftOverridesRef.current);
    } else {
      // A real document/def/catalog change: any stashed hidden-field draft no longer
      // corresponds to the current document and must not resurface later.
      draftOverridesRef.current.clear();
    }

    store.reset(models, fresh);
    for (const id of goneFocusedIds) onFocusedFieldHiddenRef.current?.(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resetKey]);

  const setFieldValue = useCallback(
    (fieldId: FormFieldId, value: FormValue) => {
      const stored = store.getStored(fieldId);
      if (!stored || stored.model.readonly) return;
      clearPreviewRef.current();
      draftVersionRef.current += 1;
      store.setValue(fieldId, value, validateFieldValue(stored.model, value));
    },
    [store],
  );

  const focusField = useCallback(
    (fieldId: FormFieldId) => store.setFocused(fieldId, true),
    [store],
  );
  const blurField = useCallback(
    (fieldId: FormFieldId) => store.setFocused(fieldId, false),
    [store],
  );

  const resetField = useCallback(
    (fieldId: FormFieldId) => {
      const stored = store.getStored(fieldId);
      if (!stored) return;
      clearPreviewRef.current();
      draftVersionRef.current += 1;
      const value = cloneFormValue(stored.initialValue);
      store.resetField(fieldId, value, validateFieldValue(stored.model, value));
    },
    [store],
  );

  const clearField = useCallback(
    (fieldId: FormFieldId) => {
      const stored = store.getStored(fieldId);
      if (!stored || stored.model.readonly) return;
      clearPreviewRef.current();
      draftVersionRef.current += 1;
      const emptyValue = emptyFormValueForModel(stored.model);
      store.clearField(
        fieldId,
        emptyValue,
        validateFieldValue(stored.model, emptyValue),
      );
    },
    [store],
  );

  const discardDrafts = useCallback(() => {
    clearPreviewRef.current();
    draftVersionRef.current += 1;
    // `store.discardAll` only resets fields currently LIVE in the
    // store. A field hidden by a Form View visibility change while dirty has its draft
    // stashed in `draftOverridesRef`, not in the live store, so an explicit "discard all
    // drafts" action must also drop it here - otherwise the stale draft would silently
    // resurrect once that field becomes visible again, contradicting the user's discard.
    draftOverridesRef.current.clear();
    store.discardAll((field) => {
      const value = cloneFormValue(field.initialValue);
      return { value, errors: validateFieldValue(field.model, value) };
    });
  }, [store]);

  const flushFields = useCallback(
    async (fieldIds: FormFieldId[]) => {
      const idSet = new Set(fieldIds);
      const currentFields = store
        .getAllStored()
        .filter((field) => idSet.has(field.model.id));
      const dirtyFields = currentFields.filter(
        (field) =>
          !field.model.readonly &&
          (field.clearRequested ||
            !formValuesEqual(field.value, field.initialValue)),
      );
      if (dirtyFields.length === 0) return null;

      // Skip validation for explicitly-cleared fields - removing a required field is allowed;
      // backend diagnostics will surface the missing-required error post-save.
      const invalid = dirtyFields.find(
        (field) =>
          !field.clearRequested &&
          validateFieldValue(field.model, field.value).length > 0,
      );
      if (invalid) {
        const message = validateFieldValue(invalid.model, invalid.value)[0];
        store.setFieldError(invalid.model.id, message);
        throw new Error(message);
      }

      const flushVersion = draftVersionRef.current;
      const committedValuesByFieldId = new Map(
        dirtyFields.map((field) => [
          field.model.id,
          cloneFormValue(field.value),
        ]),
      );
      const fieldsToCommit = dirtyFields.map((field) => ({
        ...field,
        value: cloneFormValue(field.value),
      }));

      const edits = fieldsToCommit
        .sort((a, b) => a.model.order - b.model.order)
        .flatMap(fieldToXmlEdit);

      if (edits.length === 0) return null;

      const dirtyIds = dirtyFields.map((field) => field.model.id);
      // Step 4 markers captured synchronously before the await.
      const structural = edits.some(isStructuralEdit);
      const prevNodeCount = snapshotRef.current?.parsed?.nodeCount ?? null;

      store.setFormError(null);
      store.markPending(dirtyIds);

      const currentSelectedDef = selectedDefRef.current;
      const currentCatalog = catalogRef.current;
      let editContext: XmlEditContext | undefined;
      if (currentSelectedDef && currentCatalog) {
        const fieldOrder = buildEffectiveFieldOrder(
          currentSelectedDef.defType,
          currentCatalog,
        );
        const nestedFieldOrders = buildNestedFieldOrders(
          currentSelectedDef.defType,
          currentCatalog,
        );
        const hasNestedOrders = Object.keys(nestedFieldOrders).length > 0;
        if (fieldOrder.length > 0 || hasNestedOrders) {
          editContext = {
            fieldOrder,
            ...(hasNestedOrders ? { nestedFieldOrders } : {}),
          };
        }
      }

      const run = pendingCommitRef.current
        .catch(() => null)
        .then(async () => {
          try {
            const rawXml = await commitEditsRef.current(edits, editContext);

            // Check staleness BEFORE any of the following side effects
            // (Step 4 markers, `store.applyCommit`, the hidden-draft stash update) rather
            // than applying them first and only afterward deciding whether to also flag an
            // error. `draftVersionRef` is a monotonic generation counter (bumped on every
            // per-edit action and every real, non-visibility rebuild - see the rebuild
            // effect above - never reset to a fixed value), so this comparison can never
            // be fooled by a value collision: if it doesn't match, the document/store has
            // moved on since this flush started and this commit's result must not be
            // written into it, even if some individual field's value coincidentally still
            // matches what was committed.
            if (draftVersionRef.current !== flushVersion) {
              // Not a React component, and this callback can run before/without an
              // `I18nextProvider`/`LocaleProvider` ancestor -- resolve from the app-wide i18next
              // singleton directly, same as `src/features/xml-editor/lib/objectDescriptors.ts`,
              // rather than `useTranslation()` (which returns a non-functional stub, echoing the
              // raw key back, outside a provider tree).
              const message = initI18n().t(
                "editor:formEditor.staleFormOnCommit",
                "Form changed while edits were being applied. Preview again.",
              );
              clearPreviewRef.current();
              store.markCommitError(dirtyIds, message);
              throw new Error(message);
            }

            // Record what the form itself committed so the reset path can skip the rebuild.
            lastCommittedRawXmlRef.current = rawXml;
            lastCommitStructuralRef.current = structural;
            lastCommitPrevNodeCountRef.current = prevNodeCount;
            store.applyCommit(dirtyIds, committedValuesByFieldId);
            // `store.applyCommit` only updates a field that is
            // currently LIVE in the store - it silently skips one hidden by a Form View
            // visibility change at commit time (that field isn't in the store's field map
            // at all). Update this commit's fields' stashed override (if any) to the
            // just-committed, now-clean value too, so a hidden field shown again later
            // reflects what was actually committed - not the stale pre-commit draft that
            // was stashed at the moment it was hidden.
            for (const id of dirtyIds) {
              const stashed = draftOverridesRef.current.get(id);
              if (!stashed) continue;
              const committedValue = committedValuesByFieldId.get(id);
              if (committedValue === undefined) continue;
              draftOverridesRef.current.set(id, {
                ...stashed,
                value: committedValue,
                initialValue: committedValue,
                dirty: false,
                pending: false,
                clearRequested: false,
                error: null,
                cachedValidationErrors: validateFieldValue(
                  stashed.model,
                  committedValue,
                ),
              });
            }
            return rawXml;
          } catch (e) {
            const message = formatError(e);
            store.markCommitError(dirtyIds, message);
            throw e;
          }
        });

      pendingCommitRef.current = run;
      return run;
    },
    [store],
  );

  const flushField = useCallback(
    (fieldId: FormFieldId) => flushFields([fieldId]),
    [flushFields],
  );

  const flushAll = useCallback(
    () => flushFields(store.getFieldIds()),
    [flushFields, store],
  );

  const actions = useMemo<XmlFormActions>(
    () => ({
      setFieldValue,
      focusField,
      blurField,
      resetField,
      clearField,
      discardDrafts,
      flushField,
      flushAll,
    }),
    [
      setFieldValue,
      focusField,
      blurField,
      resetField,
      clearField,
      discardDrafts,
      flushField,
      flushAll,
    ],
  );

  // `fields` is a live getter over the store, so reading `snapshot.fields` always reflects
  // the current per-field state - even after a value edit that didn't re-render the host
  // (those edits re-render only their own control). The snapshot object identity stays
  // stable (memoized on selectedDef) so the memoized form isn't re-rendered needlessly.
  const apiSnapshot = useMemo<FormSnapshot | null>(
    () =>
      selectedDef
        ? {
            defNodeId: selectedDef.nodeId,
            get fields() {
              return store.getOrderedFieldStates();
            },
          }
        : null,
    [selectedDef, store],
  );

  // Memoize the returned API so its identity is stable across host re-renders that don't
  // change any input (e.g. a saveBusy toggle from the session hook). XmlFormEditor is
  // React.memo'd on this object, so a fresh object here would re-render the whole form.
  return useMemo<XmlFormApi>(
    () => ({
      snapshot: apiSnapshot,
      store,
      actions,
      hasDraftChanges: aggregates.hasDraftChanges,
      hasPendingCommits: aggregates.hasPendingCommits,
      hasBlockingErrors: aggregates.hasBlockingErrors,
      formError: aggregates.formError,
      setFieldValue,
      focusField,
      blurField,
      resetField,
      clearField,
      discardDrafts,
      flushField,
      flushAll,
    }),
    [
      apiSnapshot,
      store,
      actions,
      aggregates,
      setFieldValue,
      focusField,
      blurField,
      resetField,
      clearField,
      discardDrafts,
      flushField,
      flushAll,
    ],
  );
}

function buildInitialFieldState(
  models: FormFieldModel[],
  descriptors: ReturnType<typeof buildFormDescriptors>,
): Map<FormFieldId, StoredFieldState> {
  const descriptorsByFieldPath = new Map(
    descriptors.map((descriptor) => [
      descriptor.fieldPath.join("."),
      descriptor,
    ]),
  );
  const next = new Map<FormFieldId, StoredFieldState>();
  for (const model of models) {
    const descriptor = descriptorsByFieldPath.get(model.fieldPath.join("."));
    const value = formValueFromModel(model, descriptor?.value);
    const cachedValidationErrors = validateFieldValue(model, value);
    next.set(model.id, {
      model,
      value,
      initialValue: cloneFormValue(value),
      dirty: false,
      touched: false,
      focused: false,
      pending: false,
      error: null,
      cachedValidationErrors,
      clearRequested: false,
    });
  }
  return next;
}

/**
 * Whether the store's current per-field values match a freshly-derived build from the
 * re-parsed document. Used to validate the optimistic skip-rebuild path (Step 4): if any
 * field's value differs - or the id set differs - the form's in-memory state has diverged
 * from the file and we must rebuild rather than skip.
 */
function storeValuesMatchFreshBuild(
  store: FormFieldStore,
  fresh: Map<FormFieldId, StoredFieldState>,
): boolean {
  if (store.getFieldIds().length !== fresh.size) return false;
  for (const [id, freshStored] of fresh) {
    const current = store.getStored(id);
    if (!current) return false;
    if (!formValuesEqual(current.value, freshStored.value)) return false;
  }
  return true;
}
