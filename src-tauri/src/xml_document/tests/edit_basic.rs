use super::*;

#[test]
fn set_child_element_text_changes_only_target() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Steel</defName>
    <label>steel</label>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "defName".to_string(),
            value: "IronOre".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(out.contains("<defName>IronOre</defName>"));
    assert!(out.contains("<label>steel</label>"));
}

#[test]
fn set_child_element_text_preserves_unknown_siblings() {
    let src = r#"<Defs>
  <ThingDef>
    <unknownBefore>123</unknownBefore>
    <defName>Steel</defName>
    <unknownAfter>456</unknownAfter>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "defName".to_string(),
            value: "NewName".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(out.contains("<unknownBefore>123</unknownBefore>"));
    assert!(out.contains("<defName>NewName</defName>"));
    assert!(out.contains("<unknownAfter>456</unknownAfter>"));

    let before_pos = out.find("<unknownBefore>").unwrap();
    let def_pos = out.find("<defName>").unwrap();
    let after_pos = out.find("<unknownAfter>").unwrap();
    assert!(before_pos < def_pos);
    assert!(def_pos < after_pos);
}

#[test]
fn remove_element_attribute_removes_attribute_only() {
    // `cdata_and_attributes.xml` has Abstract="True" Name="BaseMineable".
    // Removing Abstract should leave Name and all child elements intact.
    let mut doc = parse_to_document("test.xml", CDATA_XML);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveElementAttribute {
            element_node_id: def_id,
            attribute_name: "Abstract".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("Abstract"),
        "Abstract attribute should be removed"
    );
    assert!(
        out.contains("Name=\"BaseMineable\""),
        "Name attribute should be preserved"
    );
    assert!(
        out.contains("<defName>BaseMineable</defName>"),
        "child elements should be preserved"
    );
}

#[test]
fn remove_element_attribute_missing_attribute_is_idempotent() {
    let src = r#"<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    // Removing a non-existent attribute should succeed without modifying the document.
    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveElementAttribute {
            element_node_id: def_id,
            attribute_name: "NonExistent".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    assert_eq!(serialize_xml_document(&doc), src);
}

#[test]
fn remove_child_element_removes_list_container() {
    // Removing the entire weaponClasses list container should eliminate it.
    let mut doc = parse_to_document("test.xml", FIXTURE_WEAPON_XML);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveChildElement {
            parent_node_id: def_id,
            child_name: "weaponClasses".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("weaponClasses"),
        "weaponClasses container should be removed"
    );
    assert!(
        out.contains("<defName>SwordSample</defName>"),
        "other fields should be preserved"
    );
}

#[test]
fn set_element_attribute_preserves_other_attributes_and_children() {
    let src = r#"<Defs>
  <ThingDef ParentName="BaseOrganic" Abstract="True">
    <defName>Hay</defName>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetElementAttribute {
            element_node_id: def_id,
            attribute_name: "ParentName".to_string(),
            value: "BasePlant".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(out.contains("ParentName=\"BasePlant\""));
    assert!(out.contains("Abstract=\"True\""));
    assert!(out.contains("<defName>Hay</defName>"));
}

#[test]
fn inserted_child_serializes_before_closing_indent() {
    let src = "<Defs>\n  <ThingDef>\n    <label>steel</label>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "defName".to_string(),
            value: "Steel".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    let def_pos = out.find("<defName>Steel</defName>").unwrap();
    let close_pos = out.find("</ThingDef>").unwrap();
    assert!(def_pos < close_pos, "defName must precede </ThingDef>");
    let between = &out[def_pos + "<defName>Steel</defName>".len()..close_pos];
    assert!(
        between.contains('\n'),
        "expected a newline between inserted field and closing tag, got: {:?}",
        between
    );
}

#[test]
fn remove_child_element_leaves_siblings_intact() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Steel</defName>
    <label>steel</label>
    <description>A material.</description>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveChildElement {
            parent_node_id: def_id,
            child_name: "description".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(out.contains("<defName>Steel</defName>"));
    assert!(out.contains("<label>steel</label>"));
    assert!(!out.contains("<description>"));
}

#[test]
fn set_child_element_text_replaces_cdata_with_plain_text() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <description><![CDATA[Old <value> & data]]></description>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "description".to_string(),
            value: "New plain text".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<description>New plain text</description>"),
        "expected plain text description, got: {}",
        out
    );
    assert!(!out.contains("<![CDATA["), "CDATA section must be replaced");
    assert!(
        out.contains("<defName>X</defName>"),
        "other fields must be preserved"
    );
}

#[test]
fn set_text_on_self_closing_element_renders_as_regular_element() {
    let src = "<Defs>\n  <ThingDef>\n    <label/>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "label".to_string(),
            value: "stone".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<label>stone</label>"),
        "expected <label>stone</label>, got: {}",
        out
    );
    assert!(!out.contains("<label/>"), "self-closing form must be gone");
}

#[test]
fn schema_context_inserts_field_between_known_siblings() {
    // XML has defName and description but no label.
    // With order [defName, label, description], label must land between them.
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n    <description>D</description>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;
    let context = XmlEditContext {
        field_order: vec![
            "defName".to_string(),
            "label".to_string(),
            "description".to_string(),
        ],
        ..XmlEditContext::default()
    };
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "label".to_string(),
            value: "x".to_string(),
        },
        &context,
    )
    .unwrap();
    let out = serialize_xml_document(&doc);
    let def_pos = out.find("<defName>").unwrap();
    let label_pos = out.find("<label>").unwrap();
    let desc_pos = out.find("<description>").unwrap();
    assert!(
        def_pos < label_pos && label_pos < desc_pos,
        "expected defName < label < description, got: {}",
        out
    );
}

#[test]
fn default_context_does_not_enforce_field_order() {
    // With an empty context, inserting defName into XML that only has label
    // should append before trailing whitespace - no enforced ordering.
    let src = "<Defs>\n  <ThingDef>\n    <label>x</label>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "defName".to_string(),
            value: "Y".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();
    let out = serialize_xml_document(&doc);
    assert!(out.contains("<label>x</label>"), "label must be preserved");
    assert!(
        out.contains("<defName>Y</defName>"),
        "defName must be inserted"
    );
}

#[test]
fn schema_context_appends_unknown_field_before_trailing_whitespace() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;
    let context = XmlEditContext {
        field_order: vec!["defName".to_string(), "label".to_string()],
        ..XmlEditContext::default()
    };
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "unknownField".to_string(),
            value: "val".to_string(),
        },
        &context,
    )
    .unwrap();
    let out = serialize_xml_document(&doc);
    let unknown_pos = out.find("<unknownField>").unwrap();
    let close_pos = out.find("</ThingDef>").unwrap();
    assert!(
        unknown_pos < close_pos,
        "unknown field must precede closing tag"
    );
}

#[test]
fn schema_context_inserts_field_before_first_known_sibling() {
    // XML has only <label> but no <defName>.
    // With order [defName, label], defName must land BEFORE label.
    let src = "<Defs>\n  <ThingDef>\n    <label>x</label>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;
    let context = XmlEditContext {
        field_order: vec!["defName".to_string(), "label".to_string()],
        ..XmlEditContext::default()
    };
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "defName".to_string(),
            value: "Steel".to_string(),
        },
        &context,
    )
    .unwrap();
    let out = serialize_xml_document(&doc);
    let def_pos = out.find("<defName>").unwrap();
    let label_pos = out.find("<label>").unwrap();
    assert!(
        def_pos < label_pos,
        "expected defName before label, got: {}",
        out
    );
}

#[test]
fn xml_edit_deserializes_frontend_camel_case_fields() {
    let edit: XmlEdit = serde_json::from_str(
        r#"{
          "type": "setChildElementText",
          "parentNodeId": 7,
          "childName": "description",
          "value": "Updated"
        }"#,
    )
    .unwrap();

    match edit {
        XmlEdit::SetChildElementText {
            parent_node_id,
            child_name,
            value,
        } => {
            assert_eq!(parent_node_id, 7);
            assert_eq!(child_name, "description");
            assert_eq!(value, "Updated");
        }
        _ => panic!("expected SetChildElementText"),
    }
}
