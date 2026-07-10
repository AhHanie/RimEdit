import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
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
import type { SchemaCatalog } from "../../schema-catalog";
import type { XmlEditorSnapshot } from "../types/editorSession";
import type { XmlEdit, XmlEditContext } from "../types/xmlDocument";
import {
  buildEffectiveFieldOrder,
  buildNestedFieldOrders,
} from "../lib/schemaFieldOrder";

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

export function useXmlFormController({
  snapshot,
  catalog,
  selectedDefNodeId,
  commitEdits,
  clearPreview,
}: UseXmlFormControllerArgs): XmlFormApi {
  const selectedDef = useMemo(() => {
    const parsed = snapshot?.parsed;
    if (!parsed || parsed.defs.length === 0) return null;
    return (
      parsed.defs.find((d) => d.nodeId === selectedDefNodeId) ?? parsed.defs[0]
    );
  }, [snapshot, selectedDefNodeId]);

  const models = useMemo(() => {
    if (!selectedDef) return [];
    const activeCatalog = catalog ?? emptyCatalog;
    const defSchema = activeCatalog.defTypes[selectedDef.defType] ?? null;
    return buildFormFieldModels(selectedDef, defSchema, activeCatalog);
  }, [catalog, selectedDef]);

  const descriptors = useMemo(() => {
    if (!selectedDef) return [];
    const activeCatalog = catalog ?? emptyCatalog;
    const defSchema = activeCatalog.defTypes[selectedDef.defType] ?? null;
    return buildFormDescriptors(selectedDef, defSchema, activeCatalog);
  }, [catalog, selectedDef]);

  // Step 2: cheap, content-stable reset key. The schema/objectTypes only change when the
  // catalog is reloaded, which replaces the catalog object and bumps its WeakMap id - so we
  // avoid the per-commit `JSON.stringify(catalog.objectTypes/fields)` cost entirely.
  const catalogId = catalog ? getCatalogId(catalog) : 0;
  const resetKey = useMemo(() => {
    if (!snapshot || !selectedDef) return "empty";
    return `${snapshot.rawXml}:${selectedDef.nodeId}:${selectedDef.defType}:${catalogId}`;
  }, [snapshot, selectedDef, catalogId]);

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

  // Lazy store init - populated synchronously on first render so the very first paint has
  // fields (no flash) and no subscribers exist yet, so `initialize` notifies no one.
  const storeRef = useRef<FormFieldStore | null>(null);
  if (storeRef.current === null) {
    const s = new FormFieldStore();
    s.initialize(models, buildInitialFieldState(models, descriptors));
    storeRef.current = s;
    appliedResetKeyRef.current = resetKey;
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
    const isFormOriginated =
      lastCommittedRawXmlRef.current !== null &&
      newRawXml !== null &&
      newRawXml === lastCommittedRawXmlRef.current;
    const structurallySkippable =
      isFormOriginated &&
      !lastCommitStructuralRef.current &&
      newNodeCount !== null &&
      lastCommitPrevNodeCountRef.current !== null &&
      newNodeCount === lastCommitPrevNodeCountRef.current;

    appliedResetKeyRef.current = resetKey;
    // Consume the one-shot commit markers regardless of outcome.
    lastCommittedRawXmlRef.current = null;
    lastCommitStructuralRef.current = false;
    lastCommitPrevNodeCountRef.current = null;

    draftVersionRef.current = 0;

    if (structurallySkippable) {
      // Correctness guard for the optimistic skip: trust the form's in-memory state only if
      // it actually matches the re-parsed document. The invariant we depend on is that the
      // backend does NOT rewrite field *values* for a non-structural, count-preserving edit
      // (formatting/ordering differences are fine - they don't change field values). If it
      // ever did (e.g. canonicalizing a number), the freshly-derived values would differ and
      // we fall back to a full rebuild rather than silently showing stale values.
      const fresh = buildInitialFieldState(models, descriptors);
      if (storeValuesMatchFreshBuild(store, fresh)) return;
      store.reset(models, fresh);
      return;
    }

    store.reset(models, buildInitialFieldState(models, descriptors));
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
            // Record what the form itself committed so the reset path can skip the rebuild.
            lastCommittedRawXmlRef.current = rawXml;
            lastCommitStructuralRef.current = structural;
            lastCommitPrevNodeCountRef.current = prevNodeCount;
            store.applyCommit(dirtyIds, committedValuesByFieldId);
            if (draftVersionRef.current !== flushVersion) {
              const message =
                "Form changed while edits were being applied. Preview again.";
              clearPreviewRef.current();
              store.markCommitError(dirtyIds, message);
              throw new Error(message);
            }
            return rawXml;
          } catch (e) {
            const message = e instanceof Error ? e.message : String(e);
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
