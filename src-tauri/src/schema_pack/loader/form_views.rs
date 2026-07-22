use crate::schema_pack::model::{FormViewDef, SchemaLoadDiagnostic};
use std::collections::BTreeMap;

/// Return the first value in `items` that also appears earlier in `items` (a within-array
/// duplicate), or `None` if every entry is unique. Used to reject e.g. `["apparel", "apparel"]`
/// inside a single declaration's `hiddenFields`/`unhideFields`.
fn first_duplicate_in_list(items: &[String]) -> Option<&str> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for item in items {
        if !seen.insert(item.as_str()) {
            return Some(item.as_str());
        }
    }
    None
}

/// Validate one Def-type file's own `formViews` declarations for internal shape/consistency:
/// blank/reserved id, blank/missing label, impossible `disabled` combinations, and contradictory
/// or duplicate-within-array `hiddenFields`/`unhideFields`. Every one of these is listed in
/// Plan.md section 5 as fatal for the whole v3 Def schema file, so this returns diagnostics only
/// (not a sanitized map): `parse_def_type_schema` rejects the entire file -- the same mechanism
/// already used for a genuinely malformed def file -- the moment this returns anything non-empty,
/// rather than dropping just the offending declaration and keeping the rest.
///
/// A duplicate view id (two declarations sharing one JSON key) is caught earlier, during
/// deserialization itself, by `model::deserialize_form_views` -- by the time a `formViews` map
/// reaches this function, its keys are already known-unique.
///
/// Deliberately out of scope here (issue 03's job): resolving `hiddenFields`/`unhideFields`
/// deltas against an inherited base, validating field ids against the real known field universe,
/// and cross-pack/cross-type view precedence. This function only looks at one Def type's own
/// declarations in isolation.
pub(super) fn validate_form_view_declarations(
    def_type: &str,
    pack_id: &str,
    file_path: &str,
    form_views: &BTreeMap<String, FormViewDef>,
) -> Vec<SchemaLoadDiagnostic> {
    let mut diags = Vec::new();

    for (id, view) in form_views {
        let field_path = format!("{}.formViews.{}", def_type, id);

        if id.trim().is_empty() {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_blank_id",
                    format!(
                        "Def type '{}' has a formViews entry with a blank/whitespace-only id.",
                        def_type
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path)
                .with_args(crate::diagnostics::diagnostic_args([(
                    "defType",
                    def_type.into(),
                )])),
            );
            continue;
        }

        if id == "default" {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_reserved_id",
                    format!(
                        "Def type '{}' formViews id 'default' is reserved for the synthetic Default View and cannot be used as a schema-declared view id.",
                        def_type
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path)
                .with_args(crate::diagnostics::diagnostic_args([
                    ("defType", def_type.into()),
                    ("viewId", id.as_str().into()),
                ])),
            );
            continue;
        }

        // "View-defining" metadata beyond the delta-only controls (hiddenFields/unhideFields/
        // replace/disabled). A declaration carrying only delta controls (or `disabled: true`
        // alone) is a legitimate amendment to an inherited view and needs no label of its own --
        // issue 03 resolves it against the inherited base. A declaration carrying any of these is
        // treated as defining a view outright and must have a nonblank label.
        let has_view_metadata = view.description.is_some()
            || view.icon.is_some()
            || view.order.is_some()
            || view.recommended.is_some();
        let has_delta_content = view.hidden_fields.is_some()
            || view.unhide_fields.is_some()
            || view.replace.is_some()
            || view.disabled.is_some();

        // `disabled: true` combined with any other meaningful content is an impossible
        // declaration (Plan.md section 5). This check must run before the label checks below:
        // e.g. `{ "disabled": true, "description": "..." }` must be diagnosed as
        // disabled-with-content, not misdiagnosed as a missing label.
        if view.disabled == Some(true) {
            let other_content = view.label.is_some()
                || has_view_metadata
                || view.hidden_fields.is_some()
                || view.unhide_fields.is_some()
                || view.replace.is_some();
            if other_content {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_disabled_with_content",
                        format!(
                            "Def type '{}' formViews entry '{}' sets disabled: true but also declares other content; disabled must be the only meaningful field on the declaration.",
                            def_type, id
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("defType", def_type.into()),
                        ("viewId", id.as_str().into()),
                    ])),
                );
                continue;
            }
        }

        if view.label.is_none() && !has_view_metadata && !has_delta_content {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_empty_declaration",
                    format!(
                        "Def type '{}' formViews entry '{}' has no label and no other content (hiddenFields/unhideFields/replace/disabled/description/icon/order/recommended); the declaration is meaningless.",
                        def_type, id
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path)
                .with_args(crate::diagnostics::diagnostic_args([
                    ("defType", def_type.into()),
                    ("viewId", id.as_str().into()),
                ])),
            );
            continue;
        }

        if let Some(label) = &view.label {
            if label.trim().is_empty() {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_blank_label",
                        format!(
                            "Def type '{}' formViews entry '{}' has a blank/whitespace-only label.",
                            def_type, id
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("defType", def_type.into()),
                        ("viewId", id.as_str().into()),
                    ])),
                );
                continue;
            }
        } else if has_view_metadata {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_missing_label",
                    format!(
                        "Def type '{}' formViews entry '{}' declares description/icon/order/recommended (a new view) but has no label. A pure delta amendment (hiddenFields/unhideFields/replace/disabled only) may omit the label.",
                        def_type, id
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path)
                .with_args(crate::diagnostics::diagnostic_args([
                    ("defType", def_type.into()),
                    ("viewId", id.as_str().into()),
                ])),
            );
            continue;
        }

        if let Some(hidden) = &view.hidden_fields {
            if let Some(dup) = first_duplicate_in_list(hidden) {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_duplicate_hidden_field",
                        format!(
                            "Def type '{}' formViews entry '{}' lists field '{}' more than once in hiddenFields.",
                            def_type, id, dup
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("defType", def_type.into()),
                        ("viewId", id.as_str().into()),
                        ("fieldName", dup.into()),
                    ])),
                );
                continue;
            }
        }

        if let Some(unhide) = &view.unhide_fields {
            if let Some(dup) = first_duplicate_in_list(unhide) {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_duplicate_unhide_field",
                        format!(
                            "Def type '{}' formViews entry '{}' lists field '{}' more than once in unhideFields.",
                            def_type, id, dup
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("defType", def_type.into()),
                        ("viewId", id.as_str().into()),
                        ("fieldName", dup.into()),
                    ])),
                );
                continue;
            }
        }

        if let (Some(hidden), Some(unhide)) = (&view.hidden_fields, &view.unhide_fields) {
            let unhide_set: std::collections::BTreeSet<&str> =
                unhide.iter().map(String::as_str).collect();
            let conflicting: Vec<&str> = hidden
                .iter()
                .map(String::as_str)
                .filter(|f| unhide_set.contains(f))
                .collect();
            if !conflicting.is_empty() {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_conflicting_hidden_unhide",
                        format!(
                            "Def type '{}' formViews entry '{}' lists field(s) {:?} in both hiddenFields and unhideFields.",
                            def_type, id, conflicting
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("defType", def_type.into()),
                        ("viewId", id.as_str().into()),
                        (
                            "fieldNames",
                            conflicting
                                .iter()
                                .map(|s| s.to_string())
                                .collect::<Vec<_>>()
                                .into(),
                        ),
                    ])),
                );
                continue;
            }
        }
    }

    diags
}
