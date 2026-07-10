use super::loader::LoadedPack;
use super::model::{
    DefTemplate, DefTypeSchema, FieldSchema, FieldSchemaDef, FieldTypeKind,
    LoadedSchemaPackSummary, ObjectTypeSchema, PatchOperationMetadata, PatchOperationMetadataDef,
    PatchOperationPreview, PatchOperationPreviewDef, PatchOperationPreviewKind, SchemaCatalog,
    SchemaLoadDiagnostic, SchemaPackSourceKind, ValidationRule, XmlFieldShape,
};
use std::collections::BTreeMap;
use std::collections::HashSet;

/// Merge a set of loaded packs into a single catalog.
///
/// Precedence (applied in merge order, so last writer wins for scalars):
///   1. Lower `priority` merges first.
///   2. Higher `priority` overrides lower.
///   3. Ties: built-in packs before external.
///   4. Ties: `packId` ascending.
///
/// Duplicate `packId` values are deduplicated: the first occurrence (lowest precedence)
/// is kept and later duplicates are skipped with a warning diagnostic.
pub fn merge_packs(packs: Vec<LoadedPack>, diags: &mut Vec<SchemaLoadDiagnostic>) -> SchemaCatalog {
    // Sort so that the lowest-precedence pack merges first (last writer wins).
    // builtins (0) sort before externals (1) at the same priority.
    let mut sorted = packs;
    sorted.sort_by(|a, b| {
        a.manifest
            .priority
            .cmp(&b.manifest.priority)
            .then_with(|| {
                let ea: u8 = if a.is_builtin { 0 } else { 1 };
                let eb: u8 = if b.is_builtin { 0 } else { 1 };
                ea.cmp(&eb)
            })
            .then_with(|| a.manifest.pack_id.cmp(&b.manifest.pack_id))
    });

    // Deduplicate pack IDs; emit a warning for each duplicate.
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut deduped: Vec<LoadedPack> = Vec::new();
    for pack in sorted {
        if seen_ids.contains(&pack.manifest.pack_id) {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_duplicate_pack_id",
                    format!(
                        "Duplicate packId '{}' - later occurrence ignored.",
                        pack.manifest.pack_id
                    ),
                )
                .with_pack_id(&pack.manifest.pack_id),
            );
        } else {
            seen_ids.insert(pack.manifest.pack_id.clone());
            deduped.push(pack);
        }
    }

    let mut merged_def_types: BTreeMap<String, DefTypeSchema> = BTreeMap::new();
    let mut merged_object_types: BTreeMap<String, ObjectTypeSchema> = BTreeMap::new();
    let mut merged_patch_operations: BTreeMap<String, PatchOperationMetadata> = BTreeMap::new();
    let mut summaries: Vec<LoadedSchemaPackSummary> = Vec::new();

    for pack in &deduped {
        let source_kind = if pack.is_builtin {
            SchemaPackSourceKind::BuiltIn
        } else {
            SchemaPackSourceKind::External
        };
        summaries.push(LoadedSchemaPackSummary {
            pack_id: pack.manifest.pack_id.clone(),
            name: pack.manifest.name.clone(),
            version: pack.manifest.version.clone(),
            format_version: pack.manifest.format_version,
            game_version: pack.manifest.game_version.clone(),
            rimedit_version: pack.manifest.rimedit_version.clone(),
            author: pack.manifest.author.clone(),
            dependencies: pack.manifest.dependencies.clone(),
            priority: pack.manifest.priority,
            source_kind,
            path: pack.source_path.clone(),
        });

        for (def_name, incoming_def) in &pack.manifest.def_types {
            let entry = merged_def_types
                .entry(def_name.clone())
                .or_insert_with(|| DefTypeSchema {
                    label: None,
                    description: None,
                    inherits: Vec::new(),
                    abstract_type: false,
                    field_order: Vec::new(),
                    fields: BTreeMap::new(),
                    templates: BTreeMap::new(),
                    validation_rules: BTreeMap::new(),
                });

            // Scalar fields: later pack wins when explicitly provided.
            if incoming_def.label.is_some() {
                entry.label = incoming_def.label.clone();
            }
            if incoming_def.description.is_some() {
                entry.description = incoming_def.description.clone();
            }
            if incoming_def.abstract_type {
                entry.abstract_type = true;
            }

            // inherits: append-dedup preserving order.
            for parent in &incoming_def.inherits {
                if !entry.inherits.contains(parent) {
                    entry.inherits.push(parent.clone());
                }
            }

            // Fields: first occurrence builds from FieldSchemaDef with defaults applied;
            // subsequent packs apply only the scalars they explicitly mention.
            let pre_existing_keys: HashSet<String> = entry.fields.keys().cloned().collect();
            for (field_name, incoming_field) in &incoming_def.fields {
                if entry.fields.contains_key(field_name) {
                    let merged_field = entry.fields.get_mut(field_name).unwrap();
                    apply_field_override(merged_field, incoming_field, &pack.manifest.pack_id);
                } else {
                    entry.fields.insert(
                        field_name.clone(),
                        field_def_to_schema(incoming_field, &pack.manifest.pack_id),
                    );
                }
            }
            let new_field_names: Vec<String> = incoming_def
                .fields
                .keys()
                .filter(|k| !pre_existing_keys.contains(*k))
                .cloned()
                .collect();
            merge_field_order(
                &mut entry.field_order,
                &incoming_def.field_order,
                &new_field_names,
            );

            // Templates: later pack with same id replaces earlier one.
            for (template_id, tpl) in &incoming_def.templates {
                entry.templates.insert(
                    template_id.clone(),
                    DefTemplate {
                        id: template_id.clone(),
                        label: tpl.label.clone(),
                        description: tpl.description.clone(),
                        include_required_fields: tpl.include_required_fields,
                        prompt_fields: tpl.prompt_fields.clone(),
                        field_values: tpl.field_values.clone(),
                        source_pack_id: Some(pack.manifest.pack_id.clone()),
                    },
                );
            }

            // Validation rules: later pack with same id replaces earlier one.
            for (rule_id, rule) in &incoming_def.validation_rules {
                entry.validation_rules.insert(rule_id.clone(), rule.clone());
            }
        }

        for (obj_name, incoming_obj) in &pack.manifest.object_types {
            if let Some(entry) = merged_object_types.get_mut(obj_name) {
                apply_object_type_override(entry, incoming_obj, &pack.manifest.pack_id);
            } else {
                merged_object_types.insert(
                    obj_name.clone(),
                    object_def_to_schema(incoming_obj, &pack.manifest.pack_id),
                );
            }
        }

        for (class_name, incoming_op) in &pack.manifest.patch_operations {
            apply_patch_operation_override(
                merged_patch_operations
                    .entry(class_name.clone())
                    .or_insert_with(|| PatchOperationMetadata {
                        class_name: class_name.clone(),
                        label: None,
                        description: None,
                        field_order: Vec::new(),
                        fields: BTreeMap::new(),
                        preview: PatchOperationPreview {
                            kind: PatchOperationPreviewKind::Unsupported,
                            message: None,
                        },
                        source_pack_id: None,
                    }),
                incoming_op,
                &pack.manifest.pack_id,
                diags,
            );
        }
    }

    // Post-merge: warn for patch operation fieldOrder entries that don't resolve to a known field.
    for (class_name, op_schema) in &merged_patch_operations {
        for field_name in &op_schema.field_order {
            if !op_schema.fields.contains_key(field_name) {
                diags.push(SchemaLoadDiagnostic::warning(
                    "schema_pack_patch_operation_field_order_unknown",
                    format!(
                        "fieldOrder entry '{}' on patch operation '{}' does not resolve to a known field.",
                        field_name, class_name
                    ),
                ));
            }
        }
    }

    // Post-merge: warn for fieldOrder entries that don't resolve to a known field.
    for (def_name, def_schema) in &merged_def_types {
        let all_fields = collect_all_inherited_fields(def_name, &merged_def_types);
        for field_name in &def_schema.field_order {
            if !all_fields.contains(field_name) {
                diags.push(SchemaLoadDiagnostic::warning(
                    "schema_pack_field_order_unknown",
                    format!(
                        "fieldOrder entry '{}' on '{}' does not resolve to a known field.",
                        field_name, def_name
                    ),
                ));
            }
        }
    }

    // Post-merge: warn when validation rule fields don't resolve on the def type.
    for (def_name, def_schema) in &merged_def_types {
        let all_fields = collect_all_inherited_fields(def_name, &merged_def_types);
        for (rule_id, rule) in &def_schema.validation_rules {
            match rule {
                ValidationRule::RequiredWhen { field, when, .. } => {
                    if !all_fields.contains(field.as_str()) {
                        diags.push(SchemaLoadDiagnostic::warning(
                            "schema_pack_validation_rule_unknown_field",
                            format!(
                                "Validation rule '{rule_id}' on '{def_name}' references unknown target field '{field}'."
                            ),
                        ));
                    }
                    if !all_fields.contains(when.field.as_str()) {
                        diags.push(SchemaLoadDiagnostic::warning(
                            "schema_pack_validation_rule_unknown_condition_field",
                            format!(
                                "Validation rule '{rule_id}' on '{def_name}' references unknown condition field '{}'.",
                                when.field
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Post-merge: warn for object type fieldOrder entries referencing unknown fields (incl. inherited).
    for (obj_name, obj_schema) in &merged_object_types {
        let all_obj_fields = collect_all_object_inherited_fields(obj_name, &merged_object_types);
        for field_name in &obj_schema.field_order {
            if !all_obj_fields.contains(field_name) {
                diags.push(SchemaLoadDiagnostic::warning(
                    "schema_pack_object_field_order_unknown",
                    format!(
                        "fieldOrder entry '{}' on object type '{}' does not resolve to a known field.",
                        field_name, obj_name
                    ),
                ));
            }
        }
    }

    // Post-merge: warn when discriminator variant targets don't resolve to known object types.
    for (obj_name, obj_schema) in &merged_object_types {
        if let Some(disc) = &obj_schema.discriminator {
            for (class_name, target_ref) in &disc.variants {
                if !merged_object_types.contains_key(target_ref) {
                    diags.push(SchemaLoadDiagnostic::warning(
                        "schema_pack_unknown_discriminator_variant_target",
                        format!(
                            "Discriminator variant '{}' on '{}' maps to '{}' which is not a known object type.",
                            class_name, obj_name, target_ref
                        ),
                    ));
                }
            }
        }
    }

    // Post-merge: validate schemaRef fields reference known object types.
    for (def_name, def_schema) in &merged_def_types {
        for (field_name, field) in &def_schema.fields {
            if field.field_type.kind == FieldTypeKind::Object {
                if let Some(schema_ref) = &field.field_type.schema_ref {
                    if !schema_ref.is_empty() && !merged_object_types.contains_key(schema_ref) {
                        diags.push(SchemaLoadDiagnostic::warning(
                            "schema_pack_unknown_object_schema_ref",
                            format!(
                                "schemaRef '{}' on '{}.fields.{}' does not resolve to a known object type.",
                                schema_ref, def_name, field_name
                            ),
                        ).with_field_path(format!("{}.fields.{}.type.schemaRef", def_name, field_name)));
                    }
                }
            }
            // Warn when items.schemaRef references a missing object type.
            if let Some(items) = &field.items {
                if items.kind == FieldTypeKind::Object {
                    if let Some(schema_ref) = &items.schema_ref {
                        if !schema_ref.is_empty() && !merged_object_types.contains_key(schema_ref) {
                            diags.push(SchemaLoadDiagnostic::warning(
                                "schema_pack_unknown_list_item_schema_ref",
                                format!(
                                    "items.schemaRef '{}' on '{}.fields.{}' does not resolve to a known object type.",
                                    schema_ref, def_name, field_name
                                ),
                            ).with_field_path(format!("{}.fields.{}.items.schemaRef", def_name, field_name)));
                        }
                    }
                }
            }
        }
    }
    for (obj_name, obj_schema) in &merged_object_types {
        for (field_name, field) in &obj_schema.fields {
            if field.field_type.kind == FieldTypeKind::Object {
                if let Some(schema_ref) = &field.field_type.schema_ref {
                    if !schema_ref.is_empty() && !merged_object_types.contains_key(schema_ref) {
                        diags.push(SchemaLoadDiagnostic::warning(
                            "schema_pack_unknown_object_schema_ref",
                            format!(
                                "schemaRef '{}' on '{}.fields.{}' does not resolve to a known object type.",
                                schema_ref, obj_name, field_name
                            ),
                        ).with_field_path(format!("{}.fields.{}.type.schemaRef", obj_name, field_name)));
                    }
                }
            }
            // Warn when items.schemaRef references a missing object type.
            if let Some(items) = &field.items {
                if items.kind == FieldTypeKind::Object {
                    if let Some(schema_ref) = &items.schema_ref {
                        if !schema_ref.is_empty() && !merged_object_types.contains_key(schema_ref) {
                            diags.push(SchemaLoadDiagnostic::warning(
                                "schema_pack_unknown_list_item_schema_ref",
                                format!(
                                    "items.schemaRef '{}' on '{}.fields.{}' does not resolve to a known object type.",
                                    schema_ref, obj_name, field_name
                                ),
                            ).with_field_path(format!("{}.fields.{}.items.schemaRef", obj_name, field_name)));
                        }
                    }
                }
            }
        }
    }

    SchemaCatalog {
        format_version: 1,
        packs: summaries,
        def_types: merged_def_types,
        object_types: merged_object_types,
        patch_operations: merged_patch_operations,
    }
}

/// Convert a parsed `FieldSchemaDef` into a resolved `FieldSchema`, applying
/// canonical defaults for any scalars the pack left unspecified.
fn field_def_to_schema(def: &FieldSchemaDef, pack_id: &str) -> FieldSchema {
    FieldSchema {
        label: def.label.clone(),
        description: def.description.clone(),
        field_type: def.field_type.clone(),
        required: def.required.unwrap_or(false),
        default_value: def.default_value.clone(),
        examples: def.examples.clone(),
        validation_hints: def.validation_hints.clone(),
        reference: def.reference.clone(),
        key_reference: def.key_reference.clone(),
        typed_reference: def.typed_reference.clone(),
        key_field: def.key_field.clone(),
        value_field: def.value_field.clone(),
        default_value_field: def.default_value_field.clone(),
        value_type: def.value_type.clone(),
        repeatable: def.repeatable.unwrap_or(false),
        xml: def.xml.clone().unwrap_or(XmlFieldShape::Element),
        source_pack_id: Some(pack_id.to_string()),
        items: def.items.clone(),
        flags: def.flags.unwrap_or(false),
        default_collapsed: def.default_collapsed,
        xml_aliases: def.xml_aliases.clone(),
        role: def.role.clone(),
    }
}

/// Apply an incoming `FieldSchemaDef` as an override onto an existing `FieldSchema`.
///
/// Only scalars that the incoming pack *explicitly* set (i.e. are `Some`) are
/// written. Absent fields (`None`) leave the base value unchanged, preventing
/// an override pack that only adds examples from accidentally clearing
/// `required`, resetting `xml`, or removing `defaultValue`.
fn apply_field_override(base: &mut FieldSchema, incoming: &FieldSchemaDef, pack_id: &str) {
    if incoming.label.is_some() {
        base.label = incoming.label.clone();
    }
    if incoming.description.is_some() {
        base.description = incoming.description.clone();
    }

    // field_type is always replaced; a field entry without a type is invalid JSON.
    base.field_type = incoming.field_type.clone();

    if let Some(req) = incoming.required {
        base.required = req;
    }
    if incoming.default_value.is_some() {
        base.default_value = incoming.default_value.clone();
    }
    if let Some(rep) = incoming.repeatable {
        base.repeatable = rep;
    }
    if let Some(xml) = &incoming.xml {
        base.xml = xml.clone();
    }
    if incoming.validation_hints.is_some() {
        base.validation_hints = incoming.validation_hints.clone();
    }
    if incoming.reference.is_some() {
        base.reference = incoming.reference.clone();
    }
    if incoming.key_reference.is_some() {
        base.key_reference = incoming.key_reference.clone();
    }
    if incoming.typed_reference.is_some() {
        base.typed_reference = incoming.typed_reference.clone();
    }
    if incoming.key_field.is_some() {
        base.key_field = incoming.key_field.clone();
    }
    if incoming.value_field.is_some() {
        base.value_field = incoming.value_field.clone();
    }
    if incoming.default_value_field.is_some() {
        base.default_value_field = incoming.default_value_field.clone();
    }
    if incoming.value_type.is_some() {
        base.value_type = incoming.value_type.clone();
    }

    for ex in &incoming.examples {
        if !base.examples.contains(ex) {
            base.examples.push(ex.clone());
        }
    }

    if incoming.items.is_some() {
        base.items = incoming.items.clone();
    }
    if let Some(flags) = incoming.flags {
        base.flags = flags;
    }
    if let Some(dc) = incoming.default_collapsed {
        base.default_collapsed = Some(dc);
    }
    for alias in &incoming.xml_aliases {
        if !base.xml_aliases.contains(alias) {
            base.xml_aliases.push(alias.clone());
        }
    }
    if incoming.role.is_some() {
        base.role = incoming.role.clone();
    }

    base.source_pack_id = Some(pack_id.to_string());
}

/// Apply an incoming `PatchOperationMetadataDef` as an override onto an existing
/// `PatchOperationMetadata`, following the same "only explicit `Some` values override" rule as
/// `apply_field_override`. `preview` is only replaced when the incoming pack explicitly declares
/// one, so a lower-priority pack's declared preview support isn't silently cleared by a
/// higher-priority pack that only adds a label or an extra field.
fn apply_patch_operation_override(
    base: &mut PatchOperationMetadata,
    incoming: &PatchOperationMetadataDef,
    pack_id: &str,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) {
    if incoming.label.is_some() {
        base.label = incoming.label.clone();
    }
    if incoming.description.is_some() {
        base.description = incoming.description.clone();
    }

    let pre_existing_keys: HashSet<String> = base.fields.keys().cloned().collect();
    for (field_name, incoming_field) in &incoming.fields {
        if base.fields.contains_key(field_name) {
            let merged_field = base.fields.get_mut(field_name).unwrap();
            apply_field_override(merged_field, incoming_field, pack_id);
        } else {
            base.fields.insert(
                field_name.clone(),
                field_def_to_schema(incoming_field, pack_id),
            );
        }
    }
    let new_field_names: Vec<String> = incoming
        .fields
        .keys()
        .filter(|k| !pre_existing_keys.contains(*k))
        .cloned()
        .collect();
    merge_field_order(
        &mut base.field_order,
        &incoming.field_order,
        &new_field_names,
    );

    if let Some(preview_def) = &incoming.preview {
        base.preview =
            resolve_patch_operation_preview(preview_def, &base.class_name, pack_id, diags);
    }

    base.source_pack_id = Some(pack_id.to_string());
}

/// Resolve a patch operation's declared `preview.kind` string into `PatchOperationPreviewKind`.
/// Only `"unsupported"` is meaningful today (declarative preview behaviors are a documented
/// future extension); any other value is normalized to `Unsupported` with a warning rather than
/// failing the whole metadata file, matching how an unrecognized `FieldTypeKind` is handled.
fn resolve_patch_operation_preview(
    def: &PatchOperationPreviewDef,
    class_name: &str,
    pack_id: &str,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> PatchOperationPreview {
    let kind = match def.kind.as_deref() {
        Some("unsupported") | None => PatchOperationPreviewKind::Unsupported,
        Some(other) => {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "patch_operation_metadata_unknown_preview_kind",
                    format!(
                        "Unknown preview kind '{}' for patch operation '{}'; treated as unsupported.",
                        other, class_name
                    ),
                )
                .with_pack_id(pack_id),
            );
            PatchOperationPreviewKind::Unsupported
        }
    };
    PatchOperationPreview {
        kind,
        message: def.message.clone(),
    }
}

fn object_def_to_schema(
    def: &super::model::ObjectTypeSchemaDef,
    pack_id: &str,
) -> ObjectTypeSchema {
    let fields: BTreeMap<String, FieldSchema> = def
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), field_def_to_schema(v, pack_id)))
        .collect();
    // Append any fields omitted from fieldOrder as fallback, matching def type behavior.
    let all_field_names: Vec<String> = fields.keys().cloned().collect();
    let mut field_order = def.field_order.clone();
    merge_field_order(&mut field_order, &[], &all_field_names);
    ObjectTypeSchema {
        label: def.label.clone(),
        description: def.description.clone(),
        inherits: def.inherits.clone(),
        field_order,
        fields,
        discriminator: def
            .discriminator
            .as_ref()
            .map(super::model::ObjectTypeDiscriminator::from_def),
    }
}

fn apply_object_type_override(
    base: &mut ObjectTypeSchema,
    incoming: &super::model::ObjectTypeSchemaDef,
    pack_id: &str,
) {
    if incoming.label.is_some() {
        base.label = incoming.label.clone();
    }
    if incoming.description.is_some() {
        base.description = incoming.description.clone();
    }

    // inherits: append-dedup preserving order.
    for parent in &incoming.inherits {
        if !base.inherits.contains(parent) {
            base.inherits.push(parent.clone());
        }
    }

    let pre_existing_keys: HashSet<String> = base.fields.keys().cloned().collect();
    for (field_name, incoming_field) in &incoming.fields {
        if base.fields.contains_key(field_name) {
            let merged_field = base.fields.get_mut(field_name).unwrap();
            apply_field_override(merged_field, incoming_field, pack_id);
        } else {
            base.fields.insert(
                field_name.clone(),
                field_def_to_schema(incoming_field, pack_id),
            );
        }
    }
    let new_field_names: Vec<String> = incoming
        .fields
        .keys()
        .filter(|k| !pre_existing_keys.contains(*k))
        .cloned()
        .collect();
    merge_field_order(
        &mut base.field_order,
        &incoming.field_order,
        &new_field_names,
    );

    // Discriminator: merge variants; only override booleans when explicitly set.
    if let Some(incoming_disc) = &incoming.discriminator {
        if let Some(base_disc) = &mut base.discriminator {
            // Merge variants: later pack adds or overrides.
            for (k, v) in &incoming_disc.variants {
                base_disc.variants.insert(k.clone(), v.clone());
            }
            // Always override the attribute name.
            base_disc.attribute = incoming_disc.attribute.clone();
            // Override optional fields only when the pack explicitly set them.
            if incoming_disc.fallback_schema_ref.is_some() {
                base_disc.fallback_schema_ref = incoming_disc.fallback_schema_ref.clone();
            }
            if let Some(v) = incoming_disc.allow_missing {
                base_disc.allow_missing = v;
            }
            if let Some(v) = incoming_disc.allow_unknown {
                base_disc.allow_unknown = v;
            }
        } else {
            base.discriminator = Some(super::model::ObjectTypeDiscriminator::from_def(
                incoming_disc,
            ));
        }
    }
}

/// Collect all field names accessible on an object type, including inherited fields.
fn collect_all_object_inherited_fields(
    obj_name: &str,
    object_types: &BTreeMap<String, ObjectTypeSchema>,
) -> HashSet<String> {
    let mut fields = HashSet::new();
    let mut visited = HashSet::new();
    collect_object_fields_recursive(obj_name, object_types, &mut fields, &mut visited);
    fields
}

fn collect_object_fields_recursive(
    obj_name: &str,
    object_types: &BTreeMap<String, ObjectTypeSchema>,
    fields: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(obj_name.to_string()) {
        return;
    }
    if let Some(schema) = object_types.get(obj_name) {
        for parent in &schema.inherits {
            collect_object_fields_recursive(parent, object_types, fields, visited);
        }
        for (field_name, field_schema) in &schema.fields {
            fields.insert(field_name.clone());
            for alias in &field_schema.xml_aliases {
                fields.insert(alias.clone());
            }
        }
    }
}

/// Collect all field names accessible to a def type, including fields from all
/// ancestors, walking `inherits` recursively.
fn collect_all_inherited_fields(
    def_name: &str,
    def_types: &BTreeMap<String, DefTypeSchema>,
) -> HashSet<String> {
    let mut fields = HashSet::new();
    let mut visited = HashSet::new();
    collect_fields_recursive(def_name, def_types, &mut fields, &mut visited);
    fields
}

fn collect_fields_recursive(
    def_name: &str,
    def_types: &BTreeMap<String, DefTypeSchema>,
    fields: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(def_name.to_string()) {
        return;
    }
    if let Some(schema) = def_types.get(def_name) {
        for parent in &schema.inherits {
            collect_fields_recursive(parent, def_types, fields, visited);
        }
        for field_name in schema.fields.keys() {
            fields.insert(field_name.clone());
        }
    }
}

/// Merge incoming `field_order` into the current accumulated order using append-dedup.
///
/// If `incoming` is non-empty, names already in `current` that appear in `incoming`
/// are removed and then re-appended in `incoming` order, allowing override packs to
/// reposition fields. Any newly introduced fields not yet in `current` are appended
/// as a fallback so every known field has a stable position.
fn merge_field_order(current: &mut Vec<String>, incoming: &[String], new_fields: &[String]) {
    if !incoming.is_empty() {
        current.retain(|n| !incoming.contains(n));
        for name in incoming {
            if !current.contains(name) {
                current.push(name.clone());
            }
        }
    }
    for name in new_fields {
        if !current.contains(name) {
            current.push(name.clone());
        }
    }
}
