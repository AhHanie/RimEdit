use super::*;

#[test]
fn malformed_xml_reports_diagnostics_instead_of_panicking() {
    let file = parse_patch_file("test.xml", MALFORMED_MISSING_END_TAG_XML);
    assert!(file.had_fatal_parse_error);
    assert!(file.operations.is_empty());
    assert!(!file.diagnostics.is_empty());
}

#[test]
fn wrong_root_element_reports_diagnostic() {
    let file = parse_patch_file("test.xml", WRONG_ROOT_XML);
    assert!(!file.had_fatal_parse_error);
    assert!(file.operations.is_empty());
    assert!(file
        .diagnostics
        .iter()
        .any(|d| d.message.contains("root element must be <Patch>")));
}

#[test]
fn missing_class_attribute_is_unknown_with_diagnostic() {
    let file = parse_patch_file("test.xml", MISSING_CLASS_XML);
    assert!(!file.had_fatal_parse_error);
    assert_eq!(file.operations.len(), 1);
    assert!(file
        .diagnostics
        .iter()
        .any(|d| d.message.contains("missing a Class attribute")));
    assert!(matches!(
        file.operations[0].kind,
        PatchOperationKind::Unknown(_)
    ));
}

#[test]
fn missing_required_xpath_reports_diagnostic() {
    let file = parse_patch_file("test.xml", MISSING_XPATH_XML);
    assert!(!file.had_fatal_parse_error);
    assert_eq!(file.operations.len(), 1);
    assert!(file
        .diagnostics
        .iter()
        .any(|d| d.message.contains("missing required <xpath> field")));
    match &file.operations[0].kind {
        PatchOperationKind::Remove(inner) => assert!(inner.xpath.is_none()),
        other => panic!("expected Remove, got {:?}", other),
    }
}

#[test]
fn unexpected_child_under_patch_reports_diagnostic_and_is_skipped() {
    let file = parse_patch_file("test.xml", UNEXPECTED_CHILD_UNDER_PATCH_XML);
    assert!(!file.had_fatal_parse_error);
    // The stray child is skipped, not parsed as an operation.
    assert_eq!(file.operations.len(), 1);
    assert!(file.diagnostics.iter().any(|d| d
        .message
        .contains("unexpected child <NotAnOperation> of <Patch>; expected <Operation>")));
}

#[test]
fn unexpected_child_under_mods_reports_diagnostic_and_is_skipped() {
    let file = parse_patch_file("test.xml", UNEXPECTED_CHILD_UNDER_MODS_XML);
    assert!(!file.had_fatal_parse_error);
    assert_eq!(file.operations.len(), 1);
    assert!(file.diagnostics.iter().any(|d| d
        .message
        .contains("unexpected child <NotALi> of <mods>; expected <li>")));
    match &file.operations[0].kind {
        PatchOperationKind::FindMod { mods, .. } => {
            assert_eq!(mods, &vec!["Harmony".to_string()]);
        }
        other => panic!("expected FindMod, got {:?}", other),
    }
}
