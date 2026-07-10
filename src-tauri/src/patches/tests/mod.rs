pub use super::model::*;
pub use super::{parse_patch_file, serialize_patch_file};

use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

pub(super) fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rimedit_patch_index_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn location(root: &Path, id: &str, kind: LocationKind) -> RegisteredLocation {
    RegisteredLocation {
        id: id.to_string(),
        display_name: id.to_string(),
        root_path: root.to_string_lossy().to_string(),
        kind: kind.clone(),
        source_type: SourceType::Folder,
        read_only: kind == LocationKind::Source,
        mod_id: Some(format!("mod-{}", id)),
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

pub(super) fn settings_with_locations(
    locations: Vec<RegisteredLocation>,
    active: &str,
) -> ProjectSettings {
    ProjectSettings {
        schema_version: 2,
        game_version: "1.6".to_string(),
        locations,
        active_project_id: Some(active.to_string()),
    }
}

macro_rules! fixture {
    ($name:ident, $file:expr) => {
        pub const $name: &str = include_str!(concat!("../../../tests/fixtures/patches/", $file));
    };
}

fixture!(ADD_XML, "add.xml");
fixture!(INSERT_XML, "insert.xml");
fixture!(REMOVE_XML, "remove.xml");
fixture!(REPLACE_XML, "replace.xml");
fixture!(ATTRIBUTE_ADD_XML, "attribute_add.xml");
fixture!(ATTRIBUTE_SET_XML, "attribute_set.xml");
fixture!(ATTRIBUTE_REMOVE_XML, "attribute_remove.xml");
fixture!(ADD_MOD_EXTENSION_XML, "add_mod_extension.xml");
fixture!(SET_NAME_XML, "set_name.xml");
fixture!(TEST_OPERATION_XML, "test_operation.xml");
fixture!(SEQUENCE_XML, "sequence.xml");
fixture!(FIND_MOD_XML, "find_mod.xml");
fixture!(CONDITIONAL_MATCH_ONLY_XML, "conditional_match_only.xml");
fixture!(CONDITIONAL_NOMATCH_ONLY_XML, "conditional_nomatch_only.xml");
fixture!(CONDITIONAL_BOTH_XML, "conditional_both.xml");
fixture!(CUSTOM_OPERATION_XML, "custom_operation.xml");
fixture!(
    MALFORMED_MISSING_END_TAG_XML,
    "malformed_missing_end_tag.xml"
);
fixture!(WRONG_ROOT_XML, "wrong_root.xml");
fixture!(MISSING_CLASS_XML, "missing_class.xml");
fixture!(MISSING_XPATH_XML, "missing_xpath.xml");
fixture!(MULTIPLE_ROOTS_XML, "multiple_roots.xml");
fixture!(DUPLICATE_XPATH_XML, "duplicate_xpath.xml");
fixture!(EMPTY_ATTRIBUTE_VALUE_XML, "empty_attribute_value.xml");
fixture!(
    UNEXPECTED_CHILD_UNDER_PATCH_XML,
    "unexpected_child_under_patch.xml"
);
fixture!(
    UNEXPECTED_CHILD_UNDER_MODS_XML,
    "unexpected_child_under_mods.xml"
);

/// Every built-in operation fixture that round-trips byte-for-byte through parse + serialize.
pub const ALL_BUILT_IN_FIXTURES: &[(&str, &str)] = &[
    ("add.xml", ADD_XML),
    ("insert.xml", INSERT_XML),
    ("remove.xml", REMOVE_XML),
    ("replace.xml", REPLACE_XML),
    ("attribute_add.xml", ATTRIBUTE_ADD_XML),
    ("attribute_set.xml", ATTRIBUTE_SET_XML),
    ("attribute_remove.xml", ATTRIBUTE_REMOVE_XML),
    ("add_mod_extension.xml", ADD_MOD_EXTENSION_XML),
    ("set_name.xml", SET_NAME_XML),
    ("test_operation.xml", TEST_OPERATION_XML),
    ("sequence.xml", SEQUENCE_XML),
    ("find_mod.xml", FIND_MOD_XML),
    ("conditional_match_only.xml", CONDITIONAL_MATCH_ONLY_XML),
    ("conditional_nomatch_only.xml", CONDITIONAL_NOMATCH_ONLY_XML),
    ("conditional_both.xml", CONDITIONAL_BOTH_XML),
    ("custom_operation.xml", CUSTOM_OPERATION_XML),
];

mod diagnostics;
mod fingerprint;
mod impact_graph;
mod index;
mod parsing;
mod regressions;
mod round_trip;
mod scan;
mod serde_wire;
mod xpath;
