import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import type { SchemaCatalog } from "../../../schema-catalog";
import { parsePatchOperations } from "../../api/patchDocument";
import {
  buildCustomOperationXml,
  extractOperationForSlot,
  wrapOperationForSlot,
  type CustomFieldValue,
  type NestedOperationSlot,
} from "../../lib/customOperationXml";
import { createBuiltInOperation, isBuiltInDefaultClass } from "../../lib/patchOperationDefaults";
import { formatError } from "../../../../lib/formatError";
import type { PatchOperationId, PatchOperationNode, PatchSuccessMode } from "../../types/patchFile";
import { PatchOperationTypePicker } from "../PatchOperationTypePicker/PatchOperationTypePicker";
import { PatchValueEditor } from "../PatchValueEditor/PatchValueEditor";
import styles from "./PatchAddOperationPanel.module.css";

interface Props {
  catalog: SchemaCatalog | null;
  generateId: () => PatchOperationId;
  onAdd: (node: PatchOperationNode) => void;
  /** Where the new operation will be inserted -- determines the wrapper element name (`Operation`
   * / `li` / `match` / `nomatch`) a custom operation's raw XML must use. Defaults to `"top"`. */
  slot?: NestedOperationSlot;
  /** Label for the trigger button, e.g. "Add operation" or "Add sequence item". Defaults to the
   * translated "Add operation" when omitted. */
  triggerLabel?: string;
}

/** "Add operation" flow: pick a type (built-in or metadata-defined custom), then either add it
 * immediately (built-ins, which have a typed default) or fill in a small metadata-driven field
 * form and build its XML client-side before adding it (custom classes, which the AST only
 * represents as an opaque `unknown` node -- see docs/patches-editor/04-patch-editor-ui.md for why
 * this stays client-side rather than becoming a new backend command). Manages its own
 * open/closed state -- renders as a trigger button when closed. */
export function PatchAddOperationPanel({
  catalog,
  generateId,
  onAdd,
  slot = "top",
  triggerLabel,
}: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["patches", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("patches");
  const { t: tCommon } = useTranslation("common");
  const [open, setOpen] = useState(false);
  const [customClassName, setCustomClassName] = useState<string | null>(null);
  const [values, setValues] = useState<Record<string, CustomFieldValue>>({});
  const [success, setSuccess] = useState<PatchSuccessMode>("normal");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setOpen(false);
    setCustomClassName(null);
    setValues({});
    setSuccess("normal");
    setError(null);
  }

  function handleSelect(className: string) {
    if (isBuiltInDefaultClass(className)) {
      onAdd(createBuiltInOperation(className, generateId()));
      reset();
      return;
    }
    setCustomClassName(className);
  }

  async function handleCreateCustom() {
    if (!customClassName || !catalog?.patchOperations) return;
    const metadata = catalog.patchOperations[customClassName];
    if (!metadata) return;
    setBusy(true);
    setError(null);
    try {
      const operationXml = buildCustomOperationXml(metadata, values, [], success, slot);
      const wrapped = wrapOperationForSlot(operationXml, slot);
      const file = await parsePatchOperations("", wrapped);
      const parsed = extractOperationForSlot(file, slot);
      if (!parsed) {
        setError(t("addOperationPanel.couldNotParse"));
        return;
      }
      const node: PatchOperationNode = { ...parsed, id: generateId() };
      onAdd(node);
      reset();
    } catch (e: unknown) {
      setError(formatError(e));
    } finally {
      setBusy(false);
    }
  }

  if (!open) {
    return (
      <button type="button" className={styles.trigger} onClick={() => setOpen(true)}>
        <Plus size={12} /> {triggerLabel ?? t("addOperationPanel.defaultTrigger")}
      </button>
    );
  }

  if (!customClassName) {
    return (
      <PatchOperationTypePicker catalog={catalog} onSelect={handleSelect} onCancel={reset} />
    );
  }

  const metadata = catalog?.patchOperations?.[customClassName];

  return (
    <div className={styles.customForm}>
      <div className={styles.customHeader}>{metadata?.label || customClassName}</div>
      {metadata?.fieldOrder.map((fieldName) => {
        const field = metadata.fields[fieldName];
        if (!field) return null;
        const current = values[fieldName]?.value ?? "";
        if (field.role === "xmlValue") {
          // Custom metadata fields aren't xpath-targeted (target=null, resolvedField=null), so
          // this always resolves to raw-only mode -- structured editing needs a resolved Def
          // schema field, which a custom operation's own field has no notion of.
          return (
            <div key={fieldName} className={styles.customField}>
              <PatchValueEditor
                valueXml={current || null}
                readOnly={false}
                catalog={catalog}
                target={null}
                resolvedField={null}
                operationType="custom"
                label={field.label || fieldName}
                onChange={(value) =>
                  setValues((prev) => ({ ...prev, [fieldName]: { kind: "xml", value: value ?? "" } }))
                }
              />
            </div>
          );
        }
        return (
          <label key={fieldName} className={styles.customField}>
            <span>{field.label || fieldName}</span>
            <input
              type="text"
              value={current}
              onChange={(e) =>
                setValues((prev) => ({ ...prev, [fieldName]: { kind: "text", value: e.target.value } }))
              }
            />
          </label>
        );
      })}
      <label className={styles.customField}>
        <span>{t("addOperationPanel.success")}</span>
        <select value={success} onChange={(e) => setSuccess(e.target.value as PatchSuccessMode)}>
          <option value="normal">{t("addOperationPanel.successNormal")}</option>
          <option value="invert">{t("addOperationPanel.successInvert")}</option>
          <option value="always">{t("addOperationPanel.successAlways")}</option>
          <option value="never">{t("addOperationPanel.successNever")}</option>
        </select>
      </label>
      {error && <div className={styles.error}>{error}</div>}
      <div className={styles.customActions}>
        <button type="button" onClick={handleCreateCustom} disabled={busy}>
          {t("addOperationPanel.create")}
        </button>
        <button type="button" onClick={reset} disabled={busy}>
          {tCommon("actions.cancel")}
        </button>
      </div>
    </div>
  );
}
