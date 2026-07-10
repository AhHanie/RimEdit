export type SchemaLoadSeverity = "Error" | "Warning" | "Info";

export interface SchemaLoadDiagnostic {
  severity: SchemaLoadSeverity;
  code: string;
  message: string;
  packId?: string;
  path?: string;
  fieldPath?: string;
}

export type FieldTypeKind =
  | "string"
  | "localizedString"
  | "integer"
  | "float"
  | "boolean"
  | "enum"
  | "defReference"
  | "typeName"
  | "vector2"
  | "vector3"
  | "list"
  | "object"
  | "statMap"
  | "intRange"
  | "floatRange"
  | "color"
  | "unknown";

export interface FieldType {
  kind: FieldTypeKind;
  schemaRef?: string;
  /** Reference metadata for `items.kind = "defReference"` list items. */
  reference?: ReferenceMetadata;
}

export type XmlFieldShape =
  | "element"
  | "attribute"
  | "listOfLi"
  | "namedChildrenMap"
  | "keyedValueList"
  | "object"
  | "text"
  | "flagsText"
  | "typedReferenceList"
  | "keyedObjectList"
  | "keyedObjectMap";

/** Role a patch operation field plays. Never set on Def/object type fields -- only meaningful on
 * `PatchOperationMetadata` fields. */
export type PatchOperationFieldRole = "xpath" | "xmlValue" | "operation" | "operationList";

export type ReferenceScope = "allSources" | "projectOnly" | "samePack";

export interface ReferenceMetadata {
  defType: string;
  allowAbstract: boolean;
  scope: ReferenceScope;
  /** When present and non-empty, these concrete Def types are searched instead of defType. */
  acceptedDefTypes?: string[];
}

/** Metadata for `typedReferenceList` fields where the def type is dynamic per row. */
export interface TypedReferenceMetadata {
  allowAbstract: boolean;
  scope: ReferenceScope;
}

export interface ValidationHints {
  pattern?: string;
  min?: number;
  max?: number;
  allowedValues?: string[];
}

export type ValidationRuleOperator =
  | "equals"
  | "notEquals"
  | "greaterThan"
  | "greaterThanOrEqual"
  | "lessThan"
  | "lessThanOrEqual"
  | "present"
  | "absent";

export interface ValidationRuleCondition {
  field: string;
  operator: ValidationRuleOperator;
  value?: unknown;
}

export type ValidationRule = {
  kind: "requiredWhen";
  field: string;
  when: ValidationRuleCondition;
  message: string;
};

export interface FieldSchema {
  label?: string;
  description?: string;
  type: FieldType;
  required: boolean;
  defaultValue?: unknown;
  examples: string[];
  validationHints?: ValidationHints;
  xmlAliases?: string[];
  reference?: ReferenceMetadata;
  /** For namedChildrenMap/keyedValueList fields: the def type referenced by child element names. */
  keyReference?: ReferenceMetadata;
  /** For typedReferenceList fields: metadata for typed reference child lists. */
  typedReference?: TypedReferenceMetadata;
  /** For keyedValueList and keyedObjectList fields: logical item field populated from the child element name. */
  keyField?: string;
  /** For keyedValueList fields: logical item field populated from the child element text. */
  valueField?: string;
  /** For keyedObjectList fields: logical item field populated from a single text node (defaultValueField). */
  defaultValueField?: string;
  repeatable: boolean;
  xml: XmlFieldShape;
  sourcePackId?: string;
  items?: FieldType;
  flags: boolean;
  defaultCollapsed?: boolean;
  /** Only meaningful on `PatchOperationMetadata` fields; always undefined on Def/object fields. */
  role?: PatchOperationFieldRole;
}

export type TemplateFieldValue = string | number | boolean | string[];

export interface DefTemplate {
  id: string;
  label: string;
  description?: string;
  includeRequiredFields: boolean;
  promptFields: string[];
  fieldValues: Record<string, TemplateFieldValue>;
  sourcePackId?: string;
}

export interface DefTypeSchema {
  label?: string;
  description?: string;
  inherits: string[];
  abstractType: boolean;
  fieldOrder: string[];
  fields: Record<string, FieldSchema>;
  templates?: Record<string, DefTemplate>;
  validationRules?: Record<string, ValidationRule>;
}

export type SchemaPackSourceKind = "builtIn" | "external";

export interface LoadedSchemaPackSummary {
  packId: string;
  name: string;
  version: string;
  formatVersion: number;
  gameVersion: string | null;
  rimeditVersion: string | null;
  author: string | null;
  dependencies: string[];
  priority: number;
  sourceKind: SchemaPackSourceKind;
  path?: string;
}

export interface ObjectTypeDiscriminator {
  attribute: string;
  fallbackSchemaRef?: string;
  allowMissing: boolean;
  allowUnknown: boolean;
  variants: Record<string, string>;
}

export interface ObjectTypeSchema {
  label?: string;
  description?: string;
  /** Object type names whose fields are inherited. Defaults to [] when absent. */
  inherits?: string[];
  fieldOrder: string[];
  fields: Record<string, FieldSchema>;
  discriminator?: ObjectTypeDiscriminator;
}

/** Only `"unsupported"` is meaningful today; declarative preview behaviors are a documented
 * future extension (see `docs/patches-editor/Plan.md`). */
export type PatchOperationPreviewKind = "unsupported";

export interface PatchOperationPreview {
  kind: PatchOperationPreviewKind;
  message?: string;
}

/** Resolved patch operation metadata, keyed by `className` on `SchemaCatalog.patchOperations`.
 * Covers both built-in operations (shipped as metadata so they render through the same form path
 * as custom operations) and user/mod-defined custom operations. */
export interface PatchOperationMetadata {
  className: string;
  label?: string;
  description?: string;
  fieldOrder: string[];
  fields: Record<string, FieldSchema>;
  preview: PatchOperationPreview;
  sourcePackId?: string;
}

export interface SchemaCatalog {
  formatVersion: 1;
  packs: LoadedSchemaPackSummary[];
  defTypes: Record<string, DefTypeSchema>;
  objectTypes: Record<string, ObjectTypeSchema>;
  /** Keyed by `className`. Includes both built-in and custom/mod-defined patch operations.
   * Optional here (the real backend always sends it) so existing test fixtures that build a
   * `SchemaCatalog` literal without patch operation data don't all need updating. */
  patchOperations?: Record<string, PatchOperationMetadata>;
}

export interface SchemaCatalogLoadResult {
  catalog: SchemaCatalog;
  diagnostics: SchemaLoadDiagnostic[];
}
