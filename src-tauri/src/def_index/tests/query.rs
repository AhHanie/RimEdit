use crate::def_index::{
    get_facet_summary, resolve_def_reference, search_def_results, suggest_def_references,
    DefIdentityKey, DefIndex, DefIndexSearchQuery, DefReferenceResolution, IndexedDef,
    IndexedDefSource, IndexedSourceKind,
};
use crate::project_model::SourceType;
use crate::schema_pack::ReferenceScope;

fn make_search_fixture_source(source_kind: IndexedSourceKind) -> IndexedDefSource {
    let location_name = match source_kind {
        IndexedSourceKind::Project => "My Mod".to_string(),
        IndexedSourceKind::Source => "Core".to_string(),
    };
    IndexedDefSource {
        location_id: "loc1".to_string(),
        location_name,
        source_kind,
        source_type: SourceType::Folder,
        read_only: false,
        mod_id: None,
        game_version: None,
        expansion_name: None,
    }
}

fn make_def(
    def_type: &str,
    def_name: &str,
    label: Option<&str>,
    relative_path: &str,
    source_kind: IndexedSourceKind,
) -> IndexedDef {
    IndexedDef {
        key: DefIdentityKey {
            def_type: def_type.to_string(),
            def_name: def_name.to_string(),
        },
        def_type: def_type.to_string(),
        def_name: def_name.to_string(),
        label: label.map(|s| s.to_string()),
        parent_name: None,
        relative_path: relative_path.to_string(),
        node_id: None,
        line: None,
        column: None,
        source: make_search_fixture_source(source_kind),
        fields: Vec::new(),
        def_name_lower: String::new(),
        label_lower: String::new(),
    }
}

fn make_index(defs: Vec<IndexedDef>) -> DefIndex {
    let mut index = DefIndex {
        defs,
        errors: Vec::new(),
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    index.rebuild_computed_fields();
    index
}

fn search_names(index: &DefIndex, query: &str) -> Vec<String> {
    let q = DefIndexSearchQuery {
        query: query.to_string(),
        def_type: None,
        include_sources: true,
        limit: None,
    };
    search_def_results(index, &q)
        .into_iter()
        .map(|r| r.def.def_name)
        .collect()
}

#[test]
fn test_exact_before_prefix_before_partial() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "SuperSteel",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "Steel",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "SteelBase",
            None,
            "c.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results = search_names(&index, "Steel");
    assert_eq!(results[0], "Steel");
    assert_eq!(results[1], "SteelBase");
    assert_eq!(results[2], "SuperSteel");
}

#[test]
fn test_matches_label() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            Some("Iron Bar"),
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "D002",
            Some("Steel Bar"),
            "b.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results = search_names(&index, "steel bar");
    assert!(results.contains(&"D002".to_string()));
    assert!(!results.contains(&"D001".to_string()));
}

#[test]
fn test_label_exact_before_prefix_before_partial() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "A",
            Some("Cast Iron"),
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "B",
            Some("Iron"),
            "b.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "C",
            Some("Ironworks"),
            "c.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results = search_names(&index, "iron");
    assert_eq!(results[0], "B"); // exact label
    assert_eq!(results[1], "C"); // prefix label
    assert_eq!(results[2], "A"); // partial label
}

#[test]
fn test_matches_relative_path() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "Items/Steel.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "D002",
            None,
            "Items/Iron.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results = search_names(&index, "steel.xml");
    assert!(results.contains(&"D001".to_string()));
    assert!(!results.contains(&"D002".to_string()));
}

#[test]
fn test_matches_def_type() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "RecipeDef",
            "D002",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results = search_names(&index, "recipedef");
    assert!(results.contains(&"D002".to_string()));
    assert!(!results.contains(&"D001".to_string()));
}

#[test]
fn test_def_type_filter() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "RecipeDef",
            "D002",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let q = DefIndexSearchQuery {
        query: String::new(),
        def_type: Some("ThingDef".to_string()),
        include_sources: true,
        limit: None,
    };
    let names: Vec<_> = search_def_results(&index, &q)
        .into_iter()
        .map(|r| r.def.def_name)
        .collect();
    assert_eq!(names, vec!["D001"]);
}

#[test]
fn test_empty_query_respects_def_type_filter() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "RecipeDef",
            "D002",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let q = DefIndexSearchQuery {
        query: String::new(),
        def_type: Some("RecipeDef".to_string()),
        include_sources: true,
        limit: None,
    };
    let names: Vec<_> = search_def_results(&index, &q)
        .into_iter()
        .map(|r| r.def.def_name)
        .collect();
    assert_eq!(names, vec!["D002"]);
}

#[test]
fn test_exclude_sources() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def("ThingDef", "D002", None, "b.xml", IndexedSourceKind::Source),
    ]);
    let q = DefIndexSearchQuery {
        query: String::new(),
        def_type: None,
        include_sources: false,
        limit: None,
    };
    let names: Vec<_> = search_def_results(&index, &q)
        .into_iter()
        .map(|r| r.def.def_name)
        .collect();
    assert_eq!(names, vec!["D001"]);
}

#[test]
fn test_project_before_source_tiebreaker() {
    let mut source_def = make_def(
        "ThingDef",
        "Steel",
        None,
        "a.xml",
        IndexedSourceKind::Source,
    );
    source_def.source.location_name = "Core".to_string();
    let project_def = make_def(
        "ThingDef",
        "Steel",
        None,
        "a.xml",
        IndexedSourceKind::Project,
    );
    let index = make_index(vec![source_def, project_def]);
    let q = DefIndexSearchQuery {
        query: "Steel".to_string(),
        def_type: None,
        include_sources: true,
        limit: None,
    };
    let results = search_def_results(&index, &q);
    assert_eq!(
        results[0].def.source.source_kind,
        IndexedSourceKind::Project
    );
    assert_eq!(results[1].def.source.source_kind, IndexedSourceKind::Source);
}

#[test]
fn test_project_source_distinguishable() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def("ThingDef", "D002", None, "b.xml", IndexedSourceKind::Source),
    ]);
    let q = DefIndexSearchQuery {
        query: String::new(),
        def_type: None,
        include_sources: true,
        limit: None,
    };
    let results = search_def_results(&index, &q);
    let project_result = results.iter().find(|r| r.def.def_name == "D001").unwrap();
    let source_result = results.iter().find(|r| r.def.def_name == "D002").unwrap();
    assert_eq!(
        project_result.def.source.source_kind,
        IndexedSourceKind::Project
    );
    assert!(!project_result.def.source.read_only);
    assert_eq!(
        source_result.def.source.source_kind,
        IndexedSourceKind::Source
    );
}

#[test]
fn test_facet_summary_counts() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def("ThingDef", "D002", None, "b.xml", IndexedSourceKind::Source),
        make_def(
            "RecipeDef",
            "D003",
            None,
            "c.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let facets = get_facet_summary(&index, true);
    assert_eq!(facets.project_defs, 2);
    assert_eq!(facets.source_defs, 1);
    assert_eq!(facets.def_types.len(), 2);
    let thing = facets
        .def_types
        .iter()
        .find(|f| f.def_type == "ThingDef")
        .unwrap();
    assert_eq!(thing.project_count, 1);
    assert_eq!(thing.source_count, 1);
    assert_eq!(thing.total_count, 2);
    let recipe = facets
        .def_types
        .iter()
        .find(|f| f.def_type == "RecipeDef")
        .unwrap();
    assert_eq!(recipe.project_count, 1);
    assert_eq!(recipe.source_count, 0);
    assert_eq!(recipe.total_count, 1);
}

#[test]
fn test_facet_summary_exclude_sources() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "D001",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def("ThingDef", "D002", None, "b.xml", IndexedSourceKind::Source),
    ]);
    let facets = get_facet_summary(&index, false);
    assert_eq!(facets.project_defs, 1);
    assert_eq!(facets.source_defs, 0);
    assert_eq!(facets.def_types.len(), 1);
    assert_eq!(facets.def_types[0].project_count, 1);
    assert_eq!(facets.def_types[0].source_count, 0);
}

fn make_readonly_source_def(
    def_type: &str,
    def_name: &str,
    relative_path: &str,
    location_id: &str,
) -> IndexedDef {
    let mut def = make_def(
        def_type,
        def_name,
        None,
        relative_path,
        IndexedSourceKind::Source,
    );
    def.source.read_only = true;
    def.source.location_id = location_id.to_string();
    def
}

// --- suggest_def_references ---

#[test]
fn suggest_filters_to_target_def_types() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "Steel",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "RecipeDef",
            "MakeSomething",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results =
        suggest_def_references(&index, &["RecipeDef"], "", &ReferenceScope::AllSources, 20);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].def_name, "MakeSomething");
    assert_eq!(results[0].def_type, "RecipeDef");
}

#[test]
fn suggest_ranks_exact_before_prefix_before_partial() {
    let index = make_index(vec![
        make_def(
            "StatDef",
            "SuperGeneral",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "StatDef",
            "General",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "StatDef",
            "GeneralSpeed",
            None,
            "c.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let results = suggest_def_references(
        &index,
        &["StatDef"],
        "general",
        &ReferenceScope::AllSources,
        20,
    );
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].def_name, "General"); // exact
    assert_eq!(results[1].def_name, "GeneralSpeed"); // prefix
    assert_eq!(results[2].def_name, "SuperGeneral"); // partial
}

#[test]
fn suggest_project_defs_ranked_before_source_at_same_tier() {
    let mut source_def = make_def(
        "StatDef",
        "GeneralLaborSpeed",
        None,
        "b.xml",
        IndexedSourceKind::Source,
    );
    source_def.source.read_only = true;
    let project_def = make_def(
        "StatDef",
        "GeneralLaborSpeed",
        None,
        "a.xml",
        IndexedSourceKind::Project,
    );
    let index = make_index(vec![source_def, project_def]);
    let results = suggest_def_references(
        &index,
        &["StatDef"],
        "GeneralLaborSpeed",
        &ReferenceScope::AllSources,
        20,
    );
    assert_eq!(results.len(), 2);
    assert!(!results[0].read_only, "project def should come first");
    assert!(results[1].read_only, "source def should come second");
}

#[test]
fn suggest_project_only_scope_excludes_readonly_source_defs() {
    let source_def = make_readonly_source_def("StatDef", "CoreStat", "b.xml", "loc_core");
    let project_def = make_def(
        "StatDef",
        "MyStat",
        None,
        "a.xml",
        IndexedSourceKind::Project,
    );
    let index = make_index(vec![source_def, project_def]);
    let results =
        suggest_def_references(&index, &["StatDef"], "", &ReferenceScope::ProjectOnly, 20);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].def_name, "MyStat");
}

#[test]
fn suggest_respects_limit() {
    let index = make_index(vec![
        make_def("StatDef", "A", None, "a.xml", IndexedSourceKind::Project),
        make_def("StatDef", "B", None, "b.xml", IndexedSourceKind::Project),
        make_def("StatDef", "C", None, "c.xml", IndexedSourceKind::Project),
    ]);
    let results = suggest_def_references(&index, &["StatDef"], "", &ReferenceScope::AllSources, 2);
    assert_eq!(results.len(), 2);
}

// --- resolve_def_reference ---

#[test]
fn resolve_returns_editable_project_def() {
    let index = make_index(vec![make_def(
        "ThingDef",
        "Steel",
        None,
        "things.xml",
        IndexedSourceKind::Project,
    )]);
    let result = resolve_def_reference(&index, &["ThingDef"], "Steel", &ReferenceScope::AllSources);
    assert!(
        matches!(result, DefReferenceResolution::EditableProjectDef { ref relative_path, .. } if relative_path == "things.xml"),
        "expected EditableProjectDef"
    );
}

#[test]
fn resolve_returns_readonly_source_def() {
    let source_def = make_readonly_source_def("ThingDef", "Steel", "things.xml", "loc_core");
    let index = make_index(vec![source_def]);
    let result = resolve_def_reference(&index, &["ThingDef"], "Steel", &ReferenceScope::AllSources);
    assert!(
        matches!(
            result,
            DefReferenceResolution::ReadOnlySourceDef { ref location_id, ref relative_path, .. }
            if location_id == "loc_core" && relative_path == "things.xml"
        ),
        "expected ReadOnlySourceDef"
    );
}

#[test]
fn resolve_returns_missing_for_no_match() {
    let index = make_index(vec![make_def(
        "ThingDef",
        "Steel",
        None,
        "a.xml",
        IndexedSourceKind::Project,
    )]);
    let result = resolve_def_reference(
        &index,
        &["ThingDef"],
        "NoSuchDef",
        &ReferenceScope::AllSources,
    );
    assert!(matches!(result, DefReferenceResolution::Missing));
}

#[test]
fn resolve_returns_ambiguous_for_multiple_project_matches() {
    let index = make_index(vec![
        make_def(
            "ThingDef",
            "Steel",
            None,
            "a.xml",
            IndexedSourceKind::Project,
        ),
        make_def(
            "ThingDef",
            "Steel",
            None,
            "b.xml",
            IndexedSourceKind::Project,
        ),
    ]);
    let result = resolve_def_reference(&index, &["ThingDef"], "Steel", &ReferenceScope::AllSources);
    assert!(matches!(result, DefReferenceResolution::Ambiguous));
}

#[test]
fn resolve_returns_ambiguous_for_multiple_source_matches() {
    let def1 = make_readonly_source_def("ThingDef", "Steel", "a.xml", "loc1");
    let def2 = make_readonly_source_def("ThingDef", "Steel", "b.xml", "loc2");
    let index = make_index(vec![def1, def2]);
    let result = resolve_def_reference(&index, &["ThingDef"], "Steel", &ReferenceScope::AllSources);
    assert!(matches!(result, DefReferenceResolution::Ambiguous));
}

#[test]
fn resolve_project_only_scope_ignores_source_def() {
    let source_def = make_readonly_source_def("ThingDef", "Steel", "things.xml", "loc_core");
    let index = make_index(vec![source_def]);
    let result =
        resolve_def_reference(&index, &["ThingDef"], "Steel", &ReferenceScope::ProjectOnly);
    assert!(
        matches!(result, DefReferenceResolution::Missing),
        "ProjectOnly scope should exclude source defs"
    );
}

#[test]
fn resolve_prefers_project_def_over_source_def() {
    let source_def = make_readonly_source_def("ThingDef", "Steel", "source.xml", "loc_core");
    let project_def = make_def(
        "ThingDef",
        "Steel",
        None,
        "project.xml",
        IndexedSourceKind::Project,
    );
    let index = make_index(vec![source_def, project_def]);
    let result = resolve_def_reference(&index, &["ThingDef"], "Steel", &ReferenceScope::AllSources);
    assert!(
        matches!(result, DefReferenceResolution::EditableProjectDef { ref relative_path, .. } if relative_path == "project.xml"),
        "project def should be preferred over source def"
    );
}

#[test]
fn resolve_searches_across_multiple_target_def_types() {
    let recipe_def = make_def(
        "RecipeDef",
        "MakeSomething",
        None,
        "recipes.xml",
        IndexedSourceKind::Project,
    );
    let index = make_index(vec![recipe_def]);
    let result = resolve_def_reference(
        &index,
        &["ThingDef", "RecipeDef"],
        "MakeSomething",
        &ReferenceScope::AllSources,
    );
    assert!(matches!(
        result,
        DefReferenceResolution::EditableProjectDef { .. }
    ));
}
