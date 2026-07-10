use super::*;

#[test]
fn round_trip_untouched_returns_exact_source() {
    for src in [THING_DEFS_XML, SINGLE_DEF_XML, CDATA_XML] {
        let doc = parse_to_document("test.xml", src);
        assert!(doc.parse_diagnostics.is_empty(), "unexpected parse error");
        let serialized = serialize_xml_document(&doc);
        assert_eq!(serialized, src, "round-trip produced different output");
    }
}

#[test]
fn fixture_graphic_data_xmls_all_parse_without_errors() {
    let fixtures = [
        ("graphic_data_single.xml", FIXTURE_SINGLE_XML),
        ("graphic_data_multi.xml", FIXTURE_MULTI_XML),
        ("graphic_data_random.xml", FIXTURE_RANDOM_XML),
        ("graphic_data_stack_count.xml", FIXTURE_STACK_XML),
        ("graphic_data_nested_full.xml", FIXTURE_NESTED_FULL_XML),
        (
            "graphic_data_missing_texture.xml",
            FIXTURE_MISSING_TEXTURE_XML,
        ),
        ("graphic_data_unknown_class.xml", FIXTURE_UNKNOWN_CLASS_XML),
    ];
    for (name, src) in fixtures {
        let doc = parse_to_document(name, src);
        assert!(
            doc.parse_diagnostics.is_empty(),
            "{name} must parse without errors: {:?}",
            doc.parse_diagnostics
        );
        assert_eq!(
            doc.def_summaries.len(),
            1,
            "{name} must contain exactly one def"
        );
    }
}

#[test]
fn round_trip_preserves_unknown_nodes_and_ordering() {
    let doc = parse_to_document("test.xml", THING_DEFS_XML);
    assert!(doc.parse_diagnostics.is_empty());
    let serialized = serialize_xml_document(&doc);
    assert!(serialized.contains("<!-- steel comment -->"));
    assert!(serialized.contains("<unknownTag>"));
    assert!(serialized.contains("<nestedUnknown>"));
    let def_name_pos = serialized.find("<defName>").unwrap();
    let label_pos = serialized.find("<label>").unwrap();
    assert!(def_name_pos < label_pos);
}

#[test]
fn def_fields_extracted_from_thing_defs() {
    let doc = parse_to_document("test.xml", THING_DEFS_XML);
    assert!(doc.parse_diagnostics.is_empty());
    assert!(!doc.def_summaries.is_empty(), "expected at least one def");

    let steel = doc
        .def_summaries
        .iter()
        .find(|d| d.def_name.as_deref() == Some("Steel"));
    assert!(steel.is_some(), "expected Steel def");
    let steel = steel.unwrap();
    assert_eq!(steel.def_type, "ThingDef");
    assert_eq!(steel.label.as_deref(), Some("steel"));
}

#[test]
fn parent_name_extracted_from_def_attribute() {
    let doc = parse_to_document("test.xml", THING_DEFS_XML);
    assert!(doc.parse_diagnostics.is_empty());
    let with_parent = doc.def_summaries.iter().find(|d| d.parent_name.is_some());
    assert!(with_parent.is_some(), "expected a def with ParentName");
    assert_eq!(
        with_parent.unwrap().parent_name.as_deref(),
        Some("BaseOrganic")
    );
}

#[test]
fn malformed_xml_returns_parse_diagnostic() {
    let result = parse_xml_document("bad.xml", MALFORMED_XML);
    assert!(
        result.document.is_none(),
        "expected no document for malformed XML"
    );
    assert!(
        !result.parse_diagnostics.is_empty(),
        "expected parse diagnostics"
    );
    let diag = &result.parse_diagnostics[0];
    assert_eq!(diag.relative_path, "bad.xml");
    assert!(diag.line.is_some());
    assert!(diag.column.is_some());
    assert!(diag.byte_offset.is_some());
    assert!(!diag.message.is_empty());
}

#[test]
fn validation_diagnostics_are_empty() {
    let result = parse_xml_document("test.xml", THING_DEFS_XML);
    assert!(result.validation_diagnostics.is_empty());
}

#[test]
fn root_element_skips_xml_declaration() {
    let result = parse_xml_document("test.xml", THING_DEFS_XML);
    assert!(result.document.is_some());
    assert_eq!(
        result.document.unwrap().root_element.as_deref(),
        Some("Defs")
    );
}

#[test]
fn invalid_attribute_entity_produces_diagnostic_not_fatal() {
    let src = r#"<Defs>
  <ThingDef badAttr="&invalid;">
    <defName>X</defName>
  </ThingDef>
</Defs>"#;
    let result = parse_xml_document("test.xml", src);
    assert!(
        result.document.is_some(),
        "document should be Some despite bad entity"
    );
    assert!(
        !result.parse_diagnostics.is_empty(),
        "expected a parse diagnostic for the invalid entity"
    );
}
