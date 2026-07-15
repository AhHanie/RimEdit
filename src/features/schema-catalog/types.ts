import type { DiagnosticArgs } from "../../lib/diagnostics";

export type SchemaLoadSeverity = "Error" | "Warning" | "Info";

export interface SchemaLoadDiagnostic {
  severity: SchemaLoadSeverity;
  code: string;
  message: string;
  packId?: string;
  path?: string;
  fieldPath?: string;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
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

/**
 * Schema-defined Form View declaration as authored in a Def-type pack JSON file's `formViews`
 * object, keyed by view id. This is the parse-time/source shape (mirrors Rust `FormViewDef`);
 * RimEdit's frontend never parses schema-pack JSON directly, so this type is not consumed by any
 * runtime code path yet -- it exists for symmetry with the Rust model and for future tooling.
 * The resolved shape that actually crosses the Tauri boundary is `SchemaFormView`.
 *
 * The id `"default"` is reserved for the synthetic Default View and must never be used here;
 * validation of that constraint is issue 02's job.
 */
export interface FormViewDef {
  /**
   * Optional (not required) because a child-schema *delta* amendment to an inherited view is
   * valid with no label at all -- e.g. `{ hiddenFields: [...] }` or `{ disabled: true }` (see
   * `Plan.md` section 4's `unhideFields`/`disabled` examples). A brand-new/base view declaration
   * is expected to always provide one in practice, but that is enforced by issue 02/03's
   * validation layer, not this type. The resolved `SchemaFormView.label` stays required.
   */
  label?: string;
  description?: string;
  /** Named icon token; no arbitrary SVG/URL. Token validation is deferred (issue 02+). */
  icon?: string;
  order?: number;
  recommended?: boolean;
  /** Canonical top-level Def schema field keys to hide (never XML aliases/nested paths). */
  hiddenFields?: string[];
  /** Subtractive delta against an inherited view's hidden set. */
  unhideFields?: string[];
  replace?: boolean;
  disabled?: boolean;
}

/** Source-pack provenance for a resolved schema-defined Form View. */
export interface FormViewSource {
  packId: string;
  packVersion: string;
}

/**
 * A resolved schema-defined Form View in the merged catalog, keyed by view id on
 * `DefTypeSchema.formViews`. Empty/absent until issue 03 implements inheritance/pack-precedence
 * resolution -- issue 01 only establishes the shape. The id `"default"` is reserved for the
 * synthetic Default View and never appears as a key here.
 */
export interface SchemaFormView {
  id: string;
  label: string;
  description?: string;
  icon?: string;
  order: number;
  recommended: boolean;
  /** Canonical top-level Def schema field keys hidden by this view. */
  hiddenFieldIds: string[];
  /** The concrete Def type whose declaration is the winning source for this resolved view. */
  declaredOnDefType: string;
  source?: FormViewSource;
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
  formViews?: Record<string, SchemaFormView>;
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
