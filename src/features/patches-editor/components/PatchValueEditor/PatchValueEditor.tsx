import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SchemaCatalog } from "../../../schema-catalog";
import { formatError } from "../../../../lib/formatError";
import { emptyToNull, nullToEmpty } from "../../lib/arrayUtils";
import { dedentXmlFragment } from "../../lib/xmlDedent";
import { parsePatchValueXml, serializePatchValueFragment } from "../../api/valueXml";
import {
  listDirectDefTypeFields,
  resolveModExtensionsField,
  targetDefType,
  type PatchValueEditTarget,
  type PatchValueOperationType,
} from "../../lib/patchValueTarget";
import {
  emptyFieldValue,
  fieldValueToInitialElement,
  isStructurallySupportedField,
  parsedViewsToFieldValue,
} from "../../lib/patchValueShape";
import type { XPathResolvedField, XPathTarget } from "../../types/xpathCompletion";
import { fieldSchemaToControl } from "../../../xml-editor";
import type { ObjectFieldValue } from "../../../xml-editor";
import { ValueFieldRenderer } from "./ValueFieldRenderer";
import styles from "./PatchValueEditor.module.css";

interface Props {
  valueXml: string | null;
  readOnly: boolean;
  catalog: SchemaCatalog | null;
  /** The xpath's statically-inferred target and terminal resolved field, computed once by the
   * sibling `PatchPathInput` field's shared completion result (`usePatchXPathCompletion`) and
   * passed down here rather than resolved independently -- see `PatchOperationForm`, which owns
   * the shared result. Both are `null` while there's no result yet, no project context, or the
   * xpath doesn't resolve to anything. */
  target: XPathTarget | null;
  resolvedField: XPathResolvedField | null;
  label: string;
  operationType: PatchValueOperationType;
  onChange: (value: string | null) => void;
}

/** Raw XML editor plus schema-backed structured subform for a patch operation's `<value>`
 * payload (issue 06). Raw XML is always available; structured mode is offered once the xpath
 * resolves to a known, structurally-supported Def field (or, for Add/Insert against a bare Def
 * node, once the user picks which direct field they're adding). See
 * docs/patches-editor/06-structured-patch-value-editor.md for the shape-support boundary. */
export function PatchValueEditor({
  valueXml,
  readOnly,
  catalog,
  target,
  resolvedField,
  label,
  operationType,
  onChange,
}: Props) {
  const { t } = useTranslation("patches");
  const [pickedFieldName, setPickedFieldName] = useState<string | null>(null);
  const [mode, setMode] = useState<"raw" | "structured">("raw");
  const [structuredValue, setStructuredValue] = useState<ObjectFieldValue | null>(null);
  const [structureError, setStructureError] = useState<string | null>(null);

  // Raw XML mode shows a dedented view of valueXml -- the prop itself carries whatever
  // indentation the value happened to have in the source file (preserved verbatim for
  // byte-for-byte round-tripping when untouched) -- so display it separately from the value
  // sent upstream. Mirrors suppressNextParseRef below: skip re-deriving the draft from our own
  // edit echoing back through valueXml, but pick up genuinely external changes (undo/redo).
  const [rawDraft, setRawDraft] = useState(() => dedentXmlFragment(nullToEmpty(valueXml)));
  const lastRawEchoRef = useRef<string | null>(null);
  useEffect(() => {
    if (lastRawEchoRef.current === valueXml) {
      lastRawEchoRef.current = null;
      return;
    }
    setRawDraft(dedentXmlFragment(nullToEmpty(valueXml)));
  }, [valueXml]);

  const directDefType = targetDefType(target);
  const showFieldPicker =
    !resolvedField && directDefType !== null && (operationType === "add" || operationType === "insert");
  const fieldOptions = showFieldPicker ? listDirectDefTypeFields(directDefType, catalog) : [];

  let editTarget: PatchValueEditTarget | null = null;
  if (operationType === "addModExtension") {
    const field = directDefType ? resolveModExtensionsField(catalog) : null;
    // The built-in schema pack declares `modExtensions` as a plain scalar `listOfLi` (no shared
    // "ModExtension" base type to discriminate `<li Class="...">` entries against), even though
    // real usage is always `<li Class="...">...</li>` objects, never scalar text. Only offer a
    // structured target once the schema genuinely declares an object item shape for it -- until
    // then this always falls back to raw XML, which the Plan explicitly allows ("raw or
    // object-list style editor"), rather than letting a scalar list editor create invalid
    // `<li>text</li>` entries.
    if (field && fieldSchemaToControl("modExtensions", field) === "objectList") {
      editTarget = { fieldName: "modExtensions", field };
    }
  } else if (resolvedField) {
    editTarget = { fieldName: resolvedField.fieldName, field: resolvedField.field };
  } else if (showFieldPicker && pickedFieldName) {
    const found = fieldOptions.find(([name]) => name === pickedFieldName);
    if (found) editTarget = { fieldName: found[0], field: found[1] };
  }

  const structurallySupported = editTarget !== null && isStructurallySupportedField(editTarget.field);

  // Reset the raw/structured toggle to this target's default whenever the *target* changes (a
  // different field picked, or the xpath now resolves to something else) -- but leave the user's
  // manual toggle alone while they keep editing the same field.
  const lastTargetKeyRef = useRef<string | null>(null);
  useEffect(() => {
    const key = editTarget ? `${operationType}:${editTarget.fieldName}` : null;
    if (key === lastTargetKeyRef.current) return;
    lastTargetKeyRef.current = key;
    // Insert defaults to raw ("raw-first unless sibling item shape is clear" -- see Plan.md);
    // every other supported kind defaults to structured when available.
    setMode(structurallySupported && operationType !== "insert" ? "structured" : "raw");
    setStructureError(null);
  }, [editTarget?.fieldName, operationType, structurallySupported]);

  // Parse the current valueXml into a structured value whenever entering structured mode, the
  // target field changes while already in it, or valueXml itself changes for a reason *other*
  // than our own last edit (undo/redo, or a hand-edit made while briefly in raw mode). Every edit
  // made *through* this component's structured fields records the XML it just emitted in
  // `suppressNextParseRef` so that its own echo back down through `valueXml` doesn't trigger a
  // redundant (and potentially racy) reparse of what's already reflected in `structuredValue`.
  const suppressNextParseRef = useRef<{ value: string | null } | null>(null);
  useEffect(() => {
    if (mode !== "structured" || !editTarget || !catalog) return;
    if (suppressNextParseRef.current && suppressNextParseRef.current.value === valueXml) {
      suppressNextParseRef.current = null;
      return;
    }
    suppressNextParseRef.current = null;
    let cancelled = false;
    const xml = (valueXml ?? "").trim();
    if (xml === "") {
      setStructuredValue(emptyFieldValue(editTarget.fieldName, editTarget.field, catalog));
      setStructureError(null);
      return;
    }
    parsePatchValueXml(valueXml ?? "")
      .then((views) => {
        if (cancelled) return;
        const result = parsedViewsToFieldValue(views, editTarget.fieldName, editTarget.field, catalog);
        if (result.kind === "ok") {
          setStructuredValue(result.value);
          setStructureError(null);
        } else if (result.kind === "empty") {
          setStructuredValue(emptyFieldValue(editTarget.fieldName, editTarget.field, catalog));
          setStructureError(null);
        } else if (result.kind === "mismatch") {
          setMode("raw");
          setStructureError(
            t("valueEditor.mismatch", {
              actualName: result.actualName,
              fieldName: editTarget.fieldName,
            }),
          );
        } else {
          setMode("raw");
          setStructureError(result.reason);
        }
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setMode("raw");
        setStructureError(formatError(e));
      });
    return () => {
      cancelled = true;
    };
  }, [mode, editTarget?.fieldName, editTarget?.field, valueXml, catalog]);

  const serializeRequestIdRef = useRef(0);
  const updateStructuredValue = useCallback(
    (next: ObjectFieldValue) => {
      setStructuredValue(next);
      if (!editTarget) return;
      const element = fieldValueToInitialElement(editTarget.fieldName, next);
      const elements = element ? [element] : [];
      const requestId = ++serializeRequestIdRef.current;
      serializePatchValueFragment(elements)
        .then((xml) => {
          if (serializeRequestIdRef.current !== requestId) return;
          const nextValueXml = elements.length ? xml : null;
          suppressNextParseRef.current = { value: nextValueXml };
          onChange(nextValueXml);
        })
        .catch(() => {
          // Transient serialize failure -- leave valueXml as the last known-good value.
        });
    },
    [editTarget, onChange],
  );

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <span className={styles.label}>{label}</span>
        <div className={styles.modeToggle}>
          <button
            type="button"
            data-active={mode === "raw"}
            className={styles.modeBtn}
            onClick={() => setMode("raw")}
          >
            {t("valueEditor.rawXml")}
          </button>
          <button
            type="button"
            data-active={mode === "structured"}
            className={styles.modeBtn}
            disabled={!structurallySupported}
            title={structurallySupported ? undefined : t("valueEditor.noStructuredEditor")}
            onClick={() => setMode("structured")}
          >
            {t("valueEditor.structured")}
          </button>
        </div>
      </div>

      {showFieldPicker && (
        <label className={styles.field}>
          <span className={styles.subLabel}>
            {operationType === "insert"
              ? t("valueEditor.fieldToInsert")
              : t("valueEditor.fieldToAdd")}
          </span>
          <select
            value={pickedFieldName ?? ""}
            disabled={readOnly}
            onChange={(e) => setPickedFieldName(e.target.value || null)}
          >
            <option value="">{t("valueEditor.chooseAField")}</option>
            {fieldOptions.map(([name, schema]) => (
              <option key={name} value={name}>
                {schema.label || name}
              </option>
            ))}
          </select>
        </label>
      )}

      {mode === "structured" && editTarget && structuredValue ? (
        <div className={styles.structuredBody}>
          <ValueFieldRenderer
            fieldName={editTarget.fieldName}
            field={editTarget.field}
            value={structuredValue}
            catalog={catalog!}
            readOnly={readOnly}
            onChange={updateStructuredValue}
          />
        </div>
      ) : (
        <textarea
          className={styles.rawXml}
          rows={4}
          spellCheck={false}
          // XML is machine-readable syntax, not natural-language prose -- keep it forced LTR even
          // once a future RTL locale flips `dir` on `<html>` (see
          // docs/i18n/issues/08-editor-and-patch-ui-migration.md's "keep code editor/XML/XPath
          // controls dir=ltr by semantic policy" carve-out).
          dir="ltr"
          value={rawDraft}
          disabled={readOnly}
          onChange={(e) => {
            setRawDraft(e.target.value);
            const next = emptyToNull(e.target.value);
            lastRawEchoRef.current = next;
            onChange(next);
          }}
        />
      )}

      {structureError && <div className={styles.error}>{structureError}</div>}
    </div>
  );
}
