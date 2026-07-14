mod cache;
mod loader;
mod lookup;
mod merge;
mod model;

#[cfg(test)]
mod tests;

pub(crate) use cache::SchemaCatalogCacheState;
use loader::{load_built_in_packs, load_external_packs, LoadedPack};
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

use crate::project_model::{parse_major_minor, ProjectSettings};
use crate::schema_pack::model::SchemaLoadDiagnostic;
use std::path::PathBuf;

/// Every registered location's root as a candidate external-schema-pack search root
/// (`schema_pack::loader` searches each supplied root, its `About/`, and its
/// `SchemaPacks/<name>/` for an embedded schema pack). This is the single, shared source of
/// "configured external schema roots" for the whole app -- there is no separate roots setting
/// anywhere in `ProjectSettings`/`RegisteredLocation`, so every `build_schema_catalog` call site
/// that needs a project's real catalog context (form rendering/`AppShell`, live document
/// validation, save-preview/final-save validation, project-wide validation, patch preview) should
/// call this rather than inventing its own roots list or passing an empty one (Plan.md section
/// 2/15's "catalog-context mismatch", issue 09's "avoid inventing a second registry").
pub fn schema_pack_roots(settings: &ProjectSettings) -> Vec<PathBuf> {
    settings
        .locations
        .iter()
        .map(|l| PathBuf::from(&l.root_path))
        .collect()
}

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

/// Filter `packs` by `game_version`, with a safety fallback for an unresolvable selection.
///
/// If `game_version` is `Some` AND at least one loaded pack (built-in or external) actually
/// declares that exact `gameVersion`, packs whose `gameVersion` does not match are skipped with
/// a warning diagnostic; packs with no `gameVersion` ("universal" packs) are always included.
///
/// If `game_version` is `Some` but NO loaded pack declares that version at all -- e.g. a
/// project's configured version is stale/orphaned because the external pack that used to
/// provide it was removed, or was hand-edited to something no installed pack targets -- naively
/// filtering would NOT produce an empty catalog: version-agnostic packs still pass the filter
/// unconditionally, so the result would silently narrow down to "only the packs with no declared
/// version" instead of behaving like "no filter". If a universal pack happens to define a field
/// differently than the (now dropped) versioned pack did, that silent narrowing can produce a
/// genuinely NEW validation diagnostic -- including a blocking one -- that never existed under
/// the true unfiltered catalog. This is caught by treating an unresolvable selection as
/// equivalent to `game_version: None` (return every pack, unfiltered) with an explanatory
/// diagnostic, rather than only checking whether the *result* happens to be empty (see issue 09
/// review round 2 and `schema_pack::tests::schema_mechanics`'s `unresolvable_game_version_*`
/// tests, which reproduce the conflicting-universal-pack scenario directly).
fn filter_packs_by_game_version(
    packs: Vec<LoadedPack>,
    game_version: Option<&str>,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> Vec<LoadedPack> {
    let Some(selected) = game_version.and_then(parse_major_minor) else {
        return packs;
    };

    let any_pack_declares_selected_version = packs.iter().any(|pack| {
        pack.manifest
            .game_version
            .as_deref()
            .and_then(parse_major_minor)
            == Some(selected)
    });
    if !any_pack_declares_selected_version {
        diags.push(SchemaLoadDiagnostic::warning(
            "schema_pack_game_version_unresolvable",
            format!(
                "No installed schema pack targets game version {}.{}; showing every installed pack instead of filtering by version.",
                selected.0, selected.1
            ),
        ));
        return packs;
    }

    packs
        .into_iter()
        .filter(|pack| {
            match pack.manifest.game_version.as_deref().and_then(parse_major_minor) {
                None => true, // no game version constraint → always compatible
                Some(pack_ver) if pack_ver == selected => true,
                Some(pack_ver) => {
                    diags.push(
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
        .collect()
}

/// Load built-in packs, discover and load external packs, optionally filter by game version,
/// merge everything, and return the catalog with all diagnostics.
///
/// See [`filter_packs_by_game_version`] for the exact filtering/fallback policy.
pub fn build_schema_catalog(
    extra_schema_roots: &[PathBuf],
    game_version: Option<&str>,
) -> SchemaCatalogLoadResult {
    let mut all_diags: Vec<SchemaLoadDiagnostic> = Vec::new();

    let (built_in_packs, built_in_diags) = load_built_in_packs();
    all_diags.extend(built_in_diags);

    let (external_packs, external_diags) = load_external_packs(extra_schema_roots);
    all_diags.extend(external_diags);

    let all_raw: Vec<LoadedPack> = built_in_packs.into_iter().chain(external_packs).collect();
    let filtered = filter_packs_by_game_version(all_raw, game_version, &mut all_diags);
    let catalog = merge_packs(filtered, &mut all_diags);

    SchemaCatalogLoadResult {
        catalog,
        diagnostics: all_diags,
    }
}
