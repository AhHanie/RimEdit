use std::collections::BTreeMap;

use crate::patches::PatchIndexBuildOptions;
use crate::project_model::LocationKind;
use crate::schema_pack::{
    PatchOperationMetadata, PatchOperationPreview, PatchOperationPreviewKind,
};

use super::super::fingerprint::{custom_operations_fingerprint, settings_fingerprint};
use super::{location, settings_with_locations, temp_dir};

#[test]
fn settings_fingerprint_changes_when_location_order_changes() {
    // File order (and therefore stable preview order) depends on registered location order, so
    // reordering locations without adding/removing any must still invalidate the cache -- unlike
    // def_index's fingerprint, which intentionally normalizes order because Def order doesn't
    // matter there.
    let root_a = temp_dir();
    let root_b = temp_dir();
    let loc_a = location(&root_a, "a", LocationKind::Project);
    let loc_b = location(&root_b, "b", LocationKind::Source);

    let settings_ab = settings_with_locations(vec![loc_a.clone(), loc_b.clone()], "a");
    let settings_ba = settings_with_locations(vec![loc_b, loc_a], "a");
    let options = PatchIndexBuildOptions::for_project("a");

    let fp_ab = settings_fingerprint(&settings_ab, &options);
    let fp_ba = settings_fingerprint(&settings_ba, &options);

    assert_ne!(fp_ab, fp_ba);
    std::fs::remove_dir_all(&root_a).ok();
    std::fs::remove_dir_all(&root_b).ok();
}

fn custom_metadata(label: &str) -> PatchOperationMetadata {
    PatchOperationMetadata {
        class_name: "MyMod.PatchOperationCustom".to_string(),
        label: Some(label.to_string()),
        description: None,
        field_order: Vec::new(),
        fields: BTreeMap::new(),
        preview: PatchOperationPreview {
            kind: PatchOperationPreviewKind::Unsupported,
            message: None,
        },
        source_pack_id: None,
    }
}

#[test]
fn custom_operations_fingerprint_is_stable_for_the_same_map() {
    let mut map = BTreeMap::new();
    map.insert(
        "MyMod.PatchOperationCustom".to_string(),
        custom_metadata("Custom"),
    );
    assert_eq!(
        custom_operations_fingerprint(&map),
        custom_operations_fingerprint(&map)
    );
}

#[test]
fn custom_operations_fingerprint_changes_when_metadata_content_changes() {
    // A cached patch index must be invalidated when schema-pack-defined patch operation metadata
    // changes even though no project setting or patch file changed -- otherwise a mod's newly
    // added/edited custom operation metadata would silently keep serving a stale classification.
    let empty: BTreeMap<String, PatchOperationMetadata> = BTreeMap::new();
    let mut with_entry = BTreeMap::new();
    with_entry.insert(
        "MyMod.PatchOperationCustom".to_string(),
        custom_metadata("Custom"),
    );
    let mut with_edited_entry = BTreeMap::new();
    with_edited_entry.insert(
        "MyMod.PatchOperationCustom".to_string(),
        custom_metadata("Custom (edited)"),
    );

    let fp_empty = custom_operations_fingerprint(&empty);
    let fp_with_entry = custom_operations_fingerprint(&with_entry);
    let fp_with_edited_entry = custom_operations_fingerprint(&with_edited_entry);

    assert_ne!(fp_empty, fp_with_entry);
    assert_ne!(fp_with_entry, fp_with_edited_entry);
}
