use super::*;

#[test]
fn built_in_and_custom_fixtures_round_trip_byte_for_byte() {
    for (name, src) in ALL_BUILT_IN_FIXTURES {
        let file = parse_patch_file(name, src);
        assert!(
            file.diagnostics.is_empty(),
            "{name}: unexpected diagnostics: {:?}",
            file.diagnostics
        );
        let serialized = serialize_patch_file(&file);
        assert_eq!(
            &serialized, src,
            "{name}: round-trip produced different output"
        );
    }
}

#[test]
fn reparsing_serialized_output_preserves_operation_data() {
    for (name, src) in ALL_BUILT_IN_FIXTURES {
        let first = parse_patch_file(name, src);
        let serialized = serialize_patch_file(&first);
        let second = parse_patch_file(name, &serialized);
        assert_eq!(
            second.operations, first.operations,
            "{name}: reparsed operations differ from original parse"
        );
    }
}

#[test]
fn nested_sequence_round_trips_with_stable_ids() {
    let first = parse_patch_file("sequence.xml", SEQUENCE_XML);
    let serialized = serialize_patch_file(&first);
    let second = parse_patch_file("sequence.xml", &serialized);
    assert_eq!(second.operations, first.operations);
}
