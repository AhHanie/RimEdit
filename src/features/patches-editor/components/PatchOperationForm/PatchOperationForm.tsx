import { Plus, X } from "lucide-react";
import type { SchemaCatalog } from "../../../schema-catalog";
import { emptyToNull, insertAt, nullToEmpty, removeAt, replaceAt } from "../../lib/arrayUtils";
import type {
  AttributeOperationData,
  AttributeValueOperationData,
  PathedOperationData,
  PathedValueOperationData,
  PatchOperationKind,
  PatchOperationNode,
  PatchOrderMode,
  PatchSuccessMode,
  SetNameOperationData,
} from "../../types/patchFile";
import { PatchPathInput } from "../PatchPathInput/PatchPathInput";
import { PatchValueEditor } from "../PatchValueEditor/PatchValueEditor";
import styles from "./PatchOperationForm.module.css";

interface Props {
  node: PatchOperationNode;
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  onChange: (updater: (node: PatchOperationNode) => PatchOperationNode) => void;
}

// "sequence" and "unknown" have no scalar data field this form can patch (their content is a
// nested operation list / raw XML, both handled outside this component). "findMod" (`mods`) and
// "conditional" (`xpath`) DO have a scalar field alongside their match/nomatch slots, so they must
// stay patchable here -- excluding them previously made their form inputs silent no-ops.
type FlatDataKind = Exclude<PatchOperationKind, { type: "sequence" | "unknown" }>;

function fieldLabel(
  catalog: SchemaCatalog | null,
  className: string,
  fieldName: string,
  fallback: string,
): string {
  return catalog?.patchOperations?.[className]?.fields[fieldName]?.label || fallback;
}

export function PatchOperationForm({ node, catalog, readOnly, projectId, onChange }: Props) {
  function updateData(patch: Partial<FlatDataKind["data"]>) {
    onChange((n) => {
      const kind = n.kind;
      if (kind.type === "sequence" || kind.type === "unknown") {
        return n;
      }
      // Merging only the patched keys leaves findMod/conditional's matchOp/nomatchOp (managed
      // separately by PatchOperationNodeRow's nested tree, not this form) untouched.
      return { ...n, kind: { ...kind, data: { ...kind.data, ...patch } } as PatchOperationKind };
    });
  }

  function setSuccess(success: PatchSuccessMode) {
    onChange((n) => ({ ...n, success }));
  }

  function setNamedAttribute(name: string, value: string) {
    onChange((n) => {
      const rest = n.attributes.filter((a) => a.name !== name);
      return { ...n, attributes: value === "" ? rest : [...rest, { name, value }] };
    });
  }

  function namedAttribute(name: string): string {
    return node.attributes.find((a) => a.name === name)?.value ?? "";
  }

  const otherAttributes = node.attributes.filter(
    (a) => a.name !== "MayRequire" && a.name !== "MayRequireAnyOf",
  );

  function setOtherAttribute(index: number, field: "name" | "value", value: string) {
    onChange((n) => {
      const known = n.attributes.filter((a) => a.name === "MayRequire" || a.name === "MayRequireAnyOf");
      const other = n.attributes.filter((a) => a.name !== "MayRequire" && a.name !== "MayRequireAnyOf");
      const updated = replaceAt(other, index, { ...other[index], [field]: value });
      return { ...n, attributes: [...known, ...updated] };
    });
  }

  function addOtherAttribute() {
    onChange((n) => ({ ...n, attributes: [...n.attributes, { name: "", value: "" }] }));
  }

  function removeOtherAttribute(index: number) {
    onChange((n) => {
      const known = n.attributes.filter((a) => a.name === "MayRequire" || a.name === "MayRequireAnyOf");
      const other = n.attributes.filter((a) => a.name !== "MayRequire" && a.name !== "MayRequireAnyOf");
      return { ...n, attributes: [...known, ...removeAt(other, index)] };
    });
  }

  const kind = node.kind;

  return (
    <div className={styles.form}>
      <div className={styles.commonRow}>
        <label className={styles.field}>
          <span className={styles.label}>Success</span>
          <select
            value={node.success}
            disabled={readOnly}
            onChange={(e) => setSuccess(e.target.value as PatchSuccessMode)}
          >
            <option value="normal">Normal</option>
            <option value="invert">Invert</option>
            <option value="always">Always</option>
            <option value="never">Never</option>
          </select>
        </label>
        <label className={styles.field}>
          <span className={styles.label}>MayRequire</span>
          <input
            type="text"
            value={namedAttribute("MayRequire")}
            disabled={readOnly}
            placeholder="mod.package.id"
            onChange={(e) => setNamedAttribute("MayRequire", e.target.value)}
          />
        </label>
        <label className={styles.field}>
          <span className={styles.label}>MayRequireAnyOf</span>
          <input
            type="text"
            value={namedAttribute("MayRequireAnyOf")}
            disabled={readOnly}
            placeholder="mod.a,mod.b"
            onChange={(e) => setNamedAttribute("MayRequireAnyOf", e.target.value)}
          />
        </label>
      </div>

      {kindFields(kind, node.className, catalog, readOnly, projectId, updateData)}

      {otherAttributes.length > 0 || !readOnly ? (
        <div className={styles.otherAttributes}>
          <span className={styles.label}>Other attributes</span>
          {otherAttributes.map((attr, i) => (
            <div key={i} className={styles.attrRow}>
              <input
                type="text"
                value={attr.name}
                disabled={readOnly}
                placeholder="Name"
                onChange={(e) => setOtherAttribute(i, "name", e.target.value)}
              />
              <input
                type="text"
                value={attr.value}
                disabled={readOnly}
                placeholder="Value"
                onChange={(e) => setOtherAttribute(i, "value", e.target.value)}
              />
              {!readOnly && (
                <button type="button" className={styles.iconBtn} onClick={() => removeOtherAttribute(i)} aria-label="Remove attribute">
                  <X size={12} />
                </button>
              )}
            </div>
          ))}
          {!readOnly && (
            <button type="button" className={styles.addBtn} onClick={addOtherAttribute}>
              <Plus size={12} /> Add attribute
            </button>
          )}
        </div>
      ) : null}
    </div>
  );
}

function kindFields(
  kind: PatchOperationKind,
  className: string,
  catalog: SchemaCatalog | null,
  readOnly: boolean,
  projectId: string | null,
  updateData: (patch: Record<string, unknown>) => void,
) {
  const label = (field: string, fallback: string) => fieldLabel(catalog, className, field, fallback);
  const xpathField = (value: string | null, onChange: (xpath: string | null) => void) => (
    <PatchPathInput value={value} readOnly={readOnly} label={label("xpath", "XPath")} projectId={projectId} onChange={onChange} />
  );

  switch (kind.type) {
    case "add":
    case "insert": {
      const data = kind.data;
      return (
        <>
          {xpathField(data.xpath, (xpath) => updateData({ xpath }))}
          <PatchValueEditor
            valueXml={data.valueXml}
            xpath={data.xpath}
            readOnly={readOnly}
            catalog={catalog}
            projectId={projectId}
            operationType={kind.type}
            label={label("value", "Value")}
            onChange={(valueXml) => updateData({ valueXml })}
          />
          <OrderField value={data.order} readOnly={readOnly} onChange={(order) => updateData({ order })} />
        </>
      );
    }
    case "remove":
    case "test": {
      const data = kind.data as PathedOperationData;
      return xpathField(data.xpath, (xpath) => updateData({ xpath }));
    }
    case "replace":
    case "addModExtension": {
      const data = kind.data as PathedValueOperationData;
      return (
        <>
          {xpathField(data.xpath, (xpath) => updateData({ xpath }))}
          <PatchValueEditor
            valueXml={data.valueXml}
            xpath={data.xpath}
            readOnly={readOnly}
            catalog={catalog}
            projectId={projectId}
            operationType={kind.type}
            label={label("value", "Value")}
            onChange={(valueXml) => updateData({ valueXml })}
          />
        </>
      );
    }
    case "attributeAdd":
    case "attributeSet": {
      const data = kind.data as AttributeValueOperationData;
      return (
        <>
          {xpathField(data.xpath, (xpath) => updateData({ xpath }))}
          <TextField
            value={data.attribute}
            readOnly={readOnly}
            label={label("attribute", "Attribute")}
            onChange={(attribute) => updateData({ attribute })}
          />
          <TextField value={data.value} readOnly={readOnly} label={label("value", "Value")} onChange={(value) => updateData({ value })} />
        </>
      );
    }
    case "attributeRemove": {
      const data = kind.data as AttributeOperationData;
      return (
        <>
          {xpathField(data.xpath, (xpath) => updateData({ xpath }))}
          <TextField
            value={data.attribute}
            readOnly={readOnly}
            label={label("attribute", "Attribute")}
            onChange={(attribute) => updateData({ attribute })}
          />
        </>
      );
    }
    case "setName": {
      const data = kind.data as SetNameOperationData;
      return (
        <>
          {xpathField(data.xpath, (xpath) => updateData({ xpath }))}
          <TextField value={data.name} readOnly={readOnly} label={label("name", "Name")} onChange={(name) => updateData({ name })} />
        </>
      );
    }
    case "conditional":
      return xpathField(kind.data.xpath, (xpath) => updateData({ xpath }));
    case "findMod":
      return <ModsListField mods={kind.data.mods} readOnly={readOnly} onChange={(mods) => updateData({ mods })} />;
    case "sequence":
    case "unknown":
      return null;
  }
}

function TextField({
  value,
  readOnly,
  label,
  onChange,
}: {
  value: string | null;
  readOnly: boolean;
  label: string;
  onChange: (value: string | null) => void;
}) {
  return (
    <label className={styles.field}>
      <span className={styles.label}>{label}</span>
      <input
        type="text"
        value={nullToEmpty(value)}
        disabled={readOnly}
        onChange={(e) => onChange(emptyToNull(e.target.value))}
      />
    </label>
  );
}

function OrderField({
  value,
  readOnly,
  onChange,
}: {
  value: PatchOrderMode | null;
  readOnly: boolean;
  onChange: (value: PatchOrderMode | null) => void;
}) {
  return (
    <label className={styles.field}>
      <span className={styles.label}>Order</span>
      <select
        value={value ?? ""}
        disabled={readOnly}
        onChange={(e) => onChange(e.target.value === "" ? null : (e.target.value as PatchOrderMode))}
      >
        <option value="">(default)</option>
        <option value="append">Append</option>
        <option value="prepend">Prepend</option>
      </select>
    </label>
  );
}

function ModsListField({
  mods,
  readOnly,
  onChange,
}: {
  mods: string[];
  readOnly: boolean;
  onChange: (mods: string[]) => void;
}) {
  return (
    <div className={styles.field}>
      <span className={styles.label}>Mods</span>
      {mods.map((mod, i) => (
        <div key={i} className={styles.attrRow}>
          <input
            type="text"
            value={mod}
            disabled={readOnly}
            placeholder="Mod name"
            onChange={(e) => onChange(replaceAt(mods, i, e.target.value))}
          />
          {!readOnly && (
            <button type="button" className={styles.iconBtn} onClick={() => onChange(removeAt(mods, i))} aria-label="Remove mod">
              <X size={12} />
            </button>
          )}
        </div>
      ))}
      {!readOnly && (
        <button type="button" className={styles.addBtn} onClick={() => onChange(insertAt(mods, mods.length, ""))}>
          <Plus size={12} /> Add mod
        </button>
      )}
    </div>
  );
}
