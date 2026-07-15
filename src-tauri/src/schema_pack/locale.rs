//! Deterministic locale-resource key grammar for schema-pack locale sidecars, and the merge-time
//! overlay application that applies a pack's own sidecar overrides onto the catalog it contributed
//! to. See `docs/i18n/issues/05-schema-pack-localization.md` and `Plan.md`'s "Ownership and
//! resource model" section.
//!
//! ## Sidecar shape
//!
//! A pack manifest may declare `"localesDirectory": "<pack-root-relative dir>"`. Inside that
//! directory, each `<bcp47-tag>.json` file (e.g. `en.json`) is a flat JSON object mapping a
//! deterministic resource id to a plain string override, e.g.
//! `{ "defTypes.ThingDef.fields.label.description": "..." }`. Sidecars only ever override
//! non-technical display text (label/description/validation-rule message) -- never an identifier,
//! xml shape, type, or validation condition. Existing English `label`/`description`/`message`
//! values in the pack's own def/object/patch-operation JSON remain the canonical fallback; a
//! sidecar's job is only to override what an active locale renders, never to duplicate or replace
//! the canonical source of truth.
//!
//! ## Ownership
//!
//! A sidecar may only override a resource whose winning merged value actually came from the SAME
//! pack that shipped the sidecar. Ownership is tracked **per display-metadata scalar** (a
//! `label` and a `description`/`message` are always two independent owners, never one owner for
//! the whole containing record) and is only ever transferred to a pack when that pack's own JSON
//! explicitly set that specific scalar -- never merely because the pack touched some other,
//! unrelated property of the same record (a field's `type`/`examples`/`xml` shape, a patch
//! operation's extra field or `fieldOrder`, a Form View's `hiddenFields` delta, ...). This
//! prevents an amending pack from silently inheriting sidecar-override rights over display text it
//! never itself supplied.
//!
//! Two mechanisms implement this, both built in lockstep with the ordinary merge pass:
//!
//! - `DefTypeSchema`/`ObjectTypeSchema`'s own top-level `label`/`description`,
//!   `PatchOperationMetadata`'s own top-level `label`/`description`/`preview.message`, a resolved
//!   Form View's `label`/`description`, and `ValidationRule.message` carry no per-scalar
//!   provenance field of their own (or, for the first three, only a coarser whole-record
//!   `source_pack_id`/`source` that gets overwritten on every unrelated amendment) -- so
//!   [`LocaleOwnerMaps`] tracks "last pack that explicitly set this scalar" for those externally,
//!   keyed by def/object type name, patch operation class name, or `(def type, view id)`.
//! - `FieldSchema` (shared by def-type fields, object-type fields, and patch-operation fields)
//!   instead carries this directly on the struct, as `label_source_pack_id`/
//!   `description_source_pack_id` -- separate from the field's own coarser `source_pack_id`,
//!   which (like the whole-record fields above) is set unconditionally on every amendment and
//!   must not be used for ownership checks.
//!
//! Either way, an entry/field is only written when the incoming pack's own JSON explicitly sets
//! that specific scalar (`Some(..)`), mirroring the exact condition `merge_packs` already uses to
//! decide whether to overwrite the scalar itself -- so ownership always matches "whichever pack's
//! value the scalar is currently showing." An override targeting a resource owned by a different
//! pack, or a resource that doesn't resolve (or was never explicitly set by any pack) at all, is
//! ignored with a recoverable diagnostic; it never fails the rest of the sidecar closed.

use std::collections::BTreeMap;

use super::loader::LoadedPack;
use super::model::{
    DefTypeSchema, ObjectTypeSchema, PatchOperationMetadata, SchemaLoadDiagnostic, ValidationRule,
};

/// One pack's parsed locale sidecar overrides for a single locale: a flat map from deterministic
/// schema resource id (see [`parse_locale_key`]) to override text.
pub type SchemaLocaleOverlay = BTreeMap<String, String>;

/// A single deterministic locale resource id, parsed from its dotted string form by
/// [`parse_locale_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocaleTarget {
    DefLabel(String),
    DefDescription(String),
    DefFieldLabel(String, String),
    DefFieldDescription(String, String),
    DefTemplateLabel(String, String),
    DefTemplateDescription(String, String),
    DefFormViewLabel(String, String),
    DefFormViewDescription(String, String),
    DefValidationRuleMessage(String, String),
    ObjectLabel(String),
    ObjectDescription(String),
    ObjectFieldLabel(String, String),
    ObjectFieldDescription(String, String),
    PatchOperationLabel(String),
    PatchOperationDescription(String),
    PatchOperationFieldLabel(String, String),
    PatchOperationFieldDescription(String, String),
    PatchOperationPreviewMessage(String),
}

/// Parse a dotted resource id (e.g. `"defTypes.ThingDef.fields.label.description"`) into a
/// [`LocaleTarget`]. Returns `None` for any shape that doesn't match one of the documented
/// grammar patterns -- including a pattern with an empty path segment (e.g.
/// `"defTypes..label"`) -- so callers can uniformly treat it as an unknown key.
pub fn parse_locale_key(key: &str) -> Option<LocaleTarget> {
    let parts: Vec<&str> = key.split('.').collect();
    let ok = |s: &str| !s.trim().is_empty();
    match parts.as_slice() {
        ["defTypes", d, "label"] if ok(d) => Some(LocaleTarget::DefLabel((*d).to_string())),
        ["defTypes", d, "description"] if ok(d) => {
            Some(LocaleTarget::DefDescription((*d).to_string()))
        }
        ["defTypes", d, "fields", f, "label"] if ok(d) && ok(f) => Some(
            LocaleTarget::DefFieldLabel((*d).to_string(), (*f).to_string()),
        ),
        ["defTypes", d, "fields", f, "description"] if ok(d) && ok(f) => Some(
            LocaleTarget::DefFieldDescription((*d).to_string(), (*f).to_string()),
        ),
        ["defTypes", d, "templates", t, "label"] if ok(d) && ok(t) => Some(
            LocaleTarget::DefTemplateLabel((*d).to_string(), (*t).to_string()),
        ),
        ["defTypes", d, "templates", t, "description"] if ok(d) && ok(t) => Some(
            LocaleTarget::DefTemplateDescription((*d).to_string(), (*t).to_string()),
        ),
        ["defTypes", d, "formViews", v, "label"] if ok(d) && ok(v) => Some(
            LocaleTarget::DefFormViewLabel((*d).to_string(), (*v).to_string()),
        ),
        ["defTypes", d, "formViews", v, "description"] if ok(d) && ok(v) => Some(
            LocaleTarget::DefFormViewDescription((*d).to_string(), (*v).to_string()),
        ),
        ["defTypes", d, "validationRules", r, "message"] if ok(d) && ok(r) => Some(
            LocaleTarget::DefValidationRuleMessage((*d).to_string(), (*r).to_string()),
        ),
        ["objectTypes", o, "label"] if ok(o) => Some(LocaleTarget::ObjectLabel((*o).to_string())),
        ["objectTypes", o, "description"] if ok(o) => {
            Some(LocaleTarget::ObjectDescription((*o).to_string()))
        }
        ["objectTypes", o, "fields", f, "label"] if ok(o) && ok(f) => Some(
            LocaleTarget::ObjectFieldLabel((*o).to_string(), (*f).to_string()),
        ),
        ["objectTypes", o, "fields", f, "description"] if ok(o) && ok(f) => Some(
            LocaleTarget::ObjectFieldDescription((*o).to_string(), (*f).to_string()),
        ),
        ["patchOperations", c, "label"] if ok(c) => {
            Some(LocaleTarget::PatchOperationLabel((*c).to_string()))
        }
        ["patchOperations", c, "description"] if ok(c) => {
            Some(LocaleTarget::PatchOperationDescription((*c).to_string()))
        }
        ["patchOperations", c, "fields", f, "label"] if ok(c) && ok(f) => Some(
            LocaleTarget::PatchOperationFieldLabel((*c).to_string(), (*f).to_string()),
        ),
        ["patchOperations", c, "fields", f, "description"] if ok(c) && ok(f) => Some(
            LocaleTarget::PatchOperationFieldDescription((*c).to_string(), (*f).to_string()),
        ),
        ["patchOperations", c, "preview", "message"] if ok(c) => {
            Some(LocaleTarget::PatchOperationPreviewMessage((*c).to_string()))
        }
        _ => None,
    }
}

/// Whether `tag` is a plausible BCP-47 language tag shape: one or more `-`-separated
/// alphanumeric subtags (max 8 chars each), starting with an alphabetic primary subtag no longer
/// than 8 characters. This is a light structural check, not full BCP-47/IANA subtag-registry
/// validation -- sufficient to reject an obviously-not-a-locale file name without embedding a
/// subtag registry in the binary.
pub fn is_plausible_locale_tag(tag: &str) -> bool {
    let mut subtags = tag.split('-');
    let Some(primary) = subtags.next() else {
        return false;
    };
    if primary.is_empty() || primary.len() > 8 || !primary.chars().all(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    for subtag in subtags {
        if subtag.is_empty()
            || subtag.len() > 8
            || !subtag.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return false;
        }
    }
    true
}

/// Parse and validate one locale sidecar file's raw JSON content into a [`SchemaLocaleOverlay`].
/// `locale_tag` must already be validated as a plausible BCP-47 shape by the caller (see
/// [`is_plausible_locale_tag`]) -- this function only validates the JSON body: it must be a flat
/// object of string keys to string values. A key that doesn't match the deterministic grammar
/// ([`parse_locale_key`]), or a non-string value, is skipped with a recoverable warning rather
/// than failing the whole file, mirroring `loader::parse_def_type_schema`'s per-field
/// `schema_pack_invalid_field_type` recoverable-warning convention. A malformed top-level JSON
/// document, or a JSON value that isn't an object, IS fatal for the file (returns `None`) -- there
/// is no reasonable partial result to salvage from either.
pub fn parse_schema_pack_locale_file(
    path_label: &str,
    pack_id: &str,
    locale_tag: &str,
    raw_json: &str,
) -> (Option<SchemaLocaleOverlay>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();

    let entries: Vec<(String, serde_json::Value)> =
        match serde_json::from_str::<RawLocaleObjectEntries>(raw_json) {
            Ok(v) => v.0,
            Err(e) => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_locale_json_invalid",
                        format!("JSON parse error in locale file: {}", e),
                    )
                    .with_pack_id(pack_id)
                    .with_path(path_label),
                );
                return (None, diags);
            }
        };

    let mut seen_keys = std::collections::HashSet::new();
    let mut overlay = SchemaLocaleOverlay::new();
    for (key, raw_value) in &entries {
        if !seen_keys.insert(key.clone()) {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_locale_duplicate_key",
                    format!(
                        "Locale '{locale_tag}' key '{key}' is defined more than once in this file; the last occurrence wins."
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(key.clone()),
            );
        }
        let Some(text) = raw_value.as_str() else {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_locale_non_string_value",
                    format!(
                        "Locale '{locale_tag}' key '{key}' does not have a string value; ignored."
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(key.clone()),
            );
            continue;
        };
        if parse_locale_key(key).is_none() {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_locale_unknown_key",
                    format!(
                        "Locale '{locale_tag}' key '{key}' does not match a known schema resource id; ignored."
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(key.clone()),
            );
            continue;
        }
        // `overlay.insert` naturally implements "last occurrence wins" for a duplicate key --
        // identical to the pre-fix behavior's silent collapse -- except every occurrence has now
        // been observed and, if there's more than one, reported above.
        overlay.insert(key.clone(), text.to_string());
    }

    (Some(overlay), diags)
}

/// A duplicate key in a JSON object silently collapses to "last one wins" under
/// `serde_json::Value`'s standard `Deserialize` impl (`serde_json`/`serde`'s well-documented
/// default map behavior): by the time `serde_json::from_str::<Value>` returns, the fact that a
/// duplicate ever existed is already gone, so no downstream check over the resulting `Value` can
/// recover it. This wrapper type intercepts the object at parse time, before that information is
/// lost, via a custom [`serde::de::Deserialize`] impl driven directly off
/// [`serde::de::MapAccess`]: it records *every* `(key, value)` pair it walks, in file order,
/// including repeats, rather than folding them into a map as it goes. `parse_schema_pack_locale_file`
/// then does its own single linear pass over that list to both detect duplicates and -- via
/// `BTreeMap::insert`'s ordinary overwrite semantics -- preserve the exact same "last occurrence
/// wins" value resolution the old `Value`-based parse had, so the only externally-visible change
/// is that a duplicate is now reported, never that a different value wins.
///
/// Locale sidecars are always a single flat top-level object (see this module's "Sidecar shape"
/// doc comment) -- never nested -- so only the top level needs this treatment.
struct RawLocaleObjectEntries(Vec<(String, serde_json::Value)>);

impl<'de> serde::de::Deserialize<'de> for RawLocaleObjectEntries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct EntriesVisitor;

        impl<'de> serde::de::Visitor<'de> for EntriesVisitor {
            type Value = RawLocaleObjectEntries;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a flat JSON object of resource id to string overrides")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
                    entries.push((key, value));
                }
                Ok(RawLocaleObjectEntries(entries))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

/// "Last pack that explicitly set this scalar" provenance for the catalog record kinds/scalars
/// that don't already carry their own per-scalar `source_pack_id`/`source` field: `DefTypeSchema`/
/// `ObjectTypeSchema`'s own top-level `label`/`description`, `PatchOperationMetadata`'s own
/// top-level `label`/`description`/`preview.message`, and a resolved Form View's `label`/
/// `description` (tracked as separate maps per scalar -- see the module doc comment for why a
/// whole-record owner is wrong). `ValidationRule`'s `message` also lives here since it carries no
/// provenance field of its own. Built by `merge::merge_packs` (and, for form views,
/// `merge::resolve_all_form_views`) in lockstep with the ordinary merge loop.
///
/// `PatchOperationMetadata.source_pack_id` and a Form View's `source.pack_id` DO exist as fields on
/// those records, but -- like `FieldSchema.source_pack_id` before it -- they are set unconditionally
/// on every amendment (including one that changes only unrelated properties: a new field, a
/// `hiddenFields` delta, ...), so they are informational "last pack that touched this record" only
/// and must NOT be used for locale ownership. The maps here are the ones actually consulted.
#[derive(Default)]
pub struct LocaleOwnerMaps {
    pub def_type_labels: BTreeMap<String, String>,
    pub def_type_descriptions: BTreeMap<String, String>,
    pub object_type_labels: BTreeMap<String, String>,
    pub object_type_descriptions: BTreeMap<String, String>,
    pub patch_operation_labels: BTreeMap<String, String>,
    pub patch_operation_descriptions: BTreeMap<String, String>,
    pub patch_operation_preview_messages: BTreeMap<String, String>,
    /// Keyed by `(def_type, view_id)`.
    pub form_view_labels: BTreeMap<(String, String), String>,
    /// Keyed by `(def_type, view_id)`.
    pub form_view_descriptions: BTreeMap<(String, String), String>,
    /// Keyed by `(def_type, rule_id)`.
    pub validation_rules: BTreeMap<(String, String), String>,
}

/// Apply every pack's own `locale` overlay onto the already fully-merged catalog maps, in place.
/// Must run strictly after `merge::merge_packs`'s ordinary pack-precedence/inheritance merge is
/// complete (see this module's doc comment and the issue's "Risks" section on merge provenance).
///
/// For each override key: unresolvable resources and resources owned by a different pack are
/// skipped with a recoverable diagnostic; the rest of the pack's overlay, and every other pack's
/// overlay, still applies.
pub fn apply_locale_overlays(
    def_types: &mut BTreeMap<String, DefTypeSchema>,
    object_types: &mut BTreeMap<String, ObjectTypeSchema>,
    patch_operations: &mut BTreeMap<String, PatchOperationMetadata>,
    packs: &[LoadedPack],
    locale: &str,
    owners: &LocaleOwnerMaps,
) -> Vec<SchemaLoadDiagnostic> {
    let mut diags = Vec::new();
    for pack in packs {
        let pack_id = pack.manifest.pack_id.as_str();
        let Some(overlay) = pack.locales.get(locale) else {
            continue;
        };
        for (key, value) in overlay {
            // Already validated by `parse_schema_pack_locale_file` at load time -- only
            // well-formed keys ever reach a stored overlay, so this always succeeds.
            let Some(target) = parse_locale_key(key) else {
                continue;
            };
            apply_one_override(
                def_types,
                object_types,
                patch_operations,
                &target,
                key,
                value,
                pack_id,
                owners,
                &mut diags,
            );
        }
    }
    diags
}

fn unresolved(diags: &mut Vec<SchemaLoadDiagnostic>, pack_id: &str, key: &str) {
    diags.push(
        SchemaLoadDiagnostic::warning(
            "schema_pack_locale_unresolved_key",
            format!(
                "Pack '{pack_id}' locale override '{key}' does not resolve to a known schema resource; ignored."
            ),
        )
        .with_pack_id(pack_id)
        .with_field_path(key.to_string()),
    );
}

fn wrong_owner(diags: &mut Vec<SchemaLoadDiagnostic>, pack_id: &str, key: &str, owner: &str) {
    diags.push(
        SchemaLoadDiagnostic::warning(
            "schema_pack_locale_wrong_owner",
            format!(
                "Pack '{pack_id}' locale override '{key}' targets a resource owned by pack '{owner}'; ignored."
            ),
        )
        .with_pack_id(pack_id)
        .with_field_path(key.to_string()),
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_one_override(
    def_types: &mut BTreeMap<String, DefTypeSchema>,
    object_types: &mut BTreeMap<String, ObjectTypeSchema>,
    patch_operations: &mut BTreeMap<String, PatchOperationMetadata>,
    target: &LocaleTarget,
    key: &str,
    value: &str,
    pack_id: &str,
    owners: &LocaleOwnerMaps,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) {
    match target {
        LocaleTarget::DefLabel(d) | LocaleTarget::DefDescription(d) => {
            let Some(schema) = def_types.get_mut(d) else {
                return unresolved(diags, pack_id, key);
            };
            let owner_map = if matches!(target, LocaleTarget::DefLabel(_)) {
                &owners.def_type_labels
            } else {
                &owners.def_type_descriptions
            };
            match owner_map.get(d) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            if matches!(target, LocaleTarget::DefLabel(_)) {
                schema.label = Some(value.to_string());
            } else {
                schema.description = Some(value.to_string());
            }
        }
        LocaleTarget::DefFieldLabel(d, f) | LocaleTarget::DefFieldDescription(d, f) => {
            let Some(field) = def_types.get_mut(d).and_then(|s| s.fields.get_mut(f)) else {
                return unresolved(diags, pack_id, key);
            };
            let is_label = matches!(target, LocaleTarget::DefFieldLabel(..));
            let owner = if is_label {
                field.label_source_pack_id.clone()
            } else {
                field.description_source_pack_id.clone()
            };
            match owner {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, &owner),
                None => return unresolved(diags, pack_id, key),
            }
            if is_label {
                field.label = Some(value.to_string());
            } else {
                field.description = Some(value.to_string());
            }
        }
        LocaleTarget::DefTemplateLabel(d, t) => {
            let Some(template) = def_types.get_mut(d).and_then(|s| s.templates.get_mut(t)) else {
                return unresolved(diags, pack_id, key);
            };
            match template.source_pack_id.clone() {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, &owner),
                None => return unresolved(diags, pack_id, key),
            }
            template.label = value.to_string();
        }
        LocaleTarget::DefTemplateDescription(d, t) => {
            let Some(template) = def_types.get_mut(d).and_then(|s| s.templates.get_mut(t)) else {
                return unresolved(diags, pack_id, key);
            };
            match template.source_pack_id.clone() {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, &owner),
                None => return unresolved(diags, pack_id, key),
            }
            template.description = Some(value.to_string());
        }
        LocaleTarget::DefFormViewLabel(d, v) => {
            let Some(view) = def_types.get_mut(d).and_then(|s| s.form_views.get_mut(v)) else {
                return unresolved(diags, pack_id, key);
            };
            match owners.form_view_labels.get(&(d.clone(), v.clone())) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            view.label = value.to_string();
        }
        LocaleTarget::DefFormViewDescription(d, v) => {
            let Some(view) = def_types.get_mut(d).and_then(|s| s.form_views.get_mut(v)) else {
                return unresolved(diags, pack_id, key);
            };
            match owners.form_view_descriptions.get(&(d.clone(), v.clone())) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            view.description = Some(value.to_string());
        }
        LocaleTarget::DefValidationRuleMessage(d, r) => {
            let Some(rule) = def_types
                .get_mut(d)
                .and_then(|s| s.validation_rules.get_mut(r))
            else {
                return unresolved(diags, pack_id, key);
            };
            match owners.validation_rules.get(&(d.clone(), r.clone())) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            match rule {
                ValidationRule::RequiredWhen { message, .. } => {
                    *message = value.to_string();
                }
            }
        }
        LocaleTarget::ObjectLabel(o) | LocaleTarget::ObjectDescription(o) => {
            let Some(schema) = object_types.get_mut(o) else {
                return unresolved(diags, pack_id, key);
            };
            let owner_map = if matches!(target, LocaleTarget::ObjectLabel(_)) {
                &owners.object_type_labels
            } else {
                &owners.object_type_descriptions
            };
            match owner_map.get(o) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            if matches!(target, LocaleTarget::ObjectLabel(_)) {
                schema.label = Some(value.to_string());
            } else {
                schema.description = Some(value.to_string());
            }
        }
        LocaleTarget::ObjectFieldLabel(o, f) | LocaleTarget::ObjectFieldDescription(o, f) => {
            let Some(field) = object_types.get_mut(o).and_then(|s| s.fields.get_mut(f)) else {
                return unresolved(diags, pack_id, key);
            };
            let is_label = matches!(target, LocaleTarget::ObjectFieldLabel(..));
            let owner = if is_label {
                field.label_source_pack_id.clone()
            } else {
                field.description_source_pack_id.clone()
            };
            match owner {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, &owner),
                None => return unresolved(diags, pack_id, key),
            }
            if is_label {
                field.label = Some(value.to_string());
            } else {
                field.description = Some(value.to_string());
            }
        }
        LocaleTarget::PatchOperationLabel(c) | LocaleTarget::PatchOperationDescription(c) => {
            let Some(op) = patch_operations.get_mut(c) else {
                return unresolved(diags, pack_id, key);
            };
            let is_label = matches!(target, LocaleTarget::PatchOperationLabel(_));
            let owner_map = if is_label {
                &owners.patch_operation_labels
            } else {
                &owners.patch_operation_descriptions
            };
            match owner_map.get(c) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            if is_label {
                op.label = Some(value.to_string());
            } else {
                op.description = Some(value.to_string());
            }
        }
        LocaleTarget::PatchOperationFieldLabel(c, f)
        | LocaleTarget::PatchOperationFieldDescription(c, f) => {
            let Some(field) = patch_operations
                .get_mut(c)
                .and_then(|s| s.fields.get_mut(f))
            else {
                return unresolved(diags, pack_id, key);
            };
            let is_label = matches!(target, LocaleTarget::PatchOperationFieldLabel(..));
            let owner = if is_label {
                field.label_source_pack_id.clone()
            } else {
                field.description_source_pack_id.clone()
            };
            match owner {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, &owner),
                None => return unresolved(diags, pack_id, key),
            }
            if is_label {
                field.label = Some(value.to_string());
            } else {
                field.description = Some(value.to_string());
            }
        }
        LocaleTarget::PatchOperationPreviewMessage(c) => {
            let Some(op) = patch_operations.get_mut(c) else {
                return unresolved(diags, pack_id, key);
            };
            match owners.patch_operation_preview_messages.get(c) {
                Some(owner) if owner == pack_id => {}
                Some(owner) => return wrong_owner(diags, pack_id, key, owner),
                None => return unresolved(diags, pack_id, key),
            }
            op.preview.message = Some(value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_documented_key_shape() {
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.label"),
            Some(LocaleTarget::DefLabel("ThingDef".to_string()))
        );
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.description"),
            Some(LocaleTarget::DefDescription("ThingDef".to_string()))
        );
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.fields.label.label"),
            Some(LocaleTarget::DefFieldLabel(
                "ThingDef".to_string(),
                "label".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.fields.label.description"),
            Some(LocaleTarget::DefFieldDescription(
                "ThingDef".to_string(),
                "label".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.templates.basic.label"),
            Some(LocaleTarget::DefTemplateLabel(
                "ThingDef".to_string(),
                "basic".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.formViews.weapon.label"),
            Some(LocaleTarget::DefFormViewLabel(
                "ThingDef".to_string(),
                "weapon".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("defTypes.ThingDef.validationRules.rule1.message"),
            Some(LocaleTarget::DefValidationRuleMessage(
                "ThingDef".to_string(),
                "rule1".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("objectTypes.GraphicData.label"),
            Some(LocaleTarget::ObjectLabel("GraphicData".to_string()))
        );
        assert_eq!(
            parse_locale_key("objectTypes.GraphicData.fields.texPath.label"),
            Some(LocaleTarget::ObjectFieldLabel(
                "GraphicData".to_string(),
                "texPath".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("patchOperations.PatchOperationAdd.label"),
            Some(LocaleTarget::PatchOperationLabel(
                "PatchOperationAdd".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("patchOperations.PatchOperationAdd.fields.value.label"),
            Some(LocaleTarget::PatchOperationFieldLabel(
                "PatchOperationAdd".to_string(),
                "value".to_string()
            ))
        );
        assert_eq!(
            parse_locale_key("patchOperations.PatchOperationAdd.preview.message"),
            Some(LocaleTarget::PatchOperationPreviewMessage(
                "PatchOperationAdd".to_string()
            ))
        );
    }

    #[test]
    fn rejects_unknown_shapes_and_empty_segments() {
        assert_eq!(parse_locale_key("defTypes.ThingDef.xml"), None);
        assert_eq!(parse_locale_key("defTypes..label"), None);
        assert_eq!(parse_locale_key("defTypes.ThingDef.fields..label"), None);
        assert_eq!(parse_locale_key("notANamespace.Thing.label"), None);
        assert_eq!(parse_locale_key(""), None);
    }

    #[test]
    fn locale_tag_validator_accepts_common_shapes() {
        assert!(is_plausible_locale_tag("en"));
        assert!(is_plausible_locale_tag("en-US"));
        assert!(is_plausible_locale_tag("zh-Hans"));
    }

    #[test]
    fn locale_tag_validator_rejects_garbage() {
        assert!(!is_plausible_locale_tag(""));
        assert!(!is_plausible_locale_tag("not a locale"));
        assert!(!is_plausible_locale_tag("123"));
        assert!(!is_plausible_locale_tag("-en"));
    }

    #[test]
    fn locale_file_rejects_non_object_json() {
        let (overlay, diags) =
            parse_schema_pack_locale_file("test:en.json", "test.pack", "en", "[]");
        assert!(overlay.is_none());
        assert!(diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_json_invalid"));
    }

    #[test]
    fn locale_file_skips_unknown_keys_and_non_string_values() {
        let raw = r#"{
            "defTypes.ThingDef.label": "Thing",
            "defTypes.ThingDef.xml": "ignored-shape",
            "defTypes.ThingDef.description": 5
        }"#;
        let (overlay, diags) =
            parse_schema_pack_locale_file("test:en.json", "test.pack", "en", raw);
        let overlay = overlay.expect("object JSON must still produce an overlay");
        assert_eq!(overlay.len(), 1);
        assert_eq!(overlay.get("defTypes.ThingDef.label").unwrap(), "Thing");
        assert!(diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_unknown_key"));
        assert!(diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_non_string_value"));
    }

    // A duplicate object key in a locale sidecar used to be
    // silently accepted, because `serde_json::from_str::<Value>` already collapses duplicate
    // keys ("last one wins") before any of this file's own validation ever ran -- there was no
    // way for downstream code to observe that a duplicate had existed. The duplicate key here is
    // constructed as raw JSON text (not a Rust struct literal, which can't represent a duplicate
    // object key at all) so the parser actually sees two occurrences of the same key.
    #[test]
    fn locale_file_reports_duplicate_key_and_keeps_last_value() {
        let raw = r#"{
            "defTypes.ThingDef.label": "First",
            "defTypes.ThingDef.description": "Kept description",
            "defTypes.ThingDef.label": "Second"
        }"#;
        let (overlay, diags) =
            parse_schema_pack_locale_file("test:en.json", "test.pack", "en", raw);
        let overlay = overlay.expect("object JSON must still produce an overlay");

        // Last occurrence still wins for the resolved value -- this fix only adds visibility,
        // it does not change ordinary JSON "last one wins" value resolution.
        assert_eq!(overlay.get("defTypes.ThingDef.label").unwrap(), "Second");
        assert_eq!(
            overlay.get("defTypes.ThingDef.description").unwrap(),
            "Kept description"
        );

        let duplicate_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "schema_pack_locale_duplicate_key")
            .collect();
        assert_eq!(
            duplicate_diags.len(),
            1,
            "expected exactly one duplicate-key diagnostic, got: {:?}",
            diags.iter().map(|d| &d.code).collect::<Vec<_>>()
        );
        assert!(duplicate_diags[0]
            .message
            .contains("defTypes.ThingDef.label"));
    }
}
