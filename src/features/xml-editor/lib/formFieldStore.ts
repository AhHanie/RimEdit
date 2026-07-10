import type {
  FormFieldId,
  FormFieldModel,
  FormFieldState,
  FormValue,
} from "../types/editorForm";
import { formValuesEqual } from "./formValues";

/**
 * Internal per-field record. A superset of the public `FormFieldState` plus the cached
 * validation errors (stored so aggregates and rendering don't re-run validation).
 */
export interface StoredFieldState {
  model: FormFieldModel;
  value: FormValue;
  initialValue: FormValue;
  dirty: boolean;
  touched: boolean;
  focused: boolean;
  pending: boolean;
  error: string | null;
  cachedValidationErrors: string[];
  clearRequested: boolean;
}

export interface FieldAggregates {
  hasDraftChanges: boolean;
  hasPendingCommits: boolean;
  hasBlockingErrors: boolean;
  formError: string | null;
}

type Listener = () => void;

const EMPTY_AGGREGATES: FieldAggregates = {
  hasDraftChanges: false,
  hasPendingCommits: false,
  hasBlockingErrors: false,
  formError: null,
};

/**
 * Subscribable store for the XML form's per-field draft state.
 *
 * The whole point: a keystroke notifies only the edited field's subscribers, so only
 * that one control re-renders - independent of form size. The host hook subscribes to
 * coarse `aggregates` (dirty/pending/error/formError) and a `structureVersion` so it
 * re-renders only when those change, not on every value edit. `XmlFormEditor` subscribes
 * to the stable ordered `models` list; each `FormFieldControl` subscribes to its own id.
 *
 * Designed for `useSyncExternalStore`: every `getXxx` returns a value whose identity is
 * stable until that slice actually changes.
 */
export class FormFieldStore {
  private fields = new Map<FormFieldId, StoredFieldState>();
  private models: FormFieldModel[] = [];
  private stateCache = new Map<FormFieldId, FormFieldState>();
  private orderedCache: FormFieldState[] | null = null;

  private fieldListeners = new Map<FormFieldId, Set<Listener>>();
  private structureListeners = new Set<Listener>();
  private aggregateListeners = new Set<Listener>();

  private aggregates: FieldAggregates = EMPTY_AGGREGATES;
  private formError: string | null = null;
  private structureVersion = 0;

  // ---- structure (ordered models) ----

  getModels = (): FormFieldModel[] => this.models;

  getStructureVersion = (): number => this.structureVersion;

  subscribeStructure = (cb: Listener): (() => void) => {
    this.structureListeners.add(cb);
    return () => {
      this.structureListeners.delete(cb);
    };
  };

  // ---- per-field state ----

  getFieldState = (id: FormFieldId): FormFieldState | undefined => {
    const cached = this.stateCache.get(id);
    if (cached) return cached;
    const stored = this.fields.get(id);
    if (!stored) return undefined;
    const fs = toFieldState(stored);
    this.stateCache.set(id, fs);
    return fs;
  };

  subscribeField = (id: FormFieldId, cb: Listener): (() => void) => {
    let set = this.fieldListeners.get(id);
    if (!set) {
      set = new Set();
      this.fieldListeners.set(id, set);
    }
    set.add(cb);
    return () => {
      set!.delete(cb);
    };
  };

  /** Ordered field-state list (model order). Identity stable until a field or structure changes. */
  getOrderedFieldStates = (): FormFieldState[] => {
    if (this.orderedCache) return this.orderedCache;
    const result = this.models
      .map((m) => this.getFieldState(m.id))
      .filter((f): f is FormFieldState => !!f);
    this.orderedCache = result;
    return result;
  };

  // ---- aggregates ----

  getAggregates = (): FieldAggregates => this.aggregates;

  subscribeAggregates = (cb: Listener): (() => void) => {
    this.aggregateListeners.add(cb);
    return () => {
      this.aggregateListeners.delete(cb);
    };
  };

  // ---- reads for the controller's flush logic ----

  getStored = (id: FormFieldId): StoredFieldState | undefined =>
    this.fields.get(id);

  getAllStored = (): StoredFieldState[] => [...this.fields.values()];

  getFieldIds = (): FormFieldId[] => [...this.fields.keys()];

  // ---- initial population (no notify; used on first mount before any subscribers) ----

  initialize(
    models: FormFieldModel[],
    fields: Map<FormFieldId, StoredFieldState>,
  ): void {
    this.models = orderedModels(models);
    this.fields = fields;
    this.stateCache = new Map();
    this.orderedCache = null;
    this.formError = null;
    this.aggregates = computeAggregates(fields, null);
    this.structureVersion += 1;
  }

  /**
   * Rebuild from a fresh model/field set (external change: undo/redo, raw edit, def
   * insert, file load, catalog reload). Preserves model + field-state identity for fields
   * that are unchanged from the previous build (Step 5), and notifies only the fields that
   * actually changed, plus structure and aggregate subscribers.
   */
  reset(
    nextModels: FormFieldModel[],
    nextFields: Map<FormFieldId, StoredFieldState>,
  ): void {
    const prevFields = this.fields;
    const prevModelById = new Map(this.models.map((m) => [m.id, m]));

    const changedIds: FormFieldId[] = [];
    const mergedFields = new Map<FormFieldId, StoredFieldState>();

    for (const [id, nextStored] of nextFields) {
      const prevModel = prevModelById.get(id);
      // Reuse the previous model object identity when structurally identical so child
      // memos and the ordered-models array stay reference-stable across the rebuild.
      const model =
        prevModel && modelsEqual(prevModel, nextStored.model)
          ? prevModel
          : nextStored.model;
      const stored: StoredFieldState =
        model === nextStored.model ? nextStored : { ...nextStored, model };

      const prevStored = prevFields.get(id);
      if (prevStored && storedFieldsEqual(prevStored, stored)) {
        // Unchanged - keep previous identities (stored + cached FormFieldState).
        mergedFields.set(id, prevStored);
      } else {
        mergedFields.set(id, stored);
        this.stateCache.delete(id);
        changedIds.push(id);
      }
    }

    // Drop caches for removed fields.
    for (const id of prevFields.keys()) {
      if (!mergedFields.has(id)) {
        this.stateCache.delete(id);
        changedIds.push(id);
      }
    }

    this.fields = mergedFields;
    this.models = reuseModelArray(orderedModels(nextModels), prevModelById);
    this.orderedCache = null;
    this.formError = null;
    this.structureVersion += 1;

    const nextAggregates = computeAggregates(mergedFields, this.formError);
    const aggregatesChanged = !aggregatesEqual(this.aggregates, nextAggregates);
    this.aggregates = nextAggregates;

    this.notifyStructure();
    this.notifyFields(changedIds);
    if (aggregatesChanged) this.notifyAggregates();
  }

  // ---- mutations ----

  /** Mark the field as explicitly cleared: set empty value, set clearRequested, recompute dirty. Returns false if read-only/missing. */
  clearField(
    id: FormFieldId,
    emptyValue: FormValue,
    errors: string[],
  ): boolean {
    const cur = this.fields.get(id);
    if (!cur || cur.model.readonly) return false;
    const dirty =
      cur.model.sourceNodeId !== null ||
      !formValuesEqual(cur.initialValue, emptyValue) ||
      !formValuesEqual(cur.value, emptyValue);
    this.fields.set(id, {
      ...cur,
      value: emptyValue,
      error: null,
      dirty,
      cachedValidationErrors: errors,
      clearRequested: true,
    });
    this.formError = null;
    this.invalidate(id);
    this.notifyFields([id]);
    this.refreshAggregates();
    return true;
  }

  /** Apply a value edit. Clears form error and recomputes dirty/validation. Returns false if read-only/missing. */
  setValue(id: FormFieldId, value: FormValue, errors: string[]): boolean {
    const cur = this.fields.get(id);
    if (!cur || cur.model.readonly) return false;
    this.fields.set(id, {
      ...cur,
      value,
      error: null,
      dirty: !formValuesEqual(value, cur.initialValue),
      cachedValidationErrors: errors,
      clearRequested: false,
    });
    this.formError = null;
    this.invalidate(id);
    this.notifyFields([id]);
    this.refreshAggregates();
    return true;
  }

  setFocused(id: FormFieldId, focused: boolean): void {
    const cur = this.fields.get(id);
    if (!cur) return;
    this.fields.set(id, {
      ...cur,
      focused,
      touched: focused ? cur.touched : true,
    });
    this.invalidate(id);
    this.notifyFields([id]);
  }

  resetField(id: FormFieldId, value: FormValue, errors: string[]): void {
    const cur = this.fields.get(id);
    if (!cur) return;
    this.fields.set(id, {
      ...cur,
      value,
      error: null,
      dirty: false,
      cachedValidationErrors: errors,
      clearRequested: false,
    });
    this.invalidate(id);
    this.notifyFields([id]);
    this.refreshAggregates();
  }

  /** Discard all drafts back to their committed values. `compute` returns the reset value+errors for a field. */
  discardAll(
    compute: (field: StoredFieldState) => {
      value: FormValue;
      errors: string[];
    },
  ): void {
    const changed: FormFieldId[] = [];
    for (const [id, cur] of this.fields) {
      const { value, errors } = compute(cur);
      this.fields.set(id, {
        ...cur,
        value,
        focused: false,
        pending: false,
        error: null,
        dirty: false,
        cachedValidationErrors: errors,
        clearRequested: false,
      });
      this.invalidate(id);
      changed.push(id);
    }
    this.formError = null;
    this.notifyFields(changed);
    this.refreshAggregates();
  }

  markPending(ids: FormFieldId[]): void {
    for (const id of ids) {
      const cur = this.fields.get(id);
      if (cur) {
        this.fields.set(id, { ...cur, pending: true, error: null });
        this.invalidate(id);
      }
    }
    this.formError = null;
    this.notifyFields(ids);
    this.refreshAggregates();
  }

  /** Mark a successful commit: advance committed (initial) values and clear pending. */
  applyCommit(
    ids: FormFieldId[],
    committedValues: Map<FormFieldId, FormValue>,
  ): void {
    for (const id of ids) {
      const cur = this.fields.get(id);
      if (!cur) continue;
      const committedValue = committedValues.get(id);
      const newInitialValue =
        committedValue && formValuesEqual(cur.value, committedValue)
          ? committedValue
          : cur.initialValue;
      this.fields.set(id, {
        ...cur,
        initialValue: newInitialValue,
        dirty: !formValuesEqual(cur.value, newInitialValue),
        pending: false,
        error: null,
        clearRequested: false,
      });
      this.invalidate(id);
    }
    this.notifyFields(ids);
    this.refreshAggregates();
  }

  markCommitError(ids: FormFieldId[], message: string): void {
    this.formError = message;
    for (const id of ids) {
      const cur = this.fields.get(id);
      if (cur) {
        this.fields.set(id, { ...cur, pending: false, error: message });
        this.invalidate(id);
      }
    }
    this.notifyFields(ids);
    this.refreshAggregates();
  }

  setFieldError(id: FormFieldId, message: string): void {
    const cur = this.fields.get(id);
    if (!cur) return;
    this.fields.set(id, { ...cur, touched: true, error: message });
    this.formError = message;
    this.invalidate(id);
    this.notifyFields([id]);
    this.refreshAggregates();
  }

  setFormError(message: string | null): void {
    if (this.formError === message) return;
    this.formError = message;
    this.refreshAggregates();
  }

  // ---- internals ----

  private invalidate(id: FormFieldId): void {
    this.stateCache.delete(id);
    this.orderedCache = null;
  }

  private refreshAggregates(): void {
    const next = computeAggregates(this.fields, this.formError);
    if (!aggregatesEqual(this.aggregates, next)) {
      this.aggregates = next;
      this.notifyAggregates();
    }
  }

  private notifyFields(ids: Iterable<FormFieldId>): void {
    for (const id of ids) {
      const set = this.fieldListeners.get(id);
      if (set) for (const cb of [...set]) cb();
    }
  }

  private notifyStructure(): void {
    for (const cb of [...this.structureListeners]) cb();
  }

  private notifyAggregates(): void {
    for (const cb of [...this.aggregateListeners]) cb();
  }
}

export function toFieldState(stored: StoredFieldState): FormFieldState {
  return {
    model: stored.model,
    value: stored.value,
    initialValue: stored.initialValue,
    dirty: stored.dirty,
    touched: stored.touched,
    focused: stored.focused,
    pending: stored.pending,
    error: stored.error,
    validationErrors: stored.cachedValidationErrors,
    clearRequested: stored.clearRequested,
  };
}

function orderedModels(models: FormFieldModel[]): FormFieldModel[] {
  return [...models].sort((a, b) => a.order - b.order);
}

/** Reuse previous model identities in an already-ordered array (keeps the array's elements stable). */
function reuseModelArray(
  nextOrdered: FormFieldModel[],
  prevModelById: Map<FormFieldId, FormFieldModel>,
): FormFieldModel[] {
  return nextOrdered.map((m) => {
    const prev = prevModelById.get(m.id);
    return prev && modelsEqual(prev, m) ? prev : m;
  });
}

function computeAggregates(
  fields: Map<FormFieldId, StoredFieldState>,
  formError: string | null,
): FieldAggregates {
  let dirty = false;
  let pending = false;
  let blocking = false;
  for (const f of fields.values()) {
    if (f.dirty) dirty = true;
    if (f.pending) pending = true;
    if (f.error || f.cachedValidationErrors.length > 0) blocking = true;
  }
  return {
    hasDraftChanges: dirty,
    hasPendingCommits: pending,
    hasBlockingErrors: blocking,
    formError,
  };
}

function aggregatesEqual(a: FieldAggregates, b: FieldAggregates): boolean {
  return (
    a.hasDraftChanges === b.hasDraftChanges &&
    a.hasPendingCommits === b.hasPendingCommits &&
    a.hasBlockingErrors === b.hasBlockingErrors &&
    a.formError === b.formError
  );
}

function arraysEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}

function storedFieldsEqual(a: StoredFieldState, b: StoredFieldState): boolean {
  return (
    a.model === b.model &&
    a.dirty === b.dirty &&
    a.touched === b.touched &&
    a.focused === b.focused &&
    a.pending === b.pending &&
    a.error === b.error &&
    a.clearRequested === b.clearRequested &&
    formValuesEqual(a.value, b.value) &&
    formValuesEqual(a.initialValue, b.initialValue) &&
    arraysEqual(a.cachedValidationErrors, b.cachedValidationErrors)
  );
}

/**
 * Structural equality of two field models. Used to preserve model object identity across
 * a rebuild. Compares every field that affects editing (paths, node ids, order) so a model
 * is only reused when reusing it cannot misdirect an edit. Models contain no functions, so
 * a stable JSON serialization is a sound and cheap-enough comparison for the rare rebuild.
 */
function modelsEqual(a: FormFieldModel, b: FormFieldModel): boolean {
  if (a === b) return true;
  return JSON.stringify(a) === JSON.stringify(b);
}
