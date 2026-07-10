mod cache;
mod loader;
mod lookup;
mod merge;
mod model;

#[cfg(test)]
mod tests;

pub(crate) use cache::SchemaCatalogCacheState;
use loader::{load_built_in_packs, load_external_packs};
pub(crate) use lookup::{
    collect_all_object_inherited_fields, collect_def_subtypes, lookup_def_type, lookup_field,
    lookup_patch_operation_metadata,
};
#[allow(unused_imports)]
pub(crate) use lookup::{lookup_object_field, lookup_object_field_inherited, lookup_object_type};
use merge::merge_packs;
pub use model::SchemaCatalogLoadResult;
pub(crate) use model::{
    DefTemplate, DefTypeSchema, FieldSchema, FieldType, FieldTypeKind, PatchOperationMetadata,
    PatchOperationPreviewKind, ReferenceMetadata, ReferenceScope, SchemaCatalog, ValidationRule,
    ValidationRuleCondition, ValidationRuleOperator, XmlFieldShape,
};
#[allow(unused_imports)]
pub(crate) use model::{ObjectTypeDiscriminator, ObjectTypeSchema};
// Only referenced by patch operation metadata tests today (see `schema_pack::tests::patch_operations`
// and `patches::tests::index`); a future preview-engine/custom-metadata-form consumer will use these
// via this facade too.
#[allow(unused_imports)]
pub(crate) use model::{PatchOperationFieldRole, PatchOperationPreview};

use crate::project_model::parse_major_minor;
use crate::schema_pack::model::SchemaLoadDiagnostic;
use std::path::PathBuf;

/// Collect all distinct major.minor game-version strings from installed schema packs.
///
/// Returns only values that parse as `Major.Minor`. Packs with no `gameVersion`
/// are not included. The list is sorted and deduplicated.
pub fn list_installed_schema_game_versions(extra_schema_roots: &[PathBuf]) -> Vec<String> {
    let (built_in, _) = load_built_in_packs();
    let (external, _) = load_external_packs(extra_schema_roots);

    let mut versions: Vec<String> = built_in
        .iter()
        .chain(external.iter())
        .filter_map(|p| p.manifest.game_version.as_deref())
        .filter(|v| parse_major_minor(v).is_some())
        .map(|v| v.to_string())
        .collect();

    versions.sort();
    versions.dedup();
    versions
}

/// Load built-in packs, discover and load external packs, optionally filter by game version,
/// merge everything, and return the catalog with all diagnostics.
///
/// If `game_version` is `Some`, packs whose `gameVersion` field does not match are
/// skipped with a warning diagnostic. Packs with no `gameVersion` are always included.
pub fn build_schema_catalog(
    extra_schema_roots: &[PathBuf],
    game_version: Option<&str>,
) -> SchemaCatalogLoadResult {
    let mut all_diags: Vec<SchemaLoadDiagnostic> = Vec::new();

    let (built_in_packs, built_in_diags) = load_built_in_packs();
    all_diags.extend(built_in_diags);

    let (external_packs, external_diags) = load_external_packs(extra_schema_roots);
    all_diags.extend(external_diags);

    let all_raw = built_in_packs.into_iter().chain(external_packs);

    let filtered = if let Some(selected) = game_version.and_then(parse_major_minor) {
        all_raw
            .filter(|pack| {
                match pack.manifest.game_version.as_deref().and_then(parse_major_minor) {
                    None => true, // no game version constraint → always compatible
                    Some(pack_ver) if pack_ver == selected => true,
                    Some(pack_ver) => {
                        all_diags.push(
                            SchemaLoadDiagnostic::warning(
                                "schema_pack_game_version_mismatch",
                                format!(
                                    "Schema pack '{}' targets game version {}.{} but selected version is {}.{}; skipping.",
                                    pack.manifest.pack_id, pack_ver.0, pack_ver.1, selected.0, selected.1
                                ),
                            )
                            .with_pack_id(&pack.manifest.pack_id),
                        );
                        false
                    }
                }
            })
            .collect::<Vec<_>>()
    } else {
        all_raw.collect::<Vec<_>>()
    };

    let catalog = merge_packs(filtered, &mut all_diags);

    SchemaCatalogLoadResult {
        catalog,
        diagnostics: all_diags,
    }
}
