import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Loader2, Trash2, X } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import type {
  SchemaCatalog,
  DefTypeSchema,
  DefTemplate,
  FieldSchema,
  TemplateFieldValue,
} from "../../../schema-catalog/types";
import { searchDefs } from "../../../def-index";
import type { IndexedDef, IndexedDefSearchResult } from "../../../def-index";
import type { UseXmlEditorSessionReturn } from "../../hooks/useXmlEditorSession";
import type { CreateDefResult } from "../../types/createDef";
import type { UserDefTemplateSummary } from "../../types/defTemplates";
import styles from "./CreateDefWizard.module.css";

interface Props {
  catalog: SchemaCatalog;
  session: UseXmlEditorSessionReturn;
  onClose: () => void;
  onCreated: (result: CreateDefResult) => void;
}

type Step = 1 | 2 | 3;
type TemplateSource = "user" | "builtin" | "indexed";

interface WizardState {
  step: Step;
  defType: string;
  // Which tab is showing in step 2. Defaults to "builtin" and flips to "user"
  // once a fetch confirms user templates exist for the selected def type.
  templateSource: TemplateSource;
  // Which kind was actually chosen - drives step 3's create behavior. Distinct
  // from templateSource because the user can still switch tabs after picking one.
  selectedKind: TemplateSource | null;
  templateId: string | null;
  userTemplateId: string | null;
  // Search box contents and results for the "Indexed Defs" tab, and the result
  // the user picked (carries every identifying field insertDefFromIndexedDef
  // needs - location, path, type, name, and node id for disambiguation).
  indexedQuery: string;
  indexedLoading: boolean;
  indexedResults: IndexedDefSearchResult[];
  indexedError: string | null;
  selectedIndexedDef: IndexedDef | null;
  fieldValues: Record<string, string>;
  busy: boolean;
  error: string | null;
  // Id of a user template currently being deleted, and the error from a failed
  // delete attempt. Kept separate from `error` (which is scoped to step 3's
  // create flow) since deletion happens from step 2's list.
  deletingId: string | null;
  deleteError: string | null;
}

// Sentinel template id used for the "Blank" option (no real template).
const BLANK_TEMPLATE_ID = null;

export function CreateDefWizard({
  catalog,
  session,
  onClose,
  onCreated,
}: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t, i18n } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const [state, setState] = useState<WizardState>({
    step: 1,
    defType: "",
    templateSource: "builtin",
    selectedKind: null,
    templateId: BLANK_TEMPLATE_ID,
    userTemplateId: null,
    indexedQuery: "",
    indexedLoading: false,
    indexedResults: [],
    indexedError: null,
    selectedIndexedDef: null,
    fieldValues: {},
    busy: false,
    error: null,
    deletingId: null,
    deleteError: null,
  });
  const [searchQuery, setSearchQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const [userTemplates, setUserTemplates] = useState<UserDefTemplateSummary[]>(
    [],
  );
  // Mirrors state.defType so an in-flight delete's async refresh can detect a
  // def-type switch that happened while it was awaiting the backend, the same
  // hazard the initial user-templates fetch below already guards against with
  // its own `cancelled` flag.
  const defTypeRef = useRef(state.defType);
  defTypeRef.current = state.defType;

  useEffect(() => {
    searchRef.current?.focus();
  }, [state.step]);

  // Load user templates for the selected def type. Depends on the stable
  // `listUserDefTemplates` function reference (not on `session` itself, which
  // is a fresh object every parent render) so this doesn't refetch on
  // unrelated re-renders while the wizard is open.
  useEffect(() => {
    if (!state.defType) {
      setUserTemplates([]);
      return;
    }
    let cancelled = false;
    const requestedDefType = state.defType;
    session
      .listUserDefTemplates(requestedDefType)
      .then((templates) => {
        if (cancelled) return;
        setUserTemplates(templates);
        setState((s) =>
          s.defType === requestedDefType && templates.length > 0
            ? { ...s, templateSource: "user" }
            : s,
        );
      })
      .catch(() => {
        if (!cancelled) setUserTemplates([]);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.defType, session.listUserDefTemplates]);

  // ── Step 1: non-abstract def type names, sorted alphabetically ──────────
  const allDefTypes = Object.entries(catalog.defTypes)
    .filter(([, schema]) => !schema.abstractType)
    .map(([key, schema]) => ({ key, label: schema.label ?? key }))
    .sort((a, b) => a.label.localeCompare(b.label, i18n.language));

  const filteredDefTypes = searchQuery.trim()
    ? allDefTypes.filter(
        (dt) =>
          dt.key.toLowerCase().includes(searchQuery.toLowerCase()) ||
          dt.label.toLowerCase().includes(searchQuery.toLowerCase()),
      )
    : allDefTypes;

  function selectDefType(key: string) {
    setState((s) => ({
      ...s,
      step: 2,
      defType: key,
      templateSource: "builtin",
      selectedKind: null,
      templateId: null,
      userTemplateId: null,
      indexedQuery: "",
      indexedLoading: false,
      indexedResults: [],
      indexedError: null,
      selectedIndexedDef: null,
      fieldValues: {},
      deletingId: null,
      deleteError: null,
    }));
    setSearchQuery("");
    // Clear stale results from the previous def type immediately, rather than
    // waiting for the new fetch to resolve - otherwise a template saved for
    // the previous def type would briefly stay visible under the new one.
    setUserTemplates([]);
  }

  // ── Step 2: indexed defs for the selected def type ───────────────────────
  // Searches only while the "Indexed Defs" tab is active - switching def type
  // resets templateSource to "builtin" (selectDefType above), so this doesn't
  // fire on every def-type pick, only once the user opts into the tab.
  // Debounced ~150ms to match useDefSearch; an empty query still fetches
  // ranked results (search_defs ranks empty queries), so the tab is useful
  // without knowing an exact def name.
  useEffect(() => {
    if (!state.defType || state.templateSource !== "indexed") return;
    let cancelled = false;
    const requestedDefType = state.defType;
    const requestedQuery = state.indexedQuery;
    setState((s) => ({ ...s, indexedLoading: true, indexedError: null }));
    const timeoutId = setTimeout(() => {
      searchDefs(session.projectId, requestedQuery, requestedDefType, true, 50)
        .then((results) => {
          if (cancelled) return;
          setState((s) =>
            s.defType === requestedDefType && s.indexedQuery === requestedQuery
              ? { ...s, indexedResults: results, indexedLoading: false }
              : s,
          );
        })
        .catch((e: unknown) => {
          if (cancelled) return;
          setState((s) =>
            s.defType === requestedDefType && s.indexedQuery === requestedQuery
              ? { ...s, indexedError: formatWizardError(e), indexedLoading: false }
              : s,
          );
        });
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(timeoutId);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.defType, state.templateSource, state.indexedQuery, session.projectId]);

  function selectIndexedDef(result: IndexedDefSearchResult) {
    setState((s) => ({
      ...s,
      step: 3,
      selectedKind: "indexed",
      templateId: null,
      userTemplateId: null,
      selectedIndexedDef: result.def,
      fieldValues: {},
      error: null,
    }));
  }

  // ── Step 2: templates for the selected def type ──────────────────────────
  const defTypeSchema = catalog.defTypes[state.defType];
  const templateEntries: Array<{
    id: string | null;
    label: string;
    description?: string;
  }> = [
    {
      id: BLANK_TEMPLATE_ID,
      label: t("createDefWizard.blankTemplateLabel"),
      description: t("createDefWizard.blankTemplateDescription"),
    },
    ...Object.values(defTypeSchema?.templates ?? {})
      .sort((a, b) => a.label.localeCompare(b.label, i18n.language))
      .map((t) => ({ id: t.id, label: t.label, description: t.description })),
  ];

  function selectTemplate(
    id: string | null,
    template: DefTemplate | undefined,
  ) {
    // Pre-populate fieldValues from template defaults.
    const prefill: Record<string, string> = {};
    if (template) {
      for (const [k, v] of Object.entries(template.fieldValues)) {
        if (typeof v === "string") prefill[k] = v;
        else if (typeof v === "number" || typeof v === "boolean")
          prefill[k] = String(v);
      }
    }
    setState((s) => ({
      ...s,
      step: 3,
      selectedKind: "builtin",
      templateId: id,
      userTemplateId: null,
      fieldValues: prefill,
      error: null,
    }));
  }

  function selectUserTemplate(template: UserDefTemplateSummary) {
    setState((s) => ({
      ...s,
      step: 3,
      selectedKind: "user",
      templateId: null,
      userTemplateId: template.id,
      fieldValues: {},
      error: null,
    }));
  }

  async function handleDeleteTemplate(template: UserDefTemplateSummary) {
    const requestedDefType = state.defType;
    const ok = await confirm(
      t("createDefWizard.deleteTemplateConfirm", { name: template.name }),
      {
        title: t("createDefWizard.deleteTemplateTitle"),
        kind: "warning",
        okLabel: tCommon("actions.delete"),
        cancelLabel: tCommon("actions.cancel"),
      },
    );
    if (!ok) return;

    setState((s) => ({ ...s, deletingId: template.id, deleteError: null }));
    try {
      await session.deleteUserDefTemplate(template.id);
      const refreshed = await session.listUserDefTemplates(requestedDefType);
      // The user may have switched def types while the delete/refresh was in
      // flight - selectDefType already cleared userTemplates and kicked off
      // its own fetch for the new type, so applying this stale result would
      // clobber it with templates for a def type that is no longer selected.
      if (defTypeRef.current !== requestedDefType) return;
      setUserTemplates(refreshed);
      setState((s) => ({
        ...s,
        deletingId: null,
        // Fall back to built-in templates if that was the last user template
        // for this def type and the user-template source is still active.
        templateSource:
          s.templateSource === "user" && refreshed.length === 0
            ? "builtin"
            : s.templateSource,
      }));
    } catch (e: unknown) {
      if (defTypeRef.current !== requestedDefType) return;
      setState((s) => ({
        ...s,
        deletingId: null,
        deleteError: formatWizardError(e),
      }));
    }
  }

  // ── Step 3: form fields ──────────────────────────────────────────────────
  const selectedTemplate =
    state.selectedKind === "builtin" && state.templateId != null
      ? defTypeSchema?.templates?.[state.templateId]
      : undefined;

  const promptFields =
    state.selectedKind === "user" || state.selectedKind === "indexed"
      ? buildClonePromptFields(defTypeSchema, catalog, t)
      : buildPromptFields(defTypeSchema, selectedTemplate, catalog, t);

  function setFieldValue(name: string, value: string) {
    setState((s) => ({
      ...s,
      fieldValues: { ...s.fieldValues, [name]: value },
      error: null,
    }));
  }

  async function handleCreate() {
    setState((s) => ({ ...s, busy: true, error: null }));
    try {
      let result: CreateDefResult;
      if (state.selectedKind === "user" && state.userTemplateId) {
        result = await session.insertDefFromUserTemplate(
          state.userTemplateId,
          (state.fieldValues.defName ?? "").trim(),
        );
      } else if (state.selectedKind === "indexed" && state.selectedIndexedDef) {
        result = await session.insertDefFromIndexedDef(
          state.selectedIndexedDef,
          (state.fieldValues.defName ?? "").trim(),
        );
      } else {
        const apiValues: Record<string, TemplateFieldValue> = {};
        for (const [k, v] of Object.entries(state.fieldValues)) {
          if (v.trim() !== "") apiValues[k] = v;
        }
        result = await session.insertDefFromTemplate(
          state.defType,
          state.templateId,
          apiValues,
        );
      }
      onCreated(result);
    } catch (e: unknown) {
      setState((s) => ({ ...s, busy: false, error: formatWizardError(e) }));
    }
  }

  const stepTitle =
    state.step === 1
      ? t("createDefWizard.stepTitlePickType")
      : state.step === 2
        ? t("createDefWizard.stepTitlePickTemplate")
        : t("createDefWizard.stepTitleEnterValues");

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("createDefWizard.dialogAriaLabel")}
    >
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>{stepTitle}</span>
          <span className={styles.stepIndicator}>{state.step} / 3</span>
          <button
            className={styles.closeBtn}
            onClick={onClose}
            aria-label={tCommon("actions.close")}
          >
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          {state.step === 1 && (
            <>
              <div className={styles.searchRow}>
                <input
                  ref={searchRef}
                  className={styles.searchInput}
                  type="text"
                  placeholder={t("createDefWizard.searchDefTypesPlaceholder")}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  autoComplete="off"
                />
              </div>
              <div className={styles.list} role="listbox">
                {filteredDefTypes.length === 0 ? (
                  <div className={styles.emptyState}>
                    {t("createDefWizard.noDefTypesMatch", { query: searchQuery })}
                  </div>
                ) : (
                  filteredDefTypes.map((dt) => (
                    <button
                      key={dt.key}
                      className={styles.listItem}
                      role="option"
                      onClick={() => selectDefType(dt.key)}
                    >
                      <span className={styles.listItemLabel}>{dt.label}</span>
                      {dt.label !== dt.key && (
                        <span className={styles.listItemSub}>{dt.key}</span>
                      )}
                    </button>
                  ))
                )}
              </div>
            </>
          )}

          {state.step === 2 && (
            <>
              <div className={styles.sourceTabs} role="tablist">
                {userTemplates.length > 0 && (
                  <button
                    type="button"
                    role="tab"
                    aria-selected={state.templateSource === "user"}
                    className={`${styles.sourceTab} ${
                      state.templateSource === "user"
                        ? styles.sourceTabActive
                        : ""
                    }`}
                    onClick={() =>
                      setState((s) => ({ ...s, templateSource: "user" }))
                    }
                  >
                    {t("createDefWizard.userTemplatesTab")}
                  </button>
                )}
                <button
                  type="button"
                  role="tab"
                  aria-selected={state.templateSource === "builtin"}
                  className={`${styles.sourceTab} ${
                    state.templateSource === "builtin"
                      ? styles.sourceTabActive
                      : ""
                  }`}
                  onClick={() =>
                    setState((s) => ({ ...s, templateSource: "builtin" }))
                  }
                >
                  {t("createDefWizard.builtinTemplatesTab")}
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={state.templateSource === "indexed"}
                  className={`${styles.sourceTab} ${
                    state.templateSource === "indexed"
                      ? styles.sourceTabActive
                      : ""
                  }`}
                  onClick={() =>
                    setState((s) => ({ ...s, templateSource: "indexed" }))
                  }
                >
                  {t("createDefWizard.indexedDefsTab")}
                </button>
              </div>

              {state.templateSource === "indexed" ? (
                <>
                  <div className={styles.searchRow}>
                    <input
                      className={styles.searchInput}
                      type="text"
                      placeholder={t("createDefWizard.searchIndexedDefsPlaceholder")}
                      value={state.indexedQuery}
                      onChange={(e) =>
                        setState((s) => ({
                          ...s,
                          indexedQuery: e.target.value,
                        }))
                      }
                      autoComplete="off"
                    />
                  </div>
                  <div className={styles.list} role="listbox">
                    {state.indexedError ? (
                      <div className={styles.emptyState}>
                        {state.indexedError}
                      </div>
                    ) : state.indexedResults.length === 0 ? (
                      <div className={styles.emptyState}>
                        {state.indexedLoading
                          ? t("createDefWizard.searching")
                          : t("createDefWizard.noIndexedDefsMatch")}
                      </div>
                    ) : (
                      state.indexedResults.map((result) => (
                        <button
                          key={`${result.def.source.locationId}:${result.def.relativePath}:${result.def.defName}:${result.def.nodeId ?? ""}`}
                          className={styles.listItem}
                          role="option"
                          onClick={() => selectIndexedDef(result)}
                        >
                          <span className={styles.listItemLabel}>
                            {result.def.label ?? result.def.defName}
                          </span>
                          <span className={styles.listItemSub}>
                            {formatIndexedDefSub(result.def, t)}
                          </span>
                        </button>
                      ))
                    )}
                  </div>
                </>
              ) : state.templateSource === "user" && userTemplates.length > 0 ? (
                <div className={styles.list} role="listbox">
                  {state.deleteError && (
                    <div className={styles.deleteErrorBanner}>
                      {state.deleteError}
                    </div>
                  )}
                  {userTemplates.map((template) => (
                    <div key={template.id} className={styles.userTemplateRow}>
                      <button
                        className={styles.listItem}
                        role="option"
                        onClick={() => selectUserTemplate(template)}
                      >
                        <span className={styles.listItemLabel}>
                          {template.name}
                        </span>
                        {(template.originalLabel || template.originalDefName) && (
                          <span className={styles.listItemSub}>
                            {template.originalLabel ?? template.originalDefName}
                          </span>
                        )}
                      </button>
                      <button
                        type="button"
                        className={styles.deleteBtn}
                        aria-label={t("createDefWizard.deleteTemplateAriaLabel", { name: template.name })}
                        title={t("createDefWizard.deleteTemplateTitle")}
                        disabled={state.deletingId === template.id}
                        onClick={(e) => {
                          e.stopPropagation();
                          void handleDeleteTemplate(template);
                        }}
                      >
                        {state.deletingId === template.id ? (
                          <Loader2 size={13} className={styles.spinner} />
                        ) : (
                          <Trash2 size={13} />
                        )}
                      </button>
                    </div>
                  ))}
                </div>
              ) : (
                <div className={styles.list} role="listbox">
                  {templateEntries.map((entry) => (
                    <button
                      key={entry.id ?? "__blank__"}
                      className={styles.listItem}
                      role="option"
                      onClick={() =>
                        selectTemplate(
                          entry.id,
                          entry.id != null
                            ? defTypeSchema?.templates?.[entry.id]
                            : undefined,
                        )
                      }
                    >
                      <span className={styles.listItemLabel}>
                        {entry.label}
                      </span>
                      {entry.description && (
                        <span className={styles.listItemSub}>
                          {entry.description}
                        </span>
                      )}
                    </button>
                  ))}
                </div>
              )}
            </>
          )}

          {state.step === 3 && (
            <div className={styles.formBody}>
              {state.selectedKind === "indexed" && state.selectedIndexedDef && (
                <span className={styles.listItemSub}>
                  {t("createDefWizard.cloningFrom", {
                    defName: state.selectedIndexedDef.defName,
                    relativePath: state.selectedIndexedDef.relativePath,
                  })}
                </span>
              )}
              {promptFields.map((field) => (
                <div key={field.name} className={styles.fieldRow}>
                  <label className={styles.fieldLabel}>
                    {field.label}
                    {field.required && (
                      <span className={styles.requiredMark}>*</span>
                    )}
                  </label>
                  <input
                    className={styles.fieldInput}
                    type="text"
                    value={state.fieldValues[field.name] ?? ""}
                    onChange={(e) => setFieldValue(field.name, e.target.value)}
                    placeholder={field.placeholder ?? ""}
                    autoFocus={field.name === "defName"}
                  />
                </div>
              ))}
            </div>
          )}
        </div>

        {state.step === 3 && (
          <div className={styles.footer}>
            {state.error ? (
              <span className={styles.errorBanner} title={state.error}>
                {state.error}
              </span>
            ) : (
              <span className={styles.spacer} />
            )}
            <button
              className={styles.backBtn}
              onClick={() => setState((s) => ({ ...s, step: 2, error: null }))}
              disabled={state.busy}
            >
              {t("createDefWizard.back")}
            </button>
            <button
              className={styles.createBtn}
              onClick={() => void handleCreate()}
              disabled={
                state.busy ||
                promptFields.some(
                  (f) => f.required && !state.fieldValues[f.name]?.trim(),
                )
              }
            >
              {state.busy ? t("createDefWizard.creating") : t("createDefWizard.create")}
            </button>
          </div>
        )}
        {state.step === 2 && (
          <div className={styles.footer}>
            <span className={styles.spacer} />
            <button
              className={styles.backBtn}
              onClick={() => setState((s) => ({ ...s, step: 1, error: null }))}
            >
              {t("createDefWizard.back")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Helpers ────────────────────────────────────────────────────────────────

interface PromptField {
  name: string;
  label: string;
  required: boolean;
  placeholder?: string;
}

function buildPromptFields(
  defTypeSchema: DefTypeSchema | undefined,
  template: DefTemplate | undefined,
  catalog: SchemaCatalog,
  t: TFunction<"editor">,
): PromptField[] {
  const fields: PromptField[] = [];
  const seen = new Set<string>();

  // defName is always first; required status comes from schema metadata.
  const defNameSchema = lookupFieldInCatalog(defTypeSchema, "defName", catalog);
  fields.push({
    name: "defName",
    label: defNameSchema?.label ?? t("createDefWizard.defNameFallback"),
    required: defNameSchema?.required ?? false,
    placeholder: t("createDefWizard.defNamePlaceholder"),
  });
  seen.add("defName");

  // Template prompt fields.
  if (template) {
    for (const name of template.promptFields) {
      if (seen.has(name)) continue;
      seen.add(name);
      const schema = lookupFieldInCatalog(defTypeSchema, name, catalog);
      fields.push({
        name,
        label: schema?.label ?? fieldNameToLabel(name),
        required: schema?.required ?? false,
      });
    }
  }

  // Required fields from schema (when includeRequiredFields).
  const includeRequired = template?.includeRequiredFields ?? true;
  if (includeRequired && defTypeSchema) {
    const allRequired = collectRequiredFields(defTypeSchema, catalog);
    for (const { name, label } of allRequired) {
      if (seen.has(name)) continue;
      // Skip fields already covered by template defaults.
      if (template?.fieldValues[name] !== undefined) continue;
      seen.add(name);
      fields.push({ name, label, required: true });
    }
  }

  return fields;
}

// Both saved-template and indexed-def cloning only ever change `defName`
// (see create_def_from_user_template / create_def_from_indexed_def) - every
// other field is whatever the source XML already contains, so the wizard
// only prompts for the new defName.
function buildClonePromptFields(
  defTypeSchema: DefTypeSchema | undefined,
  catalog: SchemaCatalog,
  t: TFunction<"editor">,
): PromptField[] {
  const defNameSchema = lookupFieldInCatalog(defTypeSchema, "defName", catalog);
  return [
    {
      name: "defName",
      label: defNameSchema?.label ?? t("createDefWizard.defNameFallback"),
      required: true,
      placeholder: t("createDefWizard.defNamePlaceholder"),
    },
  ];
}

function collectRequiredFields(
  defTypeSchema: DefTypeSchema,
  catalog: SchemaCatalog,
  visited: Set<string> = new Set(),
): Array<{ name: string; label: string }> {
  const results: Array<{ name: string; label: string }> = [];

  for (const parent of defTypeSchema.inherits) {
    if (visited.has(parent)) continue;
    visited.add(parent);
    const parentSchema = catalog.defTypes[parent];
    if (parentSchema) {
      results.push(...collectRequiredFields(parentSchema, catalog, visited));
    }
  }

  for (const [name, field] of Object.entries(defTypeSchema.fields) as [
    string,
    FieldSchema,
  ][]) {
    if (field.required) {
      results.push({ name, label: field.label ?? fieldNameToLabel(name) });
    }
  }

  return results;
}

function lookupFieldInCatalog(
  defTypeSchema: DefTypeSchema | undefined,
  name: string,
  catalog: SchemaCatalog,
  visited: Set<string> = new Set(),
): FieldSchema | undefined {
  if (!defTypeSchema) return undefined;

  if (defTypeSchema.fields[name]) return defTypeSchema.fields[name];

  for (const parent of defTypeSchema.inherits) {
    if (visited.has(parent)) continue;
    visited.add(parent);
    const parentSchema = catalog.defTypes[parent];
    if (parentSchema) {
      const found = lookupFieldInCatalog(parentSchema, name, catalog, visited);
      if (found) return found;
    }
  }

  return undefined;
}

function formatIndexedDefSub(def: IndexedDef, t: TFunction<"editor">): string {
  const bits: string[] = [];
  // The def's label is already shown as the row's primary text when present,
  // so surface defName here instead of repeating the label.
  if (def.label) bits.push(def.defName);
  bits.push(
    def.source.sourceKind === "project"
      ? t("createDefWizard.projectSourceLabel")
      : def.source.locationName,
  );
  bits.push(def.line != null ? `${def.relativePath}:${def.line}` : def.relativePath);
  return bits.join(" · ");
}

function fieldNameToLabel(name: string): string {
  return name.replace(/([A-Z])/g, " $1").replace(/^./, (c) => c.toUpperCase());
}

/** Normalizes a caught error to English UI text through the shared diagnostic renderer
 * (`formatError`/`renderDiagnostic`). `formatError` (via `formatCommandError` in
 * `src/i18n/diagnostics.ts`) already unwraps a raw JSON-encoded string rejection (e.g. a command
 * that still returns `Result<T, String>`) into its `code`/`args`/`message` object before
 * rendering, so this wrapper no longer needs its own local unwrap -- keeping it as a thin
 * pass-through since it is already the call site used throughout this file. */
function formatWizardError(e: unknown): string {
  return formatError(e);
}
