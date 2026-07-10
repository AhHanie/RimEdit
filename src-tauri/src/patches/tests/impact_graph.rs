use std::collections::BTreeMap;
use std::fs;

use crate::patches::{
    build_patch_index, infer_xpath_target, PatchImpactGraph, PatchIndexBuildOptions, XPathTarget,
};
use crate::project_model::LocationKind;

use super::{location, settings_with_locations, temp_dir};

#[test]
fn xpath_target_struct_variants_serialize_fields_as_camel_case() {
    // Regression test: the enum-level `#[serde(rename_all = "camelCase")]` on `XPathTarget` only
    // renames the `kind` tag, not a struct variant's own fields -- each struct variant needs its
    // own `#[serde(rename_all = "camelCase")]` too, or fields like `def_type`/`def_name` leak onto
    // the wire as snake_case despite every Rust->TS type in this codebase otherwise being
    // camelCase (this previously crashed the frontend's `PatchPreviewDialog` reading
    // `target.defNames.length` on an actually-undefined field).
    let def = serde_json::to_value(XPathTarget::Def {
        def_type: "ThingDef".to_string(),
        def_name: "Wall".to_string(),
    })
    .unwrap();
    assert_eq!(def["defType"], "ThingDef");
    assert_eq!(def["defName"], "Wall");

    let def_type = serde_json::to_value(XPathTarget::DefType {
        def_type: "ThingDef".to_string(),
    })
    .unwrap();
    assert_eq!(def_type["defType"], "ThingDef");

    let defs = serde_json::to_value(XPathTarget::Defs {
        def_type: "ThingDef".to_string(),
        def_names: vec!["A".to_string(), "B".to_string()],
    })
    .unwrap();
    assert_eq!(defs["defType"], "ThingDef");
    assert_eq!(defs["defNames"][0], "A");
    assert_eq!(defs["defNames"][1], "B");
}

#[test]
fn infers_simple_def_name_xpath_target() {
    assert_eq!(
        infer_xpath_target(r#"Defs/ThingDef[defName="Wall"]/statBases"#),
        XPathTarget::Def {
            def_type: "ThingDef".to_string(),
            def_name: "Wall".to_string(),
        }
    );
    assert_eq!(
        infer_xpath_target(r#"/Defs/ThingDef[defName="Wall"]"#),
        XPathTarget::Def {
            def_type: "ThingDef".to_string(),
            def_name: "Wall".to_string(),
        }
    );
}

#[test]
fn infers_def_type_only_target_when_no_predicate() {
    assert_eq!(
        infer_xpath_target("Defs/ThingDef"),
        XPathTarget::DefType {
            def_type: "ThingDef".to_string(),
        }
    );
}

#[test]
fn complex_xpath_is_unsupported_instead_of_failing() {
    // A `@ParentName` predicate targets an inheritance template, not a concrete `defName`-keyed
    // Def, and multi-predicate/non-`Defs`-rooted paths are outside the conservative subset.
    assert_eq!(
        infer_xpath_target(r#"Defs/ThingDef[@ParentName="BaseThing"]"#),
        XPathTarget::Unsupported
    );
    assert_eq!(
        infer_xpath_target(r#"Defs/*[defName="Wall"]"#),
        XPathTarget::Unsupported
    );
    assert_eq!(
        infer_xpath_target(r#"//ThingDef[defName="Wall"]"#),
        XPathTarget::Unsupported
    );
}

#[test]
fn parent_axis_after_the_def_segment_is_unsupported_not_trusted() {
    // A `..` step can walk back out of the targeted Def and into a sibling; trusting the
    // `Defs/ThingDef[defName="Wall"]` prefix here would misreport the real target.
    assert_eq!(
        infer_xpath_target(r#"Defs/ThingDef[defName="Wall"]/../ThingDef[defName="Door"]"#),
        XPathTarget::Unsupported
    );
}

#[test]
fn infers_or_chained_def_name_xpath_target() {
    assert_eq!(
        infer_xpath_target(r#"Defs/ThingDef[defName="A" or defName="B" or defName="C"]"#),
        XPathTarget::Defs {
            def_type: "ThingDef".to_string(),
            def_names: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        }
    );
}

#[test]
fn or_chain_splitter_does_not_false_split_inside_a_defname_value() {
    // "MN_NetworkController" contains the substring "or" but not as a standalone token, so it
    // must not be mistaken for a separator.
    assert_eq!(
        infer_xpath_target(
            r#"Defs/ThingDef[defName="MN_NetworkController" or defName="MN_NetworkCable"]"#
        ),
        XPathTarget::Defs {
            def_type: "ThingDef".to_string(),
            def_names: vec![
                "MN_NetworkController".to_string(),
                "MN_NetworkCable".to_string()
            ],
        }
    );
}

#[test]
fn and_combined_def_name_predicate_stays_unsupported() {
    assert_eq!(
        infer_xpath_target(r#"Defs/ThingDef[defName="A" and defName="B"]"#),
        XPathTarget::Unsupported
    );
}

#[test]
fn or_chain_mixed_with_a_non_defname_term_stays_unsupported() {
    assert_eq!(
        infer_xpath_target(r#"Defs/ThingDef[defName="A" or @ParentName="B"]"#),
        XPathTarget::Unsupported
    );
}

#[test]
fn or_chain_still_resolves_with_a_further_child_path_segment() {
    // Real-world fixture (Matter Network's PowerPlusPlus.xml patch): a further child segment
    // after the OR-chained predicate must be ignored for classification purposes, same as the
    // single-defName case.
    assert_eq!(
        infer_xpath_target(
            r#"/Defs/ThingDef[defName="MN_NetworkController" or defName="MN_NetworkControllerLarge"]/comps[not(li[@Class="aRandomKiwi.PPP.CompProperties_LocalWirelessPowerReceptor"])]"#
        ),
        XPathTarget::Defs {
            def_type: "ThingDef".to_string(),
            def_names: vec![
                "MN_NetworkController".to_string(),
                "MN_NetworkControllerLarge".to_string()
            ],
        }
    );
}

#[test]
fn or_chain_resolves_with_many_terms() {
    // Real-world fixture (Matter Network's SaveOurShip2.xml patch): an 11-term OR-chain.
    assert_eq!(
        infer_xpath_target(
            r#"/Defs/ThingDef[defName="MN_NetworkCable" or defName="MN_DiskDrive" or defName="MN_NetworkInterface" or defName="MN_NetworkChute" or defName="MN_NetworkController" or defName="MN_NetworkControllerLarge" or defName="MN_MatterIOPort" or defName="MN_NetworkPowerStorage" or defName="MN_AdvancedNetworkPowerStorage" or defName="MN_NetworkRefueler" or defName="MN_AdvancedNetworkRefueler"]"#
        ),
        XPathTarget::Defs {
            def_type: "ThingDef".to_string(),
            def_names: vec![
                "MN_NetworkCable".to_string(),
                "MN_DiskDrive".to_string(),
                "MN_NetworkInterface".to_string(),
                "MN_NetworkChute".to_string(),
                "MN_NetworkController".to_string(),
                "MN_NetworkControllerLarge".to_string(),
                "MN_MatterIOPort".to_string(),
                "MN_NetworkPowerStorage".to_string(),
                "MN_AdvancedNetworkPowerStorage".to_string(),
                "MN_NetworkRefueler".to_string(),
                "MN_AdvancedNetworkRefueler".to_string(),
            ],
        }
    );
}

#[test]
fn impact_graph_maps_or_chained_xpath_to_each_named_def() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationAdd">
            <xpath>Defs/ThingDef[defName="A" or defName="B"]</xpath>
            <value><modExtensions /></value>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");
    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    let graph = PatchImpactGraph::build(&index);

    assert_eq!(graph.operations_affecting_def("ThingDef", "A").len(), 1);
    assert_eq!(graph.operations_affecting_def("ThingDef", "B").len(), 1);
    assert!(graph.operations_affecting_def("ThingDef", "C").is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn impact_graph_maps_simple_xpath_to_def() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationAdd">
            <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
            <value><MaxHitPoints>300</MaxHitPoints></value>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");
    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    let graph = PatchImpactGraph::build(&index);

    let matches = graph.operations_affecting_def("ThingDef", "Wall");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].relative_path, "Patches/a.xml");
    assert!(graph
        .operations_affecting_def("ThingDef", "Steel")
        .is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn impact_graph_indexes_complex_xpath_as_unsupported_instead_of_dropping() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationRemove">
            <xpath>Defs/ThingDef[@ParentName="BaseThing"]</xpath>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");
    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    assert_eq!(
        index.files[0].operations.len(),
        1,
        "operation must still be indexed"
    );
    let graph = PatchImpactGraph::build(&index);

    assert!(graph.operations_affecting_def_type("ThingDef").is_empty());
    assert_eq!(graph.unsupported_operations().len(), 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn def_type_only_match_affects_every_concrete_def_of_that_type() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationAdd">
            <xpath>Defs/ThingDef</xpath>
            <value><modExtensions /></value>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");
    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    let graph = PatchImpactGraph::build(&index);

    // A type-wide operation (no `defName` predicate) applies to every ThingDef, so it must show
    // up for any specific ThingDef's preview controls, not just the type-level query.
    assert_eq!(graph.operations_affecting_def_type("ThingDef").len(), 1);
    assert_eq!(graph.operations_affecting_def("ThingDef", "Wall").len(), 1);
    assert_eq!(graph.operations_affecting_def("ThingDef", "Steel").len(), 1);
    // But it must not leak into an unrelated Def type.
    assert!(graph
        .operations_affecting_def("RecipeDef", "Wall")
        .is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn conflicts_involving_def_requires_at_least_two_operations() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationRemove">
            <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
          </Operation>
          <Operation Class="PatchOperationReplace">
            <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
            <value><statBases /></value>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");
    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    let graph = PatchImpactGraph::build(&index);

    assert_eq!(graph.conflicts_involving_def("ThingDef", "Wall").len(), 2);
    assert!(graph
        .conflicts_involving_def("ThingDef", "Steel")
        .is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn patch_files_affecting_def_dedupes_by_file() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationRemove">
            <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
          </Operation>
          <Operation Class="PatchOperationAttributeRemove">
            <xpath>Defs/ThingDef[defName="Wall"]</xpath>
            <attribute>ParentName</attribute>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");
    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    let graph = PatchImpactGraph::build(&index);

    assert_eq!(
        graph.patch_files_affecting_def("ThingDef", "Wall"),
        vec![("project".to_string(), "Patches/a.xml".to_string())]
    );
    fs::remove_dir_all(&root).ok();
}
