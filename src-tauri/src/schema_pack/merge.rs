use super::loader::LoadedPack;
use super::locale::{apply_locale_overlays, LocaleOwnerMaps};
use super::lookup::collect_effective_top_level_def_fields_from_map;
use super::model::{
    DefTemplate, DefTypeSchema, FieldSchema, FieldSchemaDef, FieldTypeKind, FormViewDef,
    FormViewSource, LoadedSchemaPackSummary, ObjectTypeSchema, PatchOperationMetadata,
    PatchOperationMetadataDef, PatchOperationPreview, PatchOperationPreviewDef,
    PatchOperationPreviewKind, SchemaCatalog, SchemaFormView, SchemaLoadDiagnostic,
    SchemaPackSourceKind, ValidationRule, XmlFieldShape,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

/// One pack's `FormViewDef` declarations for a single def type -- either the raw, as-authored form
/// (`raw_form_view_layers`) or the sanitized form after unknown-field-reference filtering
/// (`sanitize_form_view_layers`'s output) -- tagged with that pack's id, version, and the source
/// file path of the Def-type JSON that declared it (`(pack_id, pack_version, path, decls)`).
/// Plan.md section 5 requires recoverable diagnostics to expose pack, path, and field path so an
/// author of an external pack can locate the offending file.
type PackFormViewDecls = (String, String, String, BTreeMap<String, FormViewDef>);

/// Test-only convenience wrapper around [`merge_packs_with_locale`] that always applies locale
/// overlays for `crate::locale::FALLBACK_LOCALE`. Every real production call site now threads an
/// explicit, caller-resolved locale through `merge_packs_with_locale` directly (issue 06 --
/// see `schema_pack::build_schema_catalog_with_locale`); this wrapper exists only so the large
/// pre-existing test surface (`schema_pack/tests/*`), which never varies locale and predates
/// issue 06, doesn't need every one of its several dozen call sites touched to pass one
/// explicitly. `#[cfg(test)]` because nothing outside `schema_pack/tests` calls it.
///
/// Precedence (applied in merge order, so last writer wins for scalars):
///   1. Lower `priority` merges first.
///   2. Higher `priority` overrides lower.
///   3. Ties: built-in packs before external.
///   4. Ties: `packId` ascending.
///
/// Duplicate `packId` values are deduplicated: the first occurrence (lowest precedence)
/// is kept and later duplicates are skipped with a warning diagnostic.
#[cfg(test)]
pub fn merge_packs(packs: Vec<LoadedPack>, diags: &mut Vec<SchemaLoadDiagnostic>) -> SchemaCatalog {
    merge_packs_with_locale(packs, diags, crate::locale::FALLBACK_LOCALE)
}

/// Same as [`merge_packs`], but applies locale overlays for the given `locale` instead of the
/// fixed fallback. `locale` is expected to already be resolved/validated by the caller (see
/// `schema_pack::build_schema_catalog_with_locale`, which validates against the application
/// locale registry before calling this).
pub fn merge_packs_with_locale(
    packs: Vec<LoadedPack>,
    diags: &mut Vec<SchemaLoadDiagnostic>,
    locale: &str,
) -> SchemaCatalog {
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
    // Per-scalar "pack that explicitly last set this label/description/message" provenance for
    // locale-overlay ownership checks (issue 05). See `locale::LocaleOwnerMaps` doc comment for why
    // this per-scalar granularity is required even for record kinds (fields, patch operations,
    // form views) that also carry their own coarser, always-overwritten `source_pack_id`/`source`.
    let mut locale_owners = LocaleOwnerMaps::default();
    // Raw `FormViewDef` declarations per def type, one entry per contributing pack, in the same
    // pack-precedence order as `deduped` (lowest precedence first). Form View resolution
    // (`resolve_all_form_views`) folds these together with each def type's ancestor chain after
    // every pack has been merged -- see the call site below for why this can't happen inline in
    // this loop (a def type's ancestors, and their own form_views declarations, may not have been
    // visited yet, and `SchemaCatalog.defTypes[defType].formViews` must be the fully resolved
    // result, not a partial pack-precedence-only layer).
    let mut raw_form_view_layers: BTreeMap<String, Vec<PackFormViewDecls>> = BTreeMap::new();

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
                    // Issue 01 scope only: resolution of FormViewDef declarations into
                    // SchemaFormView entries is issue 03's job.
                    form_views: BTreeMap::new(),
                });

            // Scalar fields: later pack wins when explicitly provided. Locale-sidecar ownership of
            // `label`/`description` is tracked per scalar, only when this pack actually set it --
            // never for the def type as a whole -- so a pack that only amends some other part of
            // this Def (fields, templates, validation rules, ...) can't thereby become able to
            // sidecar-override a label/description it never supplied (see `locale` module docs).
            if incoming_def.label.is_some() {
                entry.label = incoming_def.label.clone();
                locale_owners
                    .def_type_labels
                    .insert(def_name.clone(), pack.manifest.pack_id.clone());
            }
            if incoming_def.description.is_some() {
                entry.description = incoming_def.description.clone();
                locale_owners
                    .def_type_descriptions
                    .insert(def_name.clone(), pack.manifest.pack_id.clone());
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
                locale_owners.validation_rules.insert(
                    (def_name.clone(), rule_id.clone()),
                    pack.manifest.pack_id.clone(),
                );
            }

            // Form Views: record this pack's raw declarations for this def type; resolution
            // (pack precedence + Def-type inheritance) happens once, after every pack and every
            // def type's `inherits` chain is known (see `resolve_all_form_views` below).
            if !incoming_def.form_views.is_empty() {
                let source_path = pack
                    .manifest
                    .def_type_source_paths
                    .get(def_name)
                    .cloned()
                    .unwrap_or_default();
                raw_form_view_layers
                    .entry(def_name.clone())
                    .or_default()
                    .push((
                        pack.manifest.pack_id.clone(),
                        pack.manifest.version.clone(),
                        source_path,
                        incoming_def.form_views.clone(),
                    ));
            }
        }

        for (obj_name, incoming_obj) in &pack.manifest.object_types {
            // Same per-scalar (not per-record) ownership rule as def types above.
            if incoming_obj.label.is_some() {
                locale_owners
                    .object_type_labels
                    .insert(obj_name.clone(), pack.manifest.pack_id.clone());
            }
            if incoming_obj.description.is_some() {
                locale_owners
                    .object_type_descriptions
                    .insert(obj_name.clone(), pack.manifest.pack_id.clone());
            }
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
                class_name,
                &mut locale_owners,
                diags,
            );
        }
    }

    // Resolve schema-defined Form Views (issue 03): fold pack-precedence overlays and Def-type
    // inheritance for every def type now that `merged_def_types` (fields + `inherits`) is
    // complete. See `resolve_all_form_views` for the algorithm.
    let resolved_form_views = resolve_all_form_views(
        &merged_def_types,
        &raw_form_view_layers,
        &mut locale_owners,
        diags,
    );
    for (def_name, views) in resolved_form_views {
        if let Some(entry) = merged_def_types.get_mut(&def_name) {
            entry.form_views = views;
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

    // Apply pack-owned locale sidecar overlays, strictly after every other merge/inheritance/
    // validation pass above (see `locale` module docs and this issue's "Risks" section on merge
    // provenance). `locale` is the caller-supplied, already-resolved locale (issue 06 threads this
    // explicitly through `merge_packs_with_locale`/`build_schema_catalog_with_locale`/the
    // `load_schema_catalog` Tauri command; `merge_packs` itself keeps defaulting to
    // `crate::locale::FALLBACK_LOCALE` for the large existing test surface and every
    // locale-neutral structural caller -- see issue 06's "Document that indexing, save/validation,
    // patch computation, and diagnostic creation ... do not receive locale").
    let locale_diags = apply_locale_overlays(
        &mut merged_def_types,
        &mut merged_object_types,
        &mut merged_patch_operations,
        &deduped,
        locale,
        &locale_owners,
    );
    diags.extend(locale_diags);

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
        // Only recorded as owner when this pack's own JSON explicitly set the scalar -- see
        // `FieldSchema::label_source_pack_id` doc comment and `apply_field_override` below.
        label_source_pack_id: def.label.is_some().then(|| pack_id.to_string()),
        description_source_pack_id: def.description.is_some().then(|| pack_id.to_string()),
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
    // Label/description ownership is tracked per-scalar, independent of every other property on
    // this field, and only transferred to `pack_id` when its own JSON explicitly sets that
    // specific scalar -- otherwise a pack that amends only some other property (type, examples,
    // xml shape, ...) would wrongly gain sidecar-override rights over a label/description it
    // never supplied. See `FieldSchema::label_source_pack_id` doc comment.
    if incoming.label.is_some() {
        base.label = incoming.label.clone();
        base.label_source_pack_id = Some(pack_id.to_string());
    }
    if incoming.description.is_some() {
        base.description = incoming.description.clone();
        base.description_source_pack_id = Some(pack_id.to_string());
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
///
/// Locale-sidecar ownership of `label`/`description`/`preview.message` is recorded into `owners`
/// (keyed by `class_name`) only when this pack's own JSON explicitly sets that specific scalar --
/// mirroring `apply_field_override` -- so a pack that amends only some other part of this patch
/// operation (a new field, `fieldOrder`, ...) doesn't thereby gain sidecar-override rights over
/// display text it never supplied. `base.source_pack_id` remains the coarser "last pack that
/// touched this record at all" and must not be used for that check (see `LocaleOwnerMaps` doc
/// comment).
#[allow(clippy::too_many_arguments)]
fn apply_patch_operation_override(
    base: &mut PatchOperationMetadata,
    incoming: &PatchOperationMetadataDef,
    pack_id: &str,
    class_name: &str,
    owners: &mut LocaleOwnerMaps,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) {
    if incoming.label.is_some() {
        base.label = incoming.label.clone();
        owners
            .patch_operation_labels
            .insert(class_name.to_string(), pack_id.to_string());
    }
    if incoming.description.is_some() {
        base.description = incoming.description.clone();
        owners
            .patch_operation_descriptions
            .insert(class_name.to_string(), pack_id.to_string());
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
        owners
            .patch_operation_preview_messages
            .insert(class_name.to_string(), pack_id.to_string());
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

// ---------------------------------------------------------------------------
// Form View resolution (issue 03)
//
// Plan.md section 5's resolution rules are implemented in three passes:
//
//   1. `sanitize_form_view_layers` validates every def type's own `hiddenFields`/`unhideFields`
//      references against ITS OWN known top-level field universe (not any consuming descendant's
//      expanded universe -- see its doc comment for why) and strips unknown references, producing
//      a sanitized copy of `raw_form_view_layers`.
//   2. `compute_effective_layers` flattens a def type's full declaration history -- every
//      ancestor's own sanitized declarations (parent-first, `inherits` order), then this def
//      type's own -- into one ordered, deduplicated list. A def type reachable through more than
//      one `inherits` path (diamond inheritance) still contributes its own declarations exactly
//      once, at the position dictated by the first path that reaches it.
//   3. `fold_effective_layers` walks that flattened list and applies each declaration via
//      `apply_form_view_declaration`, in order. For a multi-parent def type this means parent B's
//      full contribution (its own ancestors' declarations plus B's own) is folded in before parent
//      C's, before the child's own declarations -- so a same-view-id delta from two *different*
//      parents (e.g. B hides an extra field, C unhides a field their shared grandparent hid) both
//      survive in the child, rather than one sibling's contribution wholesale-replacing the
//      other's. See `diamond_inheritance_merges_both_parents_deltas_into_the_child` for the exact
//      scenario this guards against.
//
// Because a shared ancestor's declarations can be folded more than once -- once for its own
// top-level resolution, and again inside every descendant's resolution -- `apply_form_view_declaration`'s
// "amendment with no inherited base" diagnostic is deduplicated via a `diagnosed_amendments_without_base`
// set (keyed by declaring def type/pack/view id, shared across the whole `resolve_all_form_views`
// call) so the same underlying declaration never produces more than one diagnostic no matter how
// many descendants pull it in. `sanitize_form_view_layers`'s unknown-field-reference diagnostics
// don't need this treatment: that pass runs exactly once per (def type, pack) pair, with no
// folding/repetition involved.
// ---------------------------------------------------------------------------

/// One flattened entry in a def type's full declaration history (`compute_effective_layers`):
/// like `PackFormViewDecls`, but additionally tagged with the concrete def type that owns/declared
/// it, since a flattened list mixes declarations from every def type in the ancestor chain.
type EffectiveFormViewLayer = (
    String,
    String,
    String,
    String,
    BTreeMap<String, FormViewDef>,
);

/// Resolve `DefTypeSchema.form_views` for every def type in the merged catalog. See the module
/// section comment above for the pipeline (sanitize -> flatten -> fold).
fn resolve_all_form_views(
    def_types: &BTreeMap<String, DefTypeSchema>,
    raw_form_view_layers: &BTreeMap<String, Vec<PackFormViewDecls>>,
    owners: &mut LocaleOwnerMaps,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> BTreeMap<String, BTreeMap<String, SchemaFormView>> {
    let sanitized_layers = sanitize_form_view_layers(def_types, raw_form_view_layers, diags);

    let mut layer_cache: HashMap<String, EffectiveLayerState> = HashMap::new();
    let mut diagnosed_amendments_without_base: HashSet<(String, String, String)> = HashSet::new();
    let mut resolved: BTreeMap<String, BTreeMap<String, SchemaFormView>> = BTreeMap::new();

    for def_type in def_types.keys() {
        let layers =
            compute_effective_layers(def_type, def_types, &sanitized_layers, &mut layer_cache);
        let views = fold_effective_layers(
            def_type,
            &layers,
            &mut diagnosed_amendments_without_base,
            owners,
            diags,
        );
        resolved.insert(def_type.clone(), views);
    }
    resolved
}

/// Validate every def type's own `hiddenFields`/`unhideFields` references against ITS OWN known
/// top-level field universe (`collect_effective_top_level_def_fields_from_map`), stripping unknown
/// entries and emitting `schema_pack_form_view_unknown_field_reference` warnings.
///
/// Deliberately validates against the OWNER def type's own field universe, not any consuming
/// descendant's expanded universe reached via `compute_effective_layers`: a pack author writes a
/// `hiddenFields` reference in the context of the concrete def type the declaration is attached
/// to, so that is the correct scope to validate against. It also keeps the check a pure function
/// of the declaration alone -- the same verdict every time, regardless of how many descendants
/// later fold it in -- which is why this pass produces no duplicate diagnostics without needing
/// the fold-time dedup guard that "amendment with no inherited base" requires below.
fn sanitize_form_view_layers(
    def_types: &BTreeMap<String, DefTypeSchema>,
    raw_form_view_layers: &BTreeMap<String, Vec<PackFormViewDecls>>,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> BTreeMap<String, Vec<PackFormViewDecls>> {
    let mut sanitized: BTreeMap<String, Vec<PackFormViewDecls>> = BTreeMap::new();
    for (def_type, layers) in raw_form_view_layers {
        let known_fields: HashSet<String> =
            collect_effective_top_level_def_fields_from_map(def_type, def_types)
                .into_iter()
                .map(|(name, _)| name)
                .collect();

        let mut sanitized_layers: Vec<PackFormViewDecls> = Vec::with_capacity(layers.len());
        for (pack_id, pack_version, path, decls) in layers {
            let mut sanitized_decls: BTreeMap<String, FormViewDef> = BTreeMap::new();
            for (view_id, view_def) in decls {
                // Defensive only: issue 02 already fatally rejects a "default" declared id at
                // parse time, so this should never occur here -- but never let it silently
                // participate in resolution if it somehow does (e.g. a future loader regression).
                if view_id == "default" {
                    continue;
                }
                let field_path = format!("{def_type}.formViews.{view_id}");
                let mut sanitized_def = view_def.clone();
                if let Some(hidden) = &view_def.hidden_fields {
                    sanitized_def.hidden_fields = Some(filter_known_field_refs(
                        hidden,
                        "hiddenFields",
                        &known_fields,
                        view_id,
                        def_type,
                        pack_id,
                        path,
                        &field_path,
                        diags,
                    ));
                }
                if let Some(unhide) = &view_def.unhide_fields {
                    sanitized_def.unhide_fields = Some(filter_known_field_refs(
                        unhide,
                        "unhideFields",
                        &known_fields,
                        view_id,
                        def_type,
                        pack_id,
                        path,
                        &field_path,
                        diags,
                    ));
                }
                sanitized_decls.insert(view_id.clone(), sanitized_def);
            }
            sanitized_layers.push((
                pack_id.clone(),
                pack_version.clone(),
                path.clone(),
                sanitized_decls,
            ));
        }
        sanitized.insert(def_type.clone(), sanitized_layers);
    }
    sanitized
}

enum EffectiveLayerState {
    InProgress,
    Done(Vec<EffectiveFormViewLayer>),
}

/// Flatten a def type's full Form View declaration history -- every ancestor's own sanitized
/// declarations (parent-first, `inherits` order), then this def type's own -- into one ordered
/// list, deduplicated by `(owner_def_type, pack_id)` so a def type reachable through more than one
/// `inherits` path (diamond inheritance) contributes its own declarations exactly once, at the
/// position dictated by the first path that reaches it.
///
/// A cycle in `inherits` is guarded the same way `collect_fields_recursive` guards field
/// traversal: a def type revisited while still in progress contributes no further layers for that
/// occurrence, silently -- matching the existing field-traversal cycle behavior (no new
/// diagnostic is added here for a cycle that field resolution doesn't already diagnose either).
fn compute_effective_layers(
    def_type: &str,
    def_types: &BTreeMap<String, DefTypeSchema>,
    sanitized_layers: &BTreeMap<String, Vec<PackFormViewDecls>>,
    cache: &mut HashMap<String, EffectiveLayerState>,
) -> Vec<EffectiveFormViewLayer> {
    match cache.get(def_type) {
        Some(EffectiveLayerState::Done(layers)) => return layers.clone(),
        Some(EffectiveLayerState::InProgress) => return Vec::new(),
        None => {}
    }
    cache.insert(def_type.to_string(), EffectiveLayerState::InProgress);

    let mut result: Vec<EffectiveFormViewLayer> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    if let Some(schema) = def_types.get(def_type) {
        for parent in &schema.inherits {
            let parent_layers =
                compute_effective_layers(parent, def_types, sanitized_layers, cache);
            for layer in parent_layers {
                let key = (layer.0.clone(), layer.1.clone());
                if seen.insert(key) {
                    result.push(layer);
                }
            }
        }
    }

    if let Some(own) = sanitized_layers.get(def_type) {
        for (pack_id, pack_version, path, decls) in own {
            let key = (def_type.to_string(), pack_id.clone());
            if seen.insert(key) {
                result.push((
                    def_type.to_string(),
                    pack_id.clone(),
                    pack_version.clone(),
                    path.clone(),
                    decls.clone(),
                ));
            }
        }
    }

    cache.insert(
        def_type.to_string(),
        EffectiveLayerState::Done(result.clone()),
    );
    result
}

/// Apply a flattened, ordered list of Form View declarations (see `compute_effective_layers`) via
/// `apply_form_view_declaration`, building the final resolved `SchemaFormView` map for one
/// concrete def type. `diagnosed_amendments_without_base` deduplicates the "amendment with no
/// inherited base" diagnostic across repeated folds of the same shared-ancestor declaration (see
/// the module section comment above).
fn fold_effective_layers(
    consuming_def_type: &str,
    layers: &[EffectiveFormViewLayer],
    diagnosed_amendments_without_base: &mut HashSet<(String, String, String)>,
    owners: &mut LocaleOwnerMaps,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> BTreeMap<String, SchemaFormView> {
    let mut current: BTreeMap<String, SchemaFormView> = BTreeMap::new();
    for (owner_def_type, pack_id, pack_version, path, decls) in layers {
        for (view_id, view_def) in decls {
            let field_path = format!("{owner_def_type}.formViews.{view_id}");
            let base = current.get(view_id).cloned();
            let result = apply_form_view_declaration(
                owner_def_type,
                consuming_def_type,
                view_id,
                base.as_ref(),
                view_def,
                pack_id,
                pack_version,
                path,
                &field_path,
                diagnosed_amendments_without_base,
                owners,
                diags,
            );
            match result {
                Some(view) => {
                    current.insert(view_id.clone(), view);
                }
                None => {
                    current.remove(view_id);
                }
            }
        }
    }
    current
}

/// Apply one `FormViewDef` amendment/new-declaration onto an optional existing resolved base view
/// for the same `{defType, viewId}`, implementing Plan.md section 5's resolution rules 3-4:
/// `disabled` first, then `replace` (clears the inherited hidden set), then `hiddenFields`
/// additions / `unhideFields` removals on top; any explicitly provided
/// label/description/icon/order/recommended overrides the inherited value, an omitted one
/// inherits. The final declaration's pack id/version and def type become the view's provenance,
/// even when the declaration is a pure delta amendment.
///
/// Field references have already been validated/filtered by `sanitize_form_view_layers` before
/// this function ever runs, so `incoming.hidden_fields`/`incoming.unhide_fields` are assumed to
/// already be a subset of the declaring def type's known fields -- and, by the monotonic growth of
/// fields down an inheritance chain, also known to every descendant that later folds this
/// declaration in.
///
/// `base` may come from either an ancestor def type's already-resolved view (Def-type
/// inheritance) or a lower-precedence pack's already-applied declaration for the SAME def type
/// (pack overlay) -- both are folded through this same function in the same declaration-order
/// sequence by the caller (see `fold_effective_layers`), so the two amendment mechanisms are
/// unified rather than treated as separate merge stages.
///
/// Returns `None` when the view is disabled/removed, or when a delta-only declaration (no label)
/// has no base to amend -- Plan.md's recoverable "inherited view amendment with no inherited
/// base" warning; the caller removes the id from the working set in that case.
///
/// Locale-sidecar ownership of `label`/`description` is recorded into `owners`, keyed by
/// `(consuming_def_type, view_id)` -- the SAME key `apply_locale_overlays` looks resources up by
/// (`consuming_def_type` is the def type whose `form_views` map this resolved view ends up under,
/// which is not always `def_type`/`declared_on_def_type`: a descendant that inherits a view
/// unchanged resolves it under its own key too) -- only when the incoming declaration explicitly
/// sets that specific scalar. `view.source.pack_id` is set unconditionally on every fold (even a
/// pure `hiddenFields` delta amendment that touches neither label nor description) and must NOT be
/// used for this check; see `LocaleOwnerMaps` doc comment.
#[allow(clippy::too_many_arguments)]
fn apply_form_view_declaration(
    def_type: &str,
    consuming_def_type: &str,
    view_id: &str,
    base: Option<&SchemaFormView>,
    incoming: &FormViewDef,
    pack_id: &str,
    pack_version: &str,
    path: &str,
    field_path: &str,
    diagnosed_amendments_without_base: &mut HashSet<(String, String, String)>,
    owners: &mut LocaleOwnerMaps,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> Option<SchemaFormView> {
    if incoming.disabled == Some(true) {
        if base.is_none() {
            diagnose_amendment_without_base(
                def_type,
                view_id,
                pack_id,
                path,
                field_path,
                "declares disabled: true",
                diagnosed_amendments_without_base,
                diags,
            );
        }
        return None;
    }

    match base {
        None => {
            let Some(label) = incoming.label.clone() else {
                diagnose_amendment_without_base(
                    def_type,
                    view_id,
                    pack_id,
                    path,
                    field_path,
                    "is a delta amendment (hiddenFields/unhideFields/replace)",
                    diagnosed_amendments_without_base,
                    diags,
                );
                return None;
            };
            let hidden_set: BTreeSet<String> = incoming
                .hidden_fields
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect();
            // A first (non-amendment) declaration always supplies its own label (enforced above),
            // so its owner is unconditionally this pack; description only if actually supplied.
            owners.form_view_labels.insert(
                (consuming_def_type.to_string(), view_id.to_string()),
                pack_id.to_string(),
            );
            if incoming.description.is_some() {
                owners.form_view_descriptions.insert(
                    (consuming_def_type.to_string(), view_id.to_string()),
                    pack_id.to_string(),
                );
            }
            Some(SchemaFormView {
                id: view_id.to_string(),
                label,
                description: incoming.description.clone(),
                icon: incoming.icon.clone(),
                order: incoming.order.unwrap_or(0),
                recommended: incoming.recommended.unwrap_or(false),
                hidden_field_ids: hidden_set.into_iter().collect(),
                declared_on_def_type: def_type.to_string(),
                source: Some(FormViewSource {
                    pack_id: pack_id.to_string(),
                    pack_version: pack_version.to_string(),
                }),
            })
        }
        Some(base_view) => {
            let mut hidden_set: BTreeSet<String> = if incoming.replace == Some(true) {
                BTreeSet::new()
            } else {
                base_view.hidden_field_ids.iter().cloned().collect()
            };
            if let Some(add) = &incoming.hidden_fields {
                for name in add {
                    hidden_set.insert(name.clone());
                }
            }
            if let Some(remove) = &incoming.unhide_fields {
                for name in remove {
                    hidden_set.remove(name);
                }
            }
            // Only transfer label/description ownership to this pack when its own declaration
            // explicitly supplied that scalar -- a delta amendment (e.g. hiddenFields-only) must
            // not thereby gain sidecar-override rights over display text it never touched.
            if incoming.label.is_some() {
                owners.form_view_labels.insert(
                    (consuming_def_type.to_string(), view_id.to_string()),
                    pack_id.to_string(),
                );
            }
            if incoming.description.is_some() {
                owners.form_view_descriptions.insert(
                    (consuming_def_type.to_string(), view_id.to_string()),
                    pack_id.to_string(),
                );
            }
            Some(SchemaFormView {
                id: view_id.to_string(),
                label: incoming
                    .label
                    .clone()
                    .unwrap_or_else(|| base_view.label.clone()),
                description: incoming
                    .description
                    .clone()
                    .or_else(|| base_view.description.clone()),
                icon: incoming.icon.clone().or_else(|| base_view.icon.clone()),
                order: incoming.order.unwrap_or(base_view.order),
                recommended: incoming.recommended.unwrap_or(base_view.recommended),
                hidden_field_ids: hidden_set.into_iter().collect(),
                declared_on_def_type: def_type.to_string(),
                source: Some(FormViewSource {
                    pack_id: pack_id.to_string(),
                    pack_version: pack_version.to_string(),
                }),
            })
        }
    }
}

/// Push a `schema_pack_form_view_amendment_without_base` warning at most once per
/// `(def_type, pack_id, view_id)`, since the same declaration can be folded more than once across
/// a multi-parent `inherits` DAG (see `compute_effective_layers`/`fold_effective_layers`).
#[allow(clippy::too_many_arguments)]
fn diagnose_amendment_without_base(
    def_type: &str,
    view_id: &str,
    pack_id: &str,
    path: &str,
    field_path: &str,
    detail: &str,
    diagnosed_amendments_without_base: &mut HashSet<(String, String, String)>,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) {
    let key = (
        def_type.to_string(),
        pack_id.to_string(),
        view_id.to_string(),
    );
    if diagnosed_amendments_without_base.insert(key) {
        diags.push(
            SchemaLoadDiagnostic::warning(
                "schema_pack_form_view_amendment_without_base",
                format!(
                    "Form View '{view_id}' on '{def_type}' {detail} but no inherited or prior view exists for that id; the declaration is ignored."
                ),
            )
            .with_pack_id(pack_id)
            .with_path(path)
            .with_field_path(field_path),
        );
    }
}

/// Filter `names` down to those present in `known_fields`, warning (not erroring) for each one
/// that isn't. Never lets an unknown field id silently stay in an effective hidden/unhide set --
/// Plan.md: "Unknown field references must never hide arbitrary current XML... warn once per
/// catalog load."
#[allow(clippy::too_many_arguments)]
fn filter_known_field_refs(
    names: &[String],
    list_label: &str,
    known_fields: &HashSet<String>,
    view_id: &str,
    def_type: &str,
    pack_id: &str,
    path: &str,
    field_path: &str,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> Vec<String> {
    let mut kept = Vec::new();
    for name in names {
        if known_fields.contains(name) {
            kept.push(name.clone());
        } else {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_form_view_unknown_field_reference",
                    format!(
                        "Form View '{view_id}' on '{def_type}' references unknown field '{name}' in {list_label}; the reference is ignored."
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path)
                .with_field_path(field_path),
            );
        }
    }
    kept
}
