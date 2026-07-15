import React, { useCallback, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { Plus, RotateCcw, Trash2, X } from "lucide-react";
import type { FormFieldState, FormValue } from "../../types/editorForm";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import type { FormFieldStore } from "../../lib/formFieldStore";
import { useXmlEditorContext } from "../../context/XmlEditorContext";
import {
  booleanFormValue,
  enumFormValue,
  flagsFormValue,
  listFormValue,
  namedMapFormValue,
  scalarFormValue,
  typedReferenceListFormValue,
} from "../../hooks/useXmlFormController";
import { ReferencePicker } from "../ReferencePicker/ReferencePicker";
import { ReferenceListEditor } from "../ReferencePicker/ReferenceListEditor";
import { ObjectListEditor } from "../ObjectListEditor/ObjectListEditor";
import { TypedReferenceListEditor } from "../TypedReferenceListEditor/TypedReferenceListEditor";
import { initI18n } from "../../../../i18n";
import styles from "./FormFieldControl.module.css";

interface Props {
  fieldId: string;
  store: FormFieldStore;
  formApi: XmlFormApi;
  nestedDepth?: number;
}

/** Stable DOM id for a field's primary input, derived from its canonical `FormFieldId`. Exported
 * so other components can look up/focus a control's DOM node without duplicating this
 * sanitization -- e.g. `XmlFormEditor`'s Form View "reveal and focus" flow (issue 08, Plan.md
 * section 8: "focuses/scrolls to the first rendered field" after `Reveal fields with issues`). */
export function fieldInputDomId(fieldId: string): string {
  return `field-${fieldId.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

export const FormFieldControl = React.memo(
  function FormFieldControl({
    fieldId,
    store,
    formApi,
    nestedDepth = 0,
  }: Props) {
    // Subscribe to this field only - a keystroke notifies just this control's subscriber,
    // so editing one field never re-renders the others (typing cost is independent of form size).
    const subscribe = useCallback(
      (cb: () => void) => store.subscribeField(fieldId, cb),
      [store, fieldId],
    );
    const getSnapshot = useCallback(
      () => store.getFieldState(fieldId),
      [store, fieldId],
    );
    const field = useSyncExternalStore(subscribe, getSnapshot);
    const { readOnly } = useXmlEditorContext();
    const { t } = useTranslation("editor");

    if (!field) return null;
    const { model } = field;

    if (model.control === "readonlyUnknown") {
      return null;
    }

    const inputId = fieldInputDomId(model.id);
    const visibleErrors =
      field.touched || field.error ? field.validationErrors : [];

    const showClearBtn =
      !model.readonly && !readOnly && model.control !== "object";
    const clearDisabled =
      field.clearRequested || (!field.dirty && model.sourceNodeId === null);

    return (
      <div
        className={styles.field}
        data-dirty={field.dirty || undefined}
        data-nested={nestedDepth > 0 || undefined}
      >
        <div className={styles.labelRow}>
          <label htmlFor={inputId} className={styles.label}>
            {model.label}
            {model.required && <span className={styles.required}>*</span>}
          </label>
          <div className={styles.fieldState}>
            {field.pending && (
              <span className={styles.pending}>{t("formFieldControl.saving")}</span>
            )}
            {field.dirty && (
              <button
                className={styles.resetBtn}
                onClick={() => formApi.resetField(model.id)}
                type="button"
                title={t("formFieldControl.resetField")}
                aria-label={t("formFieldControl.resetFieldAria", { label: model.label })}
              >
                <RotateCcw size={12} />
              </button>
            )}
            {showClearBtn && (
              <button
                className={styles.clearBtn}
                onClick={() => formApi.clearField(model.id)}
                type="button"
                title={t("formFieldControl.clearField", { label: model.label })}
                aria-label={t("formFieldControl.clearField", { label: model.label })}
                disabled={clearDisabled}
              >
                <X size={12} />
              </button>
            )}
          </div>
        </div>
        {model.description && (
          <p className={styles.description}>{model.description}</p>
        )}
        <FieldInput inputId={inputId} field={field} formApi={formApi} />
        {model.examples.length > 0 && (
          <p className={styles.hint}>
            {t("formFieldControl.examplesPrefix", {
              examples: model.examples.slice(0, 2).join(", "),
            })}
          </p>
        )}
        {model.readOnlyReason && (
          <p className={styles.hint}>{model.readOnlyReason}</p>
        )}
        {field.error && <p className={styles.error}>{field.error}</p>}
        {visibleErrors.map((error) => (
          <p key={error} className={styles.error}>
            {error}
          </p>
        ))}
      </div>
    );
  },
  (prev, next) =>
    prev.fieldId === next.fieldId &&
    Object.is(prev.store, next.store) &&
    Object.is(prev.formApi.actions, next.formApi.actions) &&
    prev.nestedDepth === next.nestedDepth,
);

interface FieldInputProps {
  inputId: string;
  field: FormFieldState;
  formApi: XmlFormApi;
}

function FieldInput({ inputId, field, formApi }: FieldInputProps) {
  const { projectId, readOnly, onNavigateDef } = useXmlEditorContext();
  const { t } = useTranslation("editor");
  const { model, value } = field;
  const isReadOnlyView = model.readonly || readOnly;

  if (
    isReadOnlyView &&
    model.control === "objectList" &&
    value.kind === "objectList"
  ) {
    return <ObjectListEditor field={field} formApi={formApi} readOnly id={inputId} />;
  }

  if (isReadOnlyView) {
    if (readOnly && model.typedReference && projectId) {
      const trlItems = value.kind === "typedReferenceList" ? value.items : [];
      return (
        <TypedReferenceListEditor
          inputId={inputId}
          items={trlItems}
          typedReference={model.typedReference}
          projectId={projectId}
          onChangeItems={() => undefined}
          readOnly
          onNavigateDef={onNavigateDef}
        />
      );
    }
    if (readOnly && model.reference && projectId) {
      if (model.control === "list") {
        const listItems = value.kind === "list" ? value.items : [];
        return (
          <ReferenceListEditor
            inputId={inputId}
            items={listItems}
            reference={model.reference}
            projectId={projectId}
            onChangeItems={() => undefined}
            readOnly
            onNavigateDef={onNavigateDef}
          />
        );
      }
      return (
        <ReferencePicker
          inputId={inputId}
          value={formValueText(value)}
          reference={model.reference}
          projectId={projectId}
          onChange={() => undefined}
          onNavigateDef={onNavigateDef}
          readOnly
        />
      );
    }
    return <span className={styles.readonlyValue}>{formValueText(value)}</span>;
  }

  switch (model.control) {
    case "textarea":
      return (
        <textarea
          id={inputId}
          className={styles.textarea}
          value={formValueText(value)}
          onChange={(e) =>
            formApi.setFieldValue(
              model.id,
              scalarFormValue(e.currentTarget.value),
            )
          }
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
          spellCheck={false}
          rows={4}
        />
      );

    case "number":
      return (
        <input
          id={inputId}
          type="number"
          className={styles.input}
          value={formValueText(value)}
          onChange={(e) =>
            formApi.setFieldValue(
              model.id,
              scalarFormValue(e.currentTarget.value),
            )
          }
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
        />
      );

    case "checkbox":
      return (
        <input
          id={inputId}
          type="checkbox"
          className={styles.checkbox}
          checked={
            value.kind === "boolean"
              ? value.value
              : formValueText(value) === "true"
          }
          onChange={(e) =>
            formApi.setFieldValue(
              model.id,
              booleanFormValue(e.currentTarget.checked),
            )
          }
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
        />
      );

    case "select": {
      const selectText = formValueText(value);
      return (
        <select
          id={inputId}
          className={styles.select}
          value={selectText}
          onChange={(e) =>
            formApi.setFieldValue(
              model.id,
              enumFormValue(e.currentTarget.value),
            )
          }
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
        >
          {(!selectText || !model.allowedValues?.includes(selectText)) && (
            <option value={selectText}>{selectText || "-"}</option>
          )}
          {(model.allowedValues ?? []).map((allowedValue) => (
            <option key={allowedValue} value={allowedValue}>
              {allowedValue}
            </option>
          ))}
        </select>
      );
    }

    case "list":
      if (model.reference && projectId) {
        const listItems = value.kind === "list" ? value.items : [];
        return (
          <ReferenceListEditor
            inputId={inputId}
            items={listItems}
            reference={model.reference}
            projectId={projectId}
            onChangeItems={(next) =>
              formApi.setFieldValue(model.id, listFormValue(next))
            }
            onFocus={() => formApi.focusField(model.id)}
            onBlur={() => formApi.blurField(model.id)}
            readOnly={readOnly}
            onNavigateDef={onNavigateDef}
          />
        );
      }
      return <ListEditor inputId={inputId} field={field} formApi={formApi} />;

    case "flags":
      return <FlagsEditor inputId={inputId} field={field} formApi={formApi} />;

    case "typedReferenceList":
      if (model.typedReference && projectId) {
        const trlItems = value.kind === "typedReferenceList" ? value.items : [];
        return (
          <TypedReferenceListEditor
            inputId={inputId}
            items={trlItems}
            typedReference={model.typedReference}
            projectId={projectId}
            onChangeItems={(next) =>
              formApi.setFieldValue(model.id, typedReferenceListFormValue(next))
            }
            onFocus={() => formApi.focusField(model.id)}
            onBlur={() => formApi.blurField(model.id)}
            onNavigateDef={onNavigateDef}
          />
        );
      }
      return (
        <span className={styles.readonlyValue}>{formValueText(value)}</span>
      );

    case "namedMap":
      return (
        <NamedMapEditor inputId={inputId} field={field} formApi={formApi} />
      );

    case "objectList":
      if (field.value.kind === "objectList") {
        return <ObjectListEditor field={field} formApi={formApi} id={inputId} />;
      }
      return (
        <span
          className={styles.readonlyValue}
          title={t("formFieldControl.useRawXmlHint")}
        >
          {field.value.kind === "readonly"
            ? field.value.value
            : t("formFieldControl.objectListRawOnly")}
        </span>
      );

    case "object":
      return (
        <span
          className={styles.readonlyValue}
          title={t("formFieldControl.useRawXmlHint")}
        >
          {t("formFieldControl.objectRawOnly")}
        </span>
      );

    case "reference":
      if (model.reference && projectId) {
        return (
          <ReferencePicker
            inputId={inputId}
            value={formValueText(value)}
            reference={model.reference}
            projectId={projectId}
            onChange={(v) =>
              formApi.setFieldValue(model.id, scalarFormValue(v))
            }
            onFocus={() => formApi.focusField(model.id)}
            onBlur={() => formApi.blurField(model.id)}
            onNavigateDef={onNavigateDef}
          />
        );
      }
      // Fall through to text if no reference metadata or projectId.
      return (
        <input
          id={inputId}
          type="text"
          className={styles.input}
          value={formValueText(value)}
          onChange={(e) =>
            formApi.setFieldValue(
              model.id,
              scalarFormValue(e.currentTarget.value),
            )
          }
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
        />
      );

    case "color":
      return (
        <ColorFieldInput
          inputId={inputId}
          value={formValueText(value)}
          onChange={(v) => formApi.setFieldValue(model.id, scalarFormValue(v))}
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
        />
      );

    case "text":
    default:
      return (
        <input
          id={inputId}
          type="text"
          className={styles.input}
          value={formValueText(value)}
          onChange={(e) =>
            formApi.setFieldValue(
              model.id,
              scalarFormValue(e.currentTarget.value),
            )
          }
          onFocus={() => formApi.focusField(model.id)}
          onBlur={() => formApi.blurField(model.id)}
        />
      );
  }
}

interface ListEditorProps {
  inputId: string;
  field: FormFieldState;
  formApi: XmlFormApi;
}

function ListEditor({ inputId, field, formApi }: ListEditorProps) {
  const { t } = useTranslation("editor");
  const items = field.value.kind === "list" ? field.value.items : [];

  function updateItems(next: string[]) {
    formApi.setFieldValue(field.model.id, listFormValue(next));
  }

  // `inputId` normally lands on the first row's input (below). With zero items there is no row
  // to carry it - fall back to the container itself as a DOM anchor Form Views' "reveal and
  // focus" flow (issue 08, Plan.md section 8) can always find, even for a required-but-absent
  // list field right after it is revealed. Never set together with the row-level id.
  const containerId = items.length === 0 ? inputId : undefined;

  return (
    <div
      className={styles.listEditor}
      role="list"
      id={containerId}
      tabIndex={containerId ? -1 : undefined}
    >
      {items.map((item, index) => (
        <div key={index} className={styles.listRow} role="listitem">
          <input
            id={index === 0 ? inputId : undefined}
            type="text"
            className={styles.listInput}
            value={item}
            onChange={(e) => {
              const next = items.map((existing, i) =>
                i === index ? e.currentTarget.value : existing,
              );
              updateItems(next);
            }}
            onFocus={() => formApi.focusField(field.model.id)}
            onBlur={() => formApi.blurField(field.model.id)}
          />
          <button
            className={styles.listRemove}
            onClick={() => updateItems(items.filter((_, i) => i !== index))}
            aria-label={t("formFieldControl.removeItem", { index: index + 1 })}
            type="button"
          >
            <Trash2 size={12} />
          </button>
        </div>
      ))}
      <button
        className={styles.listAdd}
        onClick={() => updateItems([...items, ""])}
        type="button"
      >
        <Plus size={12} />
        {t("formFieldControl.addItem")}
      </button>
    </div>
  );
}

interface FlagsEditorProps {
  inputId: string;
  field: FormFieldState;
  formApi: XmlFormApi;
}

function FlagsEditor({ inputId, field, formApi }: FlagsEditorProps) {
  const { t } = useTranslation("editor");
  const { model } = field;
  const flags =
    field.value.kind === "flags"
      ? field.value
      : { selected: [] as string[], custom: [] as string[] };
  const allowedValues = model.allowedValues ?? [];

  function toggle(flagValue: string, checked: boolean) {
    if (!("selected" in flags)) return;
    if (flagValue === "None") {
      const next = checked ? ["None"] : [];
      formApi.setFieldValue(model.id, flagsFormValue(next, []));
    } else {
      const withoutNone = flags.selected.filter((v) => v !== "None");
      const next = checked
        ? [...withoutNone, flagValue]
        : withoutNone.filter((v) => v !== flagValue);
      formApi.setFieldValue(model.id, flagsFormValue(next, flags.custom));
    }
  }

  function removeCustom(flagValue: string) {
    if (!("custom" in flags)) return;
    formApi.setFieldValue(
      model.id,
      flagsFormValue(
        flags.selected,
        flags.custom.filter((v) => v !== flagValue),
      ),
    );
  }

  return (
    <div className={styles.flagsEditor} role="group">
      {allowedValues.map((v, index) => (
        <label key={v} className={styles.flagsRow}>
          <input
            id={index === 0 ? inputId : undefined}
            type="checkbox"
            className={styles.checkbox}
            checked={flags.selected.includes(v)}
            onChange={(e) => toggle(v, e.currentTarget.checked)}
            onFocus={() => formApi.focusField(model.id)}
            onBlur={() => formApi.blurField(model.id)}
          />
          {v}
        </label>
      ))}
      {flags.custom.length > 0 && (
        <div className={styles.flagsCustomSection}>
          <span className={styles.hint}>{t("formFieldControl.unknownValuesPreserved")}</span>
          {flags.custom.map((v) => (
            <div key={v} className={styles.flagsRow}>
              <span className={styles.readonlyValue}>{v}</span>
              <button
                className={styles.listRemove}
                onClick={() => removeCustom(v)}
                aria-label={t("formFieldControl.removeUnknownFlag", { value: v })}
                type="button"
              >
                <Trash2 size={12} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

interface NamedMapEditorProps {
  inputId: string;
  field: FormFieldState;
  formApi: XmlFormApi;
}

function NamedMapEditor({ inputId, field, formApi }: NamedMapEditorProps) {
  const { projectId, onNavigateDef } = useXmlEditorContext();
  const { t } = useTranslation("editor");
  const { model } = field;
  const entries = field.value.kind === "namedMap" ? field.value.entries : [];

  function updateEntry(index: number, key: string, value: string) {
    const next = entries.map((e, i) => (i === index ? { key, value } : e));
    formApi.setFieldValue(model.id, namedMapFormValue(next));
  }

  function removeEntry(index: number) {
    formApi.setFieldValue(
      model.id,
      namedMapFormValue(entries.filter((_, i) => i !== index)),
    );
  }

  function addEntry() {
    formApi.setFieldValue(
      model.id,
      namedMapFormValue([...entries, { key: "", value: "" }]),
    );
  }

  const keysSeen = new Set<string>();
  const hasDuplicateKeys =
    !model.repeatable &&
    entries.some((e) => {
      if (!e.key) return false;
      if (keysSeen.has(e.key)) return true;
      keysSeen.add(e.key);
      return false;
    });

  const useKeyPicker = !!(model.keyReference && projectId);

  // `inputId` normally lands on the first row's key input (below). With zero entries there is
  // no row to carry it - fall back to the container itself as a DOM anchor Form Views' "reveal
  // and focus" flow (issue 08, Plan.md section 8) can always find. Never set together with the
  // row-level id.
  const containerId = entries.length === 0 ? inputId : undefined;

  return (
    <div
      className={styles.namedMapEditor}
      id={containerId}
      tabIndex={containerId ? -1 : undefined}
    >
      {entries.map((entry, index) => (
        <div key={index} className={styles.mapRow}>
          {useKeyPicker ? (
            <div className={styles.mapKeyContainer}>
              <ReferencePicker
                inputId={index === 0 ? inputId : undefined}
                value={entry.key}
                reference={model.keyReference!}
                projectId={projectId!}
                onChange={(newKey) => updateEntry(index, newKey, entry.value)}
                onFocus={() => formApi.focusField(model.id)}
                onBlur={() => formApi.blurField(model.id)}
                onNavigateDef={onNavigateDef}
              />
            </div>
          ) : (
            <input
              id={index === 0 ? inputId : undefined}
              type="text"
              className={styles.mapKey}
              value={entry.key}
              placeholder={t("formFieldControl.keyPlaceholder")}
              onChange={(e) =>
                updateEntry(index, e.currentTarget.value, entry.value)
              }
              onFocus={() => formApi.focusField(model.id)}
              onBlur={() => formApi.blurField(model.id)}
              spellCheck={false}
            />
          )}
          <input
            type="text"
            className={styles.mapValue}
            value={entry.value}
            placeholder={t("formFieldControl.valuePlaceholder")}
            onChange={(e) =>
              updateEntry(index, entry.key, e.currentTarget.value)
            }
            onFocus={() => formApi.focusField(model.id)}
            onBlur={() => formApi.blurField(model.id)}
            spellCheck={false}
          />
          <button
            className={styles.listRemove}
            onClick={() => removeEntry(index)}
            aria-label={t("formFieldControl.removeEntry", { key: entry.key || index })}
            type="button"
          >
            <Trash2 size={12} />
          </button>
        </div>
      ))}
      {hasDuplicateKeys && (
        <p className={styles.error}>{t("formFieldControl.duplicateKeysError")}</p>
      )}
      <button className={styles.listAdd} onClick={addEntry} type="button">
        <Plus size={12} />
        {t("formFieldControl.addEntry")}
      </button>
    </div>
  );
}

/** Parse a RimWorld color tuple string into a CSS rgba() string, or null if malformed. */
export function parseColorValue(raw: string): string | null {
  const trimmed = raw.trim();
  const inner =
    trimmed.startsWith("(") && trimmed.endsWith(")")
      ? trimmed.slice(1, -1)
      : null;
  if (!inner) return null;
  const parts = inner.split(",").map((s) => s.trim());
  if (parts.length < 3 || parts.length > 4) return null;
  if (parts.some((p) => p === "")) return null;
  const nums = parts.map(Number);
  if (nums.some(isNaN)) return null;
  const useInt = nums.some((n) => n > 1.0);
  const [r, g, b, a = useInt ? 255 : 1] = nums;
  const rCss = useInt ? Math.round(r) : Math.round(r * 255);
  const gCss = useInt ? Math.round(g) : Math.round(g * 255);
  const bCss = useInt ? Math.round(b) : Math.round(b * 255);
  const aCss = useInt ? a / 255 : a;
  return `rgba(${rCss}, ${gCss}, ${bCss}, ${aCss.toFixed(3)})`;
}

interface ColorFieldInputProps {
  inputId: string;
  value: string;
  onChange: (value: string) => void;
  onFocus: () => void;
  onBlur: () => void;
}

function ColorFieldInput({
  inputId,
  value,
  onChange,
  onFocus,
  onBlur,
}: ColorFieldInputProps) {
  const swatchColor = value ? parseColorValue(value) : null;
  return (
    <div className={styles.colorField}>
      <input
        id={inputId}
        type="text"
        className={styles.input}
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
        onFocus={onFocus}
        onBlur={onBlur}
      />
      <span
        className={styles.colorSwatch}
        style={swatchColor ? { background: swatchColor } : undefined}
        aria-hidden="true"
      />
    </div>
  );
}

// Plain module function, not a React component -- resolves translated text from the app-wide
// i18next singleton (`initI18n().t(...)`, same as `src/features/xml-editor/lib/objectDescriptors.ts`)
// rather than a `useTranslation()` hook, which is unavailable here.
function formValueText(value: FormValue): string {
  switch (value.kind) {
    case "boolean":
      return value.value ? "true" : "false";
    case "enum":
    case "readonly":
    case "scalar":
      return value.value;
    case "list":
      return value.items.join("\n");
    case "flags":
      return [...value.selected, ...value.custom].join("\n");
    case "namedMap":
      return value.entries.map((e) => `${e.key}=${e.value}`).join("\n");
    case "typedReferenceList":
      return value.items.map((i) => `${i.defType}:${i.defName}`).join("\n");
    case "objectList": {
      const count = value.items.length;
      return initI18n().t(
        "editor:objectListEditor.itemCountParens",
        `(${count} item${count === 1 ? "" : "s"})`,
        { count },
      );
    }
  }
}
