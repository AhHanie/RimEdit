use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub enum SchemaLoadSeverity {
    Error,
    Warning,
    // Reserved for non-blocking loader notes once schema pack dependency handling is surfaced.
    #[allow(dead_code)]
    Info,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaLoadDiagnostic {
    pub severity: SchemaLoadSeverity,
    pub code: String,
    pub message: String,
    pub pack_id: Option<String>,
    pub path: Option<String>,
    pub field_path: Option<String>,
}

impl SchemaLoadDiagnostic {
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: SchemaLoadSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            pack_id: None,
            path: None,
            field_path: None,
        }
    }

    pub fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: SchemaLoadSeverity::Warning,
            code: code.to_string(),
            message: message.into(),
            pack_id: None,
            path: None,
            field_path: None,
        }
    }

    pub fn with_pack_id(mut self, pack_id: impl Into<String>) -> Self {
        self.pack_id = Some(pack_id.into());
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_path = Some(field_path.into());
        self
    }
}

// FieldType is a struct so #[serde(other)] works on the kind enum.
// JSON shape: { "kind": "string" }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum FieldTypeKind {
    String,
    LocalizedString,
    Integer,
    Float,
    Boolean,
    Enum,
    DefReference,
    TypeName,
    Vector2,
    Vector3,
    List,
    Object,
    StatMap,
    /// RimWorld `IntRange` - compact text format `min~max` (e.g. `"5~10"`).
    IntRange,
    /// RimWorld `FloatRange` - compact text format `min~max` (e.g. `"0.9~1.1"`).
    FloatRange,
    /// RimWorld `UnityEngine.Color` - parenthesised comma-separated 3- or 4-component tuple.
    /// Accepts integer (0–255) or float (0–1) components, e.g. `"(118, 49, 57)"` or `"(0.1, 0.1, 0.1)"`.
    Color,
    Unknown,
    // Transient parse artifact only. parse_schema_pack normalizes this to Unknown
    // before returning, so external callers never observe this variant.
    #[serde(other)]
    Unrecognized,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldType {
    pub kind: FieldTypeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
    /// Reference metadata for `items.kind = "defReference"` list items.
    /// Lifted to `FieldSchema.reference` by descriptor builders for reference-aware list rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<ReferenceMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum XmlFieldShape {
    Element,
    Attribute,
    ListOfLi,
    NamedChildrenMap,
    KeyedValueList,
    Object,
    Text,
    /// Comma-separated flags text in a single XML element (e.g. `"Child, Adult"`).
    /// Used for RimWorld fields like `developmentalStageFilter`.
    FlagsText,
    /// Container whose element children are named by target def type and contain the def name.
    /// Used for `descriptionHyperlinks`; allows repeated child element names.
    TypedReferenceList,
    /// Container whose element children are keyed object entries.
    /// Each child element name is the item key (e.g. a DefName); the child's own children are
    /// the object fields. Used for RimWorld `XmlHelper.ParseElements` containers such as
    /// `PrefabDef.things`, `PrefabDef.prefabs`, and `PrefabDef.terrain`.
    KeyedObjectList,
    /// Container whose element children are `<li>` items, each with a `<key>` scalar child and a
    /// `<value>` object child. Used for RimWorld `Dictionary<K,V>` fields such as
    /// `AnimationDef.keyframeParts` and `AnimationDef.curveParts`.
    KeyedObjectMap,
}

/// Role a patch operation field plays when RimEdit resolves patch metadata fields, distinct
/// from `XmlFieldShape` (which describes how the field is written to XML). Never set on
/// Def/object type fields -- only meaningful on `PatchOperationMetadata` fields.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PatchOperationFieldRole {
    /// The field holds an XPath expression targeting a node in the combined Def document.
    Xpath,
    /// The field holds XML content to insert/replace (e.g. `PatchOperationAdd`'s `value`).
    XmlValue,
    /// The field holds a single nested patch operation (e.g. `PatchOperationConditional`'s `match`).
    Operation,
    /// The field holds a list of nested patch operations (e.g. `PatchOperationSequence`'s `operations`).
    OperationList,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceScope {
    AllSources,
    ProjectOnly,
    SamePack,
}

fn default_reference_scope() -> ReferenceScope {
    ReferenceScope::AllSources
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedReferenceMetadata {
    #[serde(default)]
    pub allow_abstract: bool,
    #[serde(default = "default_reference_scope")]
    pub scope: ReferenceScope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceMetadata {
    pub def_type: String,
    #[serde(default)]
    pub allow_abstract: bool,
    #[serde(default = "default_reference_scope")]
    pub scope: ReferenceScope,
    /// Accepted concrete Def types for polymorphic base references.
    /// When present and non-empty, suggestions/resolution/validation search these types
    /// instead of (not in addition to) the nominal def_type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_def_types: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationHints {
    pub pattern: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub allowed_values: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ValidationRuleOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Present,
    Absent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationRuleCondition {
    pub field: String,
    pub operator: ValidationRuleOperator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ValidationRule {
    #[serde(rename = "requiredWhen")]
    RequiredWhen {
        field: String,
        when: ValidationRuleCondition,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Parse-time types (Deserialize only)
// Used in SchemaPackManifest - all scalar override fields are Option<T> so
// absent values are distinguishable from explicit overrides.
// ---------------------------------------------------------------------------

/// Field definition as it appears in a pack JSON file.
///
/// `required`, `repeatable`, `xml`, and `default_value` are all `Option` so
/// that the merge layer can tell the difference between "this pack didn't
/// mention the field" and "this pack explicitly set it to false / element /
/// null". Only `field_type` is always required (it defines the field).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSchemaDef {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    pub required: Option<bool>,
    pub default_value: Option<serde_json::Value>,
    #[serde(default)]
    pub examples: Vec<String>,
    pub validation_hints: Option<ValidationHints>,
    pub reference: Option<ReferenceMetadata>,
    /// For `namedChildrenMap` fields: the def type referenced by the map keys
    /// (e.g. `"StatDef"` for `equippedStatOffsets`, `"ThingDef"` for `killedLeavings`).
    pub key_reference: Option<ReferenceMetadata>,
    /// For `typedReferenceList` fields: metadata for typed reference child lists
    /// (e.g. `descriptionHyperlinks`). The def type is dynamic per row (child element name).
    pub typed_reference: Option<TypedReferenceMetadata>,
    /// For `keyedValueList` and `keyedObjectList` fields: logical item field populated from the child element name.
    pub key_field: Option<String>,
    /// For `keyedValueList` fields: logical item field populated from the child element text.
    pub value_field: Option<String>,
    /// For `keyedObjectList` fields: logical item field populated from a single text node when the
    /// child element has no element children (mirrors `XmlHelper.ParseElements` default-value behavior).
    pub default_value_field: Option<String>,
    /// For `namedChildrenMap` fields: expected scalar type of each child element's text content.
    pub value_type: Option<FieldType>,
    pub repeatable: Option<bool>,
    pub xml: Option<XmlFieldShape>,
    /// Item type descriptor for list fields (`type.kind = "list"`).
    pub items: Option<FieldType>,
    /// When true, this list field serializes each value as a separate `<li>` and
    /// the UI renders checkboxes from `validationHints.allowedValues`.
    pub flags: Option<bool>,
    /// When set, controls whether the object-backed editor section starts collapsed.
    /// Only meaningful on fields whose `type.kind` is `object`.
    pub default_collapsed: Option<bool>,
    /// Alternative XML element names that load into this field (source `[LoadAlias]`).
    #[serde(default)]
    pub xml_aliases: Vec<String>,
    /// Only meaningful on `PatchOperationMetadata` fields (e.g. `xpath`, `xmlValue`, `operation`,
    /// `operationList`); always `None` on Def/object type fields. Patch operation metadata reuses
    /// `FieldSchemaDef`/`FieldSchema` rather than a parallel field type so built-in and custom
    /// operation forms can share one rendering path with Def/object field forms.
    #[serde(default)]
    pub role: Option<PatchOperationFieldRole>,
}

/// Template field value: any JSON scalar or array (string, number, bool, or array of strings).
pub type TemplateFieldValue = serde_json::Value;

/// A def-creation template as it appears in a schema pack JSON file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefTemplateDef {
    pub label: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub include_required_fields: bool,
    #[serde(default)]
    pub prompt_fields: Vec<String>,
    #[serde(default)]
    pub field_values: BTreeMap<String, TemplateFieldValue>,
}

fn default_true() -> bool {
    true
}

/// Discriminator metadata for polymorphic object-list items - parse-time type.
///
/// Boolean flags are `Option` so the merge layer can distinguish "explicitly
/// set to false" from "not mentioned in this pack" and avoid clobbering values
/// already established by a higher-priority pack.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectTypeDiscriminatorDef {
    pub attribute: String,
    pub fallback_schema_ref: Option<String>,
    /// None = not mentioned in this pack (do not override the base value).
    pub allow_missing: Option<bool>,
    /// None = not mentioned in this pack (do not override the base value).
    pub allow_unknown: Option<bool>,
    #[serde(default)]
    pub variants: BTreeMap<String, String>,
}

/// Discriminator metadata for polymorphic object-list items - catalog output type.
///
/// When a `listOfLi` field uses `items.kind = object`, the discriminator
/// selects which object type schema to use for each `<li>` based on an
/// attribute value (e.g. `Class="CompProperties_Glower"`).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectTypeDiscriminator {
    /// XML attribute on each `<li>` that selects the variant (e.g. `"Class"`).
    pub attribute: String,
    /// Object type to use when the attribute is absent and `allowMissing = true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_schema_ref: Option<String>,
    /// Whether a missing discriminator attribute is non-blocking.
    pub allow_missing: bool,
    /// Whether an unknown attribute value is non-blocking.
    pub allow_unknown: bool,
    /// Maps discriminator attribute values (e.g. `"CompProperties_Glower"`) to
    /// object type names.
    pub variants: BTreeMap<String, String>,
}

impl ObjectTypeDiscriminator {
    pub fn from_def(def: &ObjectTypeDiscriminatorDef) -> Self {
        Self {
            attribute: def.attribute.clone(),
            fallback_schema_ref: def.fallback_schema_ref.clone(),
            allow_missing: def.allow_missing.unwrap_or(false),
            allow_unknown: def.allow_unknown.unwrap_or(false),
            variants: def.variants.clone(),
        }
    }
}

/// Object type definition as it appears in a pack JSON file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectTypeSchemaDef {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub inherits: Vec<String>,
    #[serde(default)]
    pub field_order: Vec<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSchemaDef>,
    pub discriminator: Option<ObjectTypeDiscriminatorDef>,
}

/// A single object-type schema file. `object_type` is the type name; the rest
/// flattens into `ObjectTypeSchemaDef` so the JSON shape stays flat.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectTypeSchemaFile {
    pub object_type: String,
    #[serde(flatten)]
    pub schema: ObjectTypeSchemaDef,
}

/// Def type definition as it appears in a pack JSON file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefTypeSchemaDef {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub inherits: Vec<String>,
    #[serde(default)]
    pub abstract_type: bool,
    #[serde(default)]
    pub field_order: Vec<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSchemaDef>,
    #[serde(default)]
    pub templates: BTreeMap<String, DefTemplateDef>,
    #[serde(default)]
    pub validation_rules: BTreeMap<String, ValidationRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaPackManifest {
    pub format_version: u16,
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub game_version: Option<String>,
    pub rimedit_version: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub def_types: BTreeMap<String, DefTypeSchemaDef>,
    #[serde(default)]
    pub object_types: BTreeMap<String, ObjectTypeSchemaDef>,
    /// Keyed by `className`. Empty for packs that only declare Def/object schemas.
    #[serde(default)]
    pub patch_operations: BTreeMap<String, PatchOperationMetadataDef>,
}

/// Manifest file on disk. Does not embed def types; instead lists directories
/// where def type files can be found.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaPackManifestFile {
    pub format_version: u16,
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub game_version: Option<String>,
    pub rimedit_version: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub dependencies: Vec<String>,
    // Default to empty so a missing field produces schema_pack_def_type_directory_missing
    // rather than a generic deserialization error.
    #[serde(default)]
    pub def_type_directories: Vec<String>,
    #[serde(default)]
    pub object_type_directories: Vec<String>,
    /// Directories containing patch operation metadata JSON files (issue 03). Optional -- a pack
    /// with no custom/built-in patch operation metadata omits this entirely.
    #[serde(default)]
    pub patch_operation_directories: Vec<String>,
}

/// A single def-type schema file. `def_type` is the def type name; the rest
/// flattens into `DefTypeSchemaDef` so the JSON shape stays flat.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefTypeSchemaFile {
    pub def_type: String,
    #[serde(flatten)]
    pub schema: DefTypeSchemaDef,
}

/// Declarative preview support for a patch operation as it appears in a metadata JSON file.
/// `kind` is parsed loosely (any unrecognized string is normalized to `unsupported` with a
/// warning) since only `unsupported` is meaningful today; declarative preview behaviors are a
/// documented future extension (see `docs/patches-editor/Plan.md`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationPreviewDef {
    pub kind: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PatchOperationPreviewKind {
    Unsupported,
}

/// Resolved preview status in the merged catalog. Absent `preview` in every contributing pack
/// resolves to `Unsupported` with no message, since RimEdit cannot safely assume a custom
/// operation is previewable without an explicit declaration.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationPreview {
    pub kind: PatchOperationPreviewKind,
    pub message: Option<String>,
}

/// Patch operation metadata content as it appears in a metadata JSON file, without the
/// `className`/`formatVersion` envelope (see `PatchOperationMetadataFile`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationMetadataDef {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub field_order: Vec<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSchemaDef>,
    pub preview: Option<PatchOperationPreviewDef>,
}

/// A single patch operation metadata file. `class_name` is the operation's `Class` value; the
/// rest flattens into `PatchOperationMetadataDef` so the JSON shape stays flat. `formatVersion`
/// is checked separately before this type is deserialized (see `loader::parse_patch_operation_metadata`)
/// so an unsupported version can be reported without fighting serde over an unknown shape.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationMetadataFile {
    pub class_name: String,
    #[serde(flatten)]
    pub schema: PatchOperationMetadataDef,
}

// ---------------------------------------------------------------------------
// Catalog output types (Serialize only)
// Returned by the Tauri command - all scalar fields are concrete with defaults
// already resolved by the merge layer.
// ---------------------------------------------------------------------------

/// Resolved field schema in the merged catalog.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSchema {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub examples: Vec<String>,
    pub validation_hints: Option<ValidationHints>,
    pub reference: Option<ReferenceMetadata>,
    /// For `namedChildrenMap` and `keyedObjectList` fields: the def type referenced by the map/list keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_reference: Option<ReferenceMetadata>,
    /// For `typedReferenceList` fields: metadata for typed reference child lists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed_reference: Option<TypedReferenceMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value_field: Option<String>,
    /// For `namedChildrenMap` fields: expected scalar type of each child element's text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<FieldType>,
    pub repeatable: bool,
    pub xml: XmlFieldShape,
    pub source_pack_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<FieldType>,
    pub flags: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_collapsed: Option<bool>,
    /// Alternative XML element names that load into this field (source `[LoadAlias]`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub xml_aliases: Vec<String>,
    /// Only meaningful on `PatchOperationMetadata` fields; always `None` on Def/object fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<PatchOperationFieldRole>,
}

/// A resolved def-creation template in the merged catalog.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefTemplate {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub include_required_fields: bool,
    pub prompt_fields: Vec<String>,
    pub field_values: BTreeMap<String, TemplateFieldValue>,
    pub source_pack_id: Option<String>,
}

/// Resolved object type schema in the merged catalog.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectTypeSchema {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub inherits: Vec<String>,
    pub field_order: Vec<String>,
    pub fields: BTreeMap<String, FieldSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<ObjectTypeDiscriminator>,
}

/// Resolved def type schema in the merged catalog.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefTypeSchema {
    pub label: Option<String>,
    pub description: Option<String>,
    pub inherits: Vec<String>,
    pub abstract_type: bool,
    pub field_order: Vec<String>,
    pub fields: BTreeMap<String, FieldSchema>,
    pub templates: BTreeMap<String, DefTemplate>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub validation_rules: BTreeMap<String, ValidationRule>,
}

/// Resolved patch operation metadata in the merged catalog, keyed by `className` on
/// `SchemaCatalog::patch_operations`. Covers both built-in operations (shipped as metadata so
/// they render through the same form path as custom operations) and user/mod-defined custom
/// operations.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationMetadata {
    pub class_name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub field_order: Vec<String>,
    pub fields: BTreeMap<String, FieldSchema>,
    pub preview: PatchOperationPreview,
    pub source_pack_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaPackSourceKind {
    BuiltIn,
    External,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedSchemaPackSummary {
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub format_version: u16,
    pub game_version: Option<String>,
    pub rimedit_version: Option<String>,
    pub author: Option<String>,
    pub dependencies: Vec<String>,
    pub priority: i32,
    pub source_kind: SchemaPackSourceKind,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCatalog {
    pub format_version: u16,
    pub packs: Vec<LoadedSchemaPackSummary>,
    pub def_types: BTreeMap<String, DefTypeSchema>,
    pub object_types: BTreeMap<String, ObjectTypeSchema>,
    /// Keyed by `className`. Includes both built-in and custom/mod-defined patch operations.
    pub patch_operations: BTreeMap<String, PatchOperationMetadata>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCatalogLoadResult {
    pub catalog: SchemaCatalog,
    pub diagnostics: Vec<SchemaLoadDiagnostic>,
}
