pub use super::diagnostics::DiagnosticSeverity;
pub use super::edit::{InitialElement, KeyValuePair, NameValuePair};
pub use super::model::XmlChildShape;
pub use super::{
    apply_xml_edit, build_editor_view, parse_to_document, parse_xml_document,
    serialize_xml_document, validate_document, ValidationContext, ValidationDiagnostic, XmlEdit,
    XmlEditContext, XmlEditError,
};
pub use crate::def_index::{
    DefIdentityKey, DefIndex, IndexedDef, IndexedDefSource, IndexedSourceKind,
};
pub use crate::project_model::SourceType;
pub use crate::schema_pack::build_schema_catalog;

pub const THING_DEFS_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/thing_defs_with_unknowns.xml");

pub const FIXTURE_WEAPON_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/thingdef_weapon_sample.xml");

pub const GRAPHIC_DATA_009_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_009_shapes.xml");

pub const SINGLE_DEF_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/single_top_level_def.xml");

pub const CDATA_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/cdata_and_attributes.xml");

pub const MALFORMED_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/malformed_missing_end_tag.xml");

pub const FIXTURE_SINGLE_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_single.xml");

pub const FIXTURE_MULTI_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_multi.xml");

pub const FIXTURE_RANDOM_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_random.xml");

pub const FIXTURE_STACK_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_stack_count.xml");

pub const FIXTURE_NESTED_FULL_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_nested_full.xml");

pub const FIXTURE_MISSING_TEXTURE_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_missing_texture.xml");

pub const FIXTURE_UNKNOWN_CLASS_XML: &str =
    include_str!("../../../tests/fixtures/xml_document/graphic_data_unknown_class.xml");

pub fn validate_test_xml(src: &str, def_index: &DefIndex) -> Vec<ValidationDiagnostic> {
    let catalog_result = build_schema_catalog(&[], None);
    let doc = parse_to_document("test.xml", src);
    let context = ValidationContext {
        catalog: &catalog_result.catalog,
        def_index,
    };
    validate_document(&doc, &context)
}

pub fn empty_def_index() -> DefIndex {
    DefIndex::default()
}

pub fn indexed_test_def(relative_path: &str, source_kind: IndexedSourceKind) -> IndexedDef {
    let read_only = source_kind == IndexedSourceKind::Source;
    IndexedDef {
        key: DefIdentityKey {
            def_type: "ThingDef".to_string(),
            def_name: "Steel".to_string(),
        },
        def_type: "ThingDef".to_string(),
        def_name: "Steel".to_string(),
        label: None,
        parent_name: None,
        relative_path: relative_path.to_string(),
        node_id: Some(1),
        line: Some(1),
        column: Some(7),
        source: IndexedDefSource {
            location_id: if read_only { "source" } else { "project" }.to_string(),
            location_name: if read_only { "Source" } else { "Project" }.to_string(),
            source_kind,
            source_type: SourceType::Folder,
            read_only,
            mod_id: None,
            game_version: None,
            expansion_name: None,
        },
        fields: vec![],
        def_name_lower: String::new(),
        label_lower: String::new(),
    }
}

pub fn skill_def_index(def_name: &str) -> DefIndex {
    let mut skill_def = indexed_test_def("skills.xml", IndexedSourceKind::Source);
    skill_def.key.def_type = "SkillDef".to_string();
    skill_def.key.def_name = def_name.to_string();
    skill_def.def_type = "SkillDef".to_string();
    skill_def.def_name = def_name.to_string();
    DefIndex {
        defs: vec![skill_def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    }
}

pub fn make_typed_ref_index(def_type: &str, def_name: &str) -> DefIndex {
    let mut def = indexed_test_def("source.xml", IndexedSourceKind::Source);
    def.key.def_type = def_type.to_string();
    def.key.def_name = def_name.to_string();
    def.def_type = def_type.to_string();
    def.def_name = def_name.to_string();
    DefIndex {
        defs: vec![def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    }
}

pub fn validate_test_xml_with_fixture(
    src: &str,
    fixture_name: &str,
    def_index: &DefIndex,
) -> Vec<ValidationDiagnostic> {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack")
        .join(fixture_name);
    let catalog_result = build_schema_catalog(&[fixture_path], None);
    let doc = parse_to_document("test.xml", src);
    let context = ValidationContext {
        catalog: &catalog_result.catalog,
        def_index,
    };
    validate_document(&doc, &context)
}

mod edit_basic;
mod edit_lists;
mod edit_maps;
mod edit_nested;
mod edit_typed_references;
mod editor_view;
mod form_views_visibility_regression;
mod nested_edits;
mod parsing;
mod profile;
mod validation_accepted_def_types;
mod validation_color;
mod validation_conditional_rules;
mod validation_core;
mod validation_fixtures;
mod validation_keyed_object_map;
mod validation_keyed_value_defaults;
mod validation_references;
mod validation_scalar_list;
mod validation_shapes;
mod validation_sound_def;
