use super::super::edit::TypedReferenceListItem;
use super::*;

#[test]
fn set_typed_reference_list_items_creates_named_element_children() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![
                TypedReferenceListItem {
                    def_type: "ThingDef".to_string(),
                    def_name: "SimpleProstheticLeg".to_string(),
                },
                TypedReferenceListItem {
                    def_type: "HediffDef".to_string(),
                    def_name: "SimpleProstheticLeg".to_string(),
                },
            ],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<ThingDef>SimpleProstheticLeg</ThingDef>"),
        "expected ThingDef child: {}",
        out
    );
    assert!(
        out.contains("<HediffDef>SimpleProstheticLeg</HediffDef>"),
        "expected HediffDef child: {}",
        out
    );
    let thing_pos = out.find("<ThingDef>").unwrap();
    let hediff_pos = out.find("<HediffDef>").unwrap();
    assert!(
        thing_pos < hediff_pos,
        "ThingDef must precede HediffDef: {}",
        out
    );
}

#[test]
fn set_typed_reference_list_items_same_type_repeated() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![
                TypedReferenceListItem {
                    def_type: "ThingDef".to_string(),
                    def_name: "Steel".to_string(),
                },
                TypedReferenceListItem {
                    def_type: "ThingDef".to_string(),
                    def_name: "Wood".to_string(),
                },
            ],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<ThingDef>Steel</ThingDef>"),
        "first ThingDef must be present: {}",
        out
    );
    assert!(
        out.contains("<ThingDef>Wood</ThingDef>"),
        "second ThingDef must be present: {}",
        out
    );
    // Both must appear in the output - repeated element names are preserved.
    let count = out.matches("<ThingDef>Steel</ThingDef>").count()
        + out.matches("<ThingDef>Wood</ThingDef>").count();
    assert_eq!(
        count, 2,
        "expected two typed-reference <ThingDef> elements, got {count}: {out}"
    );
}

#[test]
fn set_typed_reference_list_items_creates_container_when_missing() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![TypedReferenceListItem {
                def_type: "ThingDef".to_string(),
                def_name: "Steel".to_string(),
            }],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<descriptionHyperlinks>"),
        "container must be created: {}",
        out
    );
    assert!(
        out.contains("<ThingDef>Steel</ThingDef>"),
        "item must be present: {}",
        out
    );
}

#[test]
fn set_typed_reference_list_items_replaces_existing_children() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n    <descriptionHyperlinks>\n      <ThingDef>OldItem</ThingDef>\n    </descriptionHyperlinks>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![TypedReferenceListItem {
                def_type: "HediffDef".to_string(),
                def_name: "NewItem".to_string(),
            }],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("OldItem"),
        "old item must be removed: {}",
        out
    );
    assert!(
        out.contains("<HediffDef>NewItem</HediffDef>"),
        "new item must be present: {}",
        out
    );
}

#[test]
fn set_typed_reference_list_items_empty_keeps_container() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n    <descriptionHyperlinks>\n      <ThingDef>Steel</ThingDef>\n    </descriptionHyperlinks>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<descriptionHyperlinks>"),
        "empty container must be kept: {}",
        out
    );
    assert!(!out.contains("Steel"), "old item must be removed: {}", out);
}

#[test]
fn set_typed_reference_list_items_invalid_def_type_returns_error() {
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    let result = apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![TypedReferenceListItem {
                def_type: "123Invalid".to_string(),
                def_name: "Steel".to_string(),
            }],
        },
        &XmlEditContext::default(),
    );

    assert!(
        matches!(result, Err(XmlEditError::InvalidElementName(_))),
        "invalid def type name must return InvalidElementName error"
    );
}

#[test]
fn set_typed_reference_list_items_inserts_in_field_order() {
    // XML has defName but not descriptionHyperlinks; with field order from Def,
    // descriptionHyperlinks should land after description.
    let src = "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n    <description>D</description>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    let context = XmlEditContext {
        field_order: vec![
            "defName".to_string(),
            "label".to_string(),
            "description".to_string(),
            "descriptionHyperlinks".to_string(),
        ],
        ..XmlEditContext::default()
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetTypedReferenceListItems {
            parent_node_id: def_id,
            object_path: vec![],
            field_name: "descriptionHyperlinks".to_string(),
            items: vec![TypedReferenceListItem {
                def_type: "ThingDef".to_string(),
                def_name: "Steel".to_string(),
            }],
        },
        &context,
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    let desc_pos = out.find("<description>").unwrap();
    let links_pos = out.find("<descriptionHyperlinks>").unwrap();
    assert!(
        desc_pos < links_pos,
        "descriptionHyperlinks must appear after description: {}",
        out
    );
}
