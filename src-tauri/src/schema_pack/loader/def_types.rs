use crate::schema_pack::model::{DefTypeSchemaFile, FieldTypeKind, SchemaLoadDiagnostic};

/// Parse a single def type file. `pack_id` is used only for diagnostic context.
/// `manifest_format_version` is the owning pack's manifest `formatVersion`: it gates whether
/// `formViews` is even attempted on this Def type (see below) and must therefore be resolved by
/// the caller (from the pack's already-parsed manifest) before this function runs.
pub fn parse_def_type_schema(
    path_label: &str,
    pack_id: &str,
    raw_json: &str,
    manifest_format_version: u16,
) -> (Option<DefTypeSchemaFile>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();

    let mut value: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_def_type_json_invalid",
                    format!("JSON parse error in def file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    let def_type = value
        .get("defType")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if def_type.is_empty() {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_missing_def_type",
                "Def file is missing a non-empty defType field.",
            )
            .with_pack_id(pack_id)
            .with_path(path_label),
        );
        return (None, diags);
    }

    // formViews requires manifest formatVersion 3 (Plan.md section 4). This gate MUST run before
    // any v3-only structural validation: a v1/v2 pack's formViews (whether well-formed, malformed
    // shape, or semantically invalid -- e.g. the reserved "default" id) is simply not a supported
    // feature on that pack version. It is a contract violation worth a diagnostic, but must not
    // sink the rest of the Def-type file (fields/templates/etc. still load), and the below
    // whole-file-fatal structural checks must never even run against content this pack version
    // doesn't support. Detect + report + strip the key here, before full struct deserialization,
    // so garbage formViews content on an old pack can't cause a spurious whole-file "malformed
    // JSON" failure either.
    if manifest_format_version < 3 && value_has_nonempty_form_views(&value) {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_form_views_requires_v3",
                format!(
                    "Def type '{}' declares formViews, but the pack manifest formatVersion is {}. formViews requires formatVersion 3; all formViews declarations for this Def type are ignored.",
                    def_type, manifest_format_version
                ),
            )
            .with_pack_id(pack_id)
            .with_path(path_label)
            .with_field_path(format!("{}.formViews", def_type))
            .with_args(crate::diagnostics::diagnostic_args([
                ("defType", def_type.as_str().into()),
                ("formatVersion", (manifest_format_version as i64).into()),
            ])),
        );
        if let Some(obj) = value.as_object_mut() {
            obj.remove("formViews");
        }
    }

    let mut def_file: DefTypeSchemaFile = if manifest_format_version < 3 {
        // `formViews` has already been stripped above if it was present -- a v1/v2 pack never
        // reaches the v3-only duplicate-id detection below, so deserializing from the
        // already-built `value` (rather than raw text) loses nothing relevant here.
        match serde_json::from_value(value) {
            Ok(d) => d,
            Err(e) => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_def_type_json_invalid",
                        format!("Failed to deserialize def file: {}", e),
                    )
                    .with_pack_id(pack_id)
                    .with_path(path_label),
                );
                return (None, diags);
            }
        }
    } else {
        // Deserialize directly from the original JSON text, not from the already-built `value`
        // above: `serde_json::Value`'s `Map` construction silently collapses a duplicate object
        // key (e.g. two `"formViews": { "weapon": {...}, "weapon": {...} }` entries) before we
        // would ever see it. Deserializing straight from `raw_json` lets
        // `DefTypeSchemaDef.form_views`'s custom `deserialize_form_views` (see model.rs) observe
        // genuine duplicate keys via the flatten buffering's `MapAccess` and reject them as an
        // ordinary deserialize error here.
        //
        // A structured field path (e.g. via `serde_path_to_error`) was investigated for a
        // deserialize-level failure under `formViews` (e.g. `"label": 1` instead of a string),
        // but `serde_path_to_error` cannot see through `DefTypeSchemaFile`'s `#[serde(flatten)]`
        // field: flatten's derive-generated code buffers the whole object into an internal
        // `Content` tree via a plain (untracked) deserialize, then redistributes each field from
        // that buffer with a fresh, untracked deserializer -- so path tracking is lost the moment
        // it crosses the flatten boundary, for any field, not just `formViews` (confirmed
        // empirically: wrapping still produced `path() == "."`). Reworking `DefTypeSchemaFile`
        // away from `flatten` to fix this would touch every field on `DefTypeSchemaDef`, which is
        // disproportionate for this one diagnostic's precision. `serde_json::Error`'s `Display`
        // already includes a line/column ("at line N column M"), which is included in the message
        // below -- an acceptable fallback location hint without a structured field path.
        match serde_json::from_str(raw_json) {
            Ok(d) => d,
            Err(e) => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_def_type_json_invalid",
                        format!("Failed to deserialize def file: {}", e),
                    )
                    .with_pack_id(pack_id)
                    .with_path(path_label),
                );
                return (None, diags);
            }
        }
    };

    // Normalize Unrecognized field type kinds to Unknown, emitting warnings.
    for (field_name, field) in def_file.schema.fields.iter_mut() {
        if field.field_type.kind == FieldTypeKind::Unrecognized {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_invalid_field_type",
                    format!(
                        "Unrecognized field type kind in {}.{}. Field treated as unknown.",
                        def_file.def_type, field_name
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(format!("{}.fields.{}", def_file.def_type, field_name))
                .with_args(crate::diagnostics::diagnostic_args([
                    ("defType", def_file.def_type.as_str().into()),
                    ("fieldName", field_name.as_str().into()),
                ])),
            );
            field.field_type.kind = FieldTypeKind::Unknown;
        }
    }

    // Per Plan.md section 5, a malformed formViews shape, blank/reserved id, blank/missing
    // label, impossible `disabled` combination, or contradictory/duplicate field list is fatal
    // for the whole v3 Def schema file -- not a recoverable per-declaration skip. Mirror the same
    // whole-file-rejection mechanism used above for a genuinely malformed def file. Only reached
    // for a confirmed v3+ pack: a v1/v2 pack's formViews was already stripped to empty above, so
    // this loop body never runs for it.
    if manifest_format_version >= 3 && !def_file.schema.form_views.is_empty() {
        let form_view_diags = super::form_views::validate_form_view_declarations(
            &def_file.def_type,
            pack_id,
            path_label,
            &def_file.schema.form_views,
        );
        if !form_view_diags.is_empty() {
            diags.extend(form_view_diags);
            return (None, diags);
        }
    }

    (Some(def_file), diags)
}

/// Whether `value`'s top-level `formViews` key is present with meaningful content: only a wholly
/// absent key or an explicit empty object count as "no formViews" (nothing to gate or report,
/// since both declare zero views). Any other shape counts as present -- including an explicit
/// `null`: `null` is NOT equivalent to "key absent" here. `#[serde(default)]` on
/// `DefTypeSchemaDef.form_views` only substitutes a default when the key is missing entirely, not
/// when it is present-but-null, so an unstripped `formViews: null` would otherwise reach
/// `serde_json::from_value` and fail to deserialize into `BTreeMap<String, FormViewDef>`, taking
/// the whole v1/v2 Def-type file down with it (the same whole-file-loss bug a non-object shape
/// like an array would cause if left unstripped). A non-empty object also counts as present, same
/// as before.
fn value_has_nonempty_form_views(value: &serde_json::Value) -> bool {
    match value.get("formViews") {
        None => false,
        Some(serde_json::Value::Object(map)) => !map.is_empty(),
        Some(_) => true,
    }
}
