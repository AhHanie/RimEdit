import type { ReferenceMetadata, TypedReferenceMetadata, ValidationHints, XmlFieldShape } from "../../schema-catalog";
import type { ValidationDiagnostic } from "./xmlDocument";

export interface FormSectionDefaults {
  path: string[];
  defaultCollapsed?: boolean;
  hasData?: boolean;
}

export type FormControlKind =
  | "text"
  | "textarea"
  | "number"
  | "checkbox"
  | "select"
  | "reference"
  | "list"
  | "object"
  | "objectList"
  | "namedMap"
  | "flags"
  | "typedReferenceList"
  | "color"
  | "readonlyUnknown";

export interface FormFieldDescriptor {
  key: string;
  label: string;
  description?: string;
  control: FormControlKind;
  value: unknown;
  defaultValue?: unknown;
  examples: string[];
  required: boolean;
  repeatable: boolean;
  xmlShape: XmlFieldShape;
  fieldPath: string[];
  nodeId: number | null;
  dirty: boolean;
  readonly: boolean;
  readOnlyReason?: string;
  diagnostics: ValidationDiagnostic[];
  allowedValues?: string[];
  validationHints?: ValidationHints;
  reference?: ReferenceMetadata;
  /** For namedMap controls: reference metadata applied to the key inputs. */
  keyReference?: ReferenceMetadata;
  typedReference?: TypedReferenceMetadata;
  /** For objectList controls: the schema ref of each item (e.g. "CompProperties"). */
  itemSchemaRef?: string;
  /** For keyedObjectList controls: the field name in each item that holds the element key. */
  keyField?: string;
  /** For keyedObjectList controls: the field whose value is encoded as scalar text on the item element. */
  defaultValueField?: string;
  /** Ancestry chain of section defaults, one entry per nesting level. */
  sectionDefaults: FormSectionDefaults[];
  /** When a field was matched via an XML alias, the actual element name present in the document. */
  xmlElementName?: string;
}

export type FormFieldId = string;

export type FormFieldPath =
  | {
      kind: "childElement";
      childName: string;
    }
  | {
      kind: "attribute";
      attributeName: string;
    }
  | {
      kind: "nestedObjectField";
      objectPath: string[];
      fieldName: string;
    }
  | {
      kind: "listItems";
      childName: string;
    }
  | {
      kind: "nestedListItems";
      objectPath: string[];
      fieldName: string;
    }
  | {
      kind: "namedMap";
      objectPath: string[];
      mapName: string;
    }
  | {
      kind: "objectList";
      objectPath: string[];
      fieldName: string;
    }
  | {
      kind: "unknownChild";
      childName: string;
      nodeId: number;
    }
  | {
      kind: "typedReferenceList";
      objectPath: string[];
      fieldName: string;
    }
  | {
      kind: "nestedAttribute";
      objectPath: string[];
      attributeName: string;
    };

/** One item in a typed-reference list (e.g. `descriptionHyperlinks`). */
export interface TypedReferenceItem {
  /** Node id of the child element; null for newly added items. */
  nodeId: number | null;
  /** Target def type (child element name, e.g. `ThingDef`). */
  defType: string;
  /** Target def name (child element text). */
  defName: string;
}

/**
 * Recursive field value for object-list items and nested object fields.
 * Replaces the old flat ObjectListItemFieldValue with full support for
 * nested objects, nested object lists, and all other schema-backed shapes.
 */
export type ObjectFieldValue =
  | { kind: "scalar"; value: string }
  | { kind: "boolean"; value: boolean }
  | { kind: "enum"; value: string }
  | { kind: "list"; items: string[] }
  | { kind: "flags"; selected: string[]; custom: string[]; xmlShape?: XmlFieldShape }
  | { kind: "namedMap"; entries: { key: string; value: string }[] }
  | { kind: "typedReferenceList"; items: TypedReferenceItem[] }
  | {
      kind: "object";
      schemaRef: string | null;
      fields: Record<string, ObjectFieldValue>;
      nodeId: number | null;
      initialUnknownFieldCount: number;
      fieldXmlNames?: Record<string, string>;
      /** Canonical field ordering from the schema, used for edit serialization. */
      fieldOrder: string[];
      /** Names of `fields` entries that are XML attributes on this object element (not child elements). */
      xmlAttributeFields?: string[];
    }
  | { kind: "objectList"; itemSchemaRef: string; items: ObjectListItemValue[] }
  | { kind: "readonly"; reason: string };

/** @deprecated Use ObjectFieldValue. Kept for source compatibility. */
export type ObjectListItemFieldValue = ObjectFieldValue;

/** One item in an editable object list (e.g. a ThingDef comp). */
export interface ObjectListItemValue {
  /** Node id of the `<li>` element; null for newly added items. */
  nodeId: number | null;
  /** Stable client-side identifier for new items (no nodeId yet). Used as React key to prevent
   *  key-shifting when multiple unsaved items are reordered or removed. */
  clientId?: string;
  /** Value of the discriminator attribute (e.g. `Class`). */
  className: string;
  /** Resolved object type name from the discriminator, or null when unknown. */
  schemaRef: string | null;
  /** Field values keyed by canonical field name. */
  fields: Record<string, ObjectFieldValue>;
  /** Number of element children not covered by the resolved schema. */
  initialUnknownFieldCount: number;
  /** Maps canonical field name → actual XML element name when they differ (alias case). */
  fieldXmlNames?: Record<string, string>;
  /** Effective field insertion order (inherited + own), used so edits can place new child
   *  elements in schema-defined order without a catalog lookup at diff time. */
  fieldOrder?: string[];
  /** Names of `fields` entries that are XML attributes on the item element (not child elements). */
  attributeFields?: string[];
  /** True when the item was loaded via defaultValueField shorthand (scalar text on the item element). */
  defaultValueFieldShorthand?: boolean;
}

export type FormValue =
  | {
      kind: "scalar";
      value: string;
    }
  | {
      kind: "boolean";
      value: boolean;
    }
  | {
      kind: "enum";
      value: string;
    }
  | {
      kind: "list";
      items: string[];
    }
  | {
      kind: "flags";
      /** Known flag values that are selected. */
      selected: string[];
      /** Unknown flag values preserved from the original XML. */
      custom: string[];
    }
  | {
      kind: "namedMap";
      entries: { key: string; value: string }[];
    }
  | {
      kind: "readonly";
      value: string;
    }
  | {
      kind: "objectList";
      items: ObjectListItemValue[];
    }
  | {
      kind: "typedReferenceList";
      items: TypedReferenceItem[];
    };

export interface FormFieldModel {
  id: FormFieldId;
  key: string;
  label: string;
  description?: string;
  control: FormControlKind;
  defaultValue?: unknown;
  examples: string[];
  required: boolean;
  repeatable: boolean;
  xmlShape: XmlFieldShape;
  path: FormFieldPath;
  fieldPath: string[];
  sourceNodeId: number | null;
  defNodeId: number;
  order: number;
  readonly: boolean;
  readOnlyReason?: string;
  diagnostics: ValidationDiagnostic[];
  allowedValues?: string[];
  validationHints?: ValidationHints;
  reference?: ReferenceMetadata;
  /** For namedMap controls: reference metadata applied to the key inputs. */
  keyReference?: ReferenceMetadata;
  typedReference?: TypedReferenceMetadata;
  /** For objectList controls: the schema ref of each item (e.g. "CompProperties"). */
  itemSchemaRef?: string;
  /** For keyedObjectList controls: the field name in each item that holds the element key. */
  keyField?: string;
  /** For keyedObjectList controls: the field whose value is encoded as scalar text on the item element. */
  defaultValueField?: string;
  /** Ancestry chain of section defaults, one entry per nesting level. */
  sectionDefaults: FormSectionDefaults[];
}

export interface FormFieldState {
  model: FormFieldModel;
  value: FormValue;
  initialValue: FormValue;
  dirty: boolean;
  touched: boolean;
  focused: boolean;
  pending: boolean;
  error: string | null;
  validationErrors: string[];
  clearRequested: boolean;
}

export interface FormSnapshot {
  defNodeId: number;
  fields: FormFieldState[];
}

export type FormEditIntent =
  | {
      type: "setValue";
      fieldId: FormFieldId;
      value: FormValue;
    }
  | {
      type: "resetField";
      fieldId: FormFieldId;
    };

export interface FormCommitResult {
  rawXml: string;
  changedFieldIds: FormFieldId[];
}
