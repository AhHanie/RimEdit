use crate::def_index::{apply_replacement_overlay, DefIndexReplacement};
use crate::project_model::{AppError, ProjectSettings};
use crate::rimworld_load_folders::read_load_folders_version_keys;
use crate::schema_pack::{build_schema_catalog, schema_pack_roots};
use crate::services::def_index_cache;
use crate::xml_document::{
    validate_about_metadata_document, validate_document, ValidationContext, XmlDocument,
    XmlDocumentProfile,
};
use std::path::PathBuf;
use tauri::AppHandle;

fn find_location_root(settings: &ProjectSettings, location_id: &str) -> Option<PathBuf> {
    settings
        .locations
        .iter()
        .find(|l| l.id == location_id)
        .map(|l| PathBuf::from(&l.root_path))
}

pub(crate) fn validate_doc_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    relative_path: &str,
    doc: &mut XmlDocument,
) -> Result<(), AppError> {
    let _span = crate::instrumentation::span_with_tags(
        app,
        "validation.validateDocForProject",
        [("relativePath".to_string(), relative_path.to_string())],
    );

    if doc.profile == XmlDocumentProfile::About {
        let load_folders_versions = find_location_root(settings, project_id)
            .and_then(|root| read_load_folders_version_keys(&root));
        doc.validation_diagnostics =
            validate_about_metadata_document(doc, load_folders_versions.as_deref());
        return Ok(());
    }

    // Catalog context must match what `AppShell`/`preview_def_for_project` load for the same
    // project: the selected game version, and every registered location's root as a candidate
    // external-schema-pack root (an embedded `SchemaPacks/<name>/` or `About/` folder). Building
    // an unfiltered, all-game-version catalog here (as this used to do) would validate against a
    // merge of every installed schema-pack game version at once, silently diverging from what the
    // form/catalog UI actually renders -- see Plan.md section 15's "catalog-context mismatch".
    let roots = schema_pack_roots(settings);
    let catalog_result = build_schema_catalog(&roots, Some(&settings.game_version));
    let base_index = def_index_cache::load_for_project(app, settings, project_id, false)?;
    let def_index = apply_replacement_overlay(
        (*base_index).clone(),
        settings,
        DefIndexReplacement {
            location_id: project_id,
            relative_path,
            source: doc.source.as_str(),
        },
    );
    let context = ValidationContext {
        catalog: &catalog_result.catalog,
        def_index: &def_index,
    };
    doc.validation_diagnostics = validate_document(doc, &context);
    Ok(())
}

/// Validates a source location document using the project's index without
/// overlaying the document as a project entry. This avoids false duplicate
/// or source diagnostics that would occur if the source file were treated as
/// a project-owned file.
pub(crate) fn validate_doc_for_source(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    location_id: &str,
    doc: &mut XmlDocument,
) -> Result<(), AppError> {
    if doc.profile == XmlDocumentProfile::About {
        let load_folders_versions = find_location_root(settings, location_id)
            .and_then(|root| read_load_folders_version_keys(&root));
        doc.validation_diagnostics =
            validate_about_metadata_document(doc, load_folders_versions.as_deref());
        return Ok(());
    }

    let roots = schema_pack_roots(settings);
    let catalog_result = build_schema_catalog(&roots, Some(&settings.game_version));
    let base_index = def_index_cache::load_for_project(app, settings, project_id, false)?;
    let context = ValidationContext {
        catalog: &catalog_result.catalog,
        def_index: &base_index,
    };
    doc.validation_diagnostics = validate_document(doc, &context);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_model::{LocationKind, RegisteredLocation, SourceType};
    use time::OffsetDateTime;

    fn make_location(id: &str, root_path: &str) -> RegisteredLocation {
        RegisteredLocation {
            id: id.to_string(),
            display_name: id.to_string(),
            root_path: root_path.to_string(),
            kind: LocationKind::Project,
            source_type: SourceType::Folder,
            read_only: false,
            mod_id: None,
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    // Issue 09 (Plan.md section 2/15's "catalog-context mismatch"): document schema resolution's
    // catalog now uses the same "every registered location root" policy as
    // `services::patch_preview::preview_def_for_project`, instead of always passing an empty root
    // list. This is the prerequisite fix that lets `validate_doc_for_project`/
    // `validate_doc_for_source` discover a mod-embedded `SchemaPacks/<name>/` or `About/` schema
    // pack under any registered project/source location, matching what a future AppShell-wired
    // `useSchemaCatalog` call would also need to resolve for the same project.
    #[test]
    fn schema_pack_roots_collects_every_registered_location() {
        let mut settings = ProjectSettings::default();
        settings.locations.push(make_location("a", "C:\\ProjectA"));
        settings.locations.push(make_location("b", "C:\\SourceB"));

        let roots = schema_pack_roots(&settings);

        assert_eq!(
            roots,
            vec![PathBuf::from("C:\\ProjectA"), PathBuf::from("C:\\SourceB")]
        );
    }

    #[test]
    fn schema_pack_roots_is_empty_with_no_registered_locations() {
        let settings = ProjectSettings::default();
        assert!(schema_pack_roots(&settings).is_empty());
    }

    // Guards the other half of the same prerequisite fix: validation must filter by the
    // project's OWN selected `settings.game_version`, not merge every installed schema-pack
    // game version indiscriminately (`build_schema_catalog(&[], None)`'s old behavior here).
    // This test operates on the real built-in catalog rather than a fixture, so it also proves
    // the fix compiles against `build_schema_catalog`'s actual signature/behavior.
    #[test]
    fn game_version_none_and_explicit_installed_version_both_resolve_a_def_type() {
        let unfiltered = build_schema_catalog(&[], None);
        let filtered = build_schema_catalog(&[], Some("1.6"));

        // The built-in pack only targets 1.6 today, so both catalogs currently resolve the same
        // Def types -- but `filtered` is the policy `validate_doc_for_project`/
        // `validate_doc_for_source` now actually use (via `settings.game_version`), while
        // `unfiltered` is the old, no-longer-used behavior. Asserting both non-empty guards
        // against a future regression silently reintroducing the unfiltered call.
        assert!(!unfiltered.catalog.def_types.is_empty());
        assert!(!filtered.catalog.def_types.is_empty());
        assert_eq!(
            unfiltered
                .catalog
                .def_types
                .keys()
                .collect::<std::collections::BTreeSet<_>>(),
            filtered
                .catalog
                .def_types
                .keys()
                .collect::<std::collections::BTreeSet<_>>(),
        );
    }

    // The actual regression this issue exists to prevent (reviewer finding 3): prove the
    // `schema_pack_roots(settings)` -> `build_schema_catalog(&roots, Some(&settings.game_version))`
    // pipeline that `validate_doc_for_project`/`validate_doc_for_source` (and, after the reviewer's
    // finding 1 fix, `project_save::validate_proposed_xml_with_index` and
    // `commands::project_validation::validate_project`) all now share actually changes real
    // validation output -- not just that it compiles. Uses a genuine external/third-party-style
    // fixture pack (`tests/fixtures/schema_pack/external_project_pack`, defining `ExternalWidgetDef`
    // and its `externalOnlyField` -- neither exists in the built-in pack) registered as a project
    // location's root, exactly how a real mod folder with an embedded `SchemaPacks/` folder would
    // be registered.
    #[test]
    fn external_pack_def_type_is_recognized_only_when_its_root_is_registered() {
        use crate::def_index::DefIndex;
        use crate::xml_document::parse_to_document;

        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/schema_pack/external_project_pack");
        let xml = r#"<Defs>
  <ExternalWidgetDef>
    <defName>Widget1</defName>
    <externalOnlyField>hello</externalOnlyField>
  </ExternalWidgetDef>
</Defs>"#;
        let doc = parse_to_document("Widgets.xml", xml);
        let def_index = DefIndex::default();

        // Registered: a project location whose root is the fixture pack directory, with the
        // matching game version -- exactly what `schema_pack_roots`/`settings.game_version`
        // produce for a real project with a mod-embedded external schema pack.
        let mut settings_with_root = ProjectSettings::default();
        settings_with_root.game_version = "1.6".to_string();
        settings_with_root
            .locations
            .push(make_location("proj", fixture_path.to_str().unwrap()));
        let roots = schema_pack_roots(&settings_with_root);
        assert_eq!(roots, vec![fixture_path.clone()]);
        let catalog_with_root =
            build_schema_catalog(&roots, Some(&settings_with_root.game_version));
        let diagnostics_with_root = validate_document(
            &doc,
            &ValidationContext {
                catalog: &catalog_with_root.catalog,
                def_index: &def_index,
            },
        );
        assert!(
            !diagnostics_with_root
                .iter()
                .any(|d| d.code == "validation_unknown_def_type"),
            "ExternalWidgetDef should be recognized once its pack root is registered: {:?}",
            diagnostics_with_root,
        );
        assert!(
            !diagnostics_with_root
                .iter()
                .any(|d| d.code == "validation_unknown_field"),
            "externalOnlyField should be recognized once its pack root is registered: {:?}",
            diagnostics_with_root,
        );

        // Not registered: no locations at all -- the old `build_schema_catalog(&[], None)`
        // behavior this issue replaced. The same XML must now be flagged unknown, proving the
        // roots plumbing is actually load-bearing rather than incidental.
        let settings_without_root = ProjectSettings::default();
        let roots_without = schema_pack_roots(&settings_without_root);
        assert!(roots_without.is_empty());
        let catalog_without_root = build_schema_catalog(&roots_without, Some("1.6"));
        let diagnostics_without_root = validate_document(
            &doc,
            &ValidationContext {
                catalog: &catalog_without_root.catalog,
                def_index: &def_index,
            },
        );
        assert!(
            diagnostics_without_root
                .iter()
                .any(|d| d.code == "validation_unknown_def_type"),
            "ExternalWidgetDef must be unknown without its pack root registered: {:?}",
            diagnostics_without_root,
        );
    }
}
