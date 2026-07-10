use super::*;

#[test]
fn remove_nested_element_simple_inline_xml() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
      <texPath>Things/Fixture</texPath>
      <graphicClass>Graphic_Single</graphicClass>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: false,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>"),
        "texPath should be removed: {out}"
    );
    assert!(
        out.contains("<graphicClass>Graphic_Single</graphicClass>"),
        "graphicClass should be preserved"
    );
}

#[test]
fn remove_nested_element_removes_leaf_only() {
    // `graphic_data_nested_full.xml` has graphicData.texPath and graphicData.graphicClass.
    // Removing texPath should leave graphicClass and other graphicData children intact.
    let mut doc = parse_to_document("test.xml", FIXTURE_NESTED_FULL_XML);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: false,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>Things/Fixture/Single/FixtureSingle</texPath>"),
        "direct graphicData.texPath should be removed"
    );
    assert!(
        out.contains("<graphicClass>Graphic_Single</graphicClass>"),
        "graphicClass should be preserved"
    );
    assert!(
        out.contains("<graphicData>"),
        "graphicData container should be preserved"
    );
}

#[test]
fn remove_nested_element_missing_path_is_idempotent() {
    // No graphicData at all - removing graphicData.texPath should be a no-op.
    let src = r#"<Defs>
  <ThingDef>
    <defName>NoGraphics</defName>
    <label>no graphics</label>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: false,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    assert_eq!(serialize_xml_document(&doc), src);
}

#[test]
fn remove_nested_element_does_not_prune_unknown_siblings() {
    // Remove graphicData.texPath while preserving graphicData.graphicClass and
    // other nested children (shadowData, damageData, etc.).
    let mut doc = parse_to_document("test.xml", FIXTURE_NESTED_FULL_XML);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: false,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>Things/Fixture/Single/FixtureSingle</texPath>"),
        "direct graphicData.texPath should be removed"
    );
    assert!(
        out.contains("<shadowData>"),
        "shadowData sibling should be preserved"
    );
    assert!(
        out.contains("<damageData>"),
        "damageData sibling should be preserved"
    );
    assert!(
        out.contains("<linkType>Basic</linkType>"),
        "linkType sibling should be preserved"
    );
}

#[test]
fn set_nested_object_field_text_updates_child() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/Old</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedObjectFieldText {
            parent_node_id: def_id,
            object_name: "graphicData".to_string(),
            field_name: "texPath".to_string(),
            value: "Items/New".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(out.contains("<texPath>Items/New</texPath>"));
    assert!(!out.contains("<texPath>Items/Old</texPath>"));
}

#[test]
fn nested_object_children_survive_edit_and_reparse() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/Old</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedObjectFieldText {
            parent_node_id: def_id,
            object_name: "graphicData".to_string(),
            field_name: "texPath".to_string(),
            value: "Items/New".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let serialized = serialize_xml_document(&doc);
    let doc2 = parse_to_document("test.xml", &serialized);
    assert!(doc2.parse_diagnostics.is_empty());
    let view = build_editor_view(&doc2);
    let def = &view.defs[0];
    let gfx = def
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    let nested = gfx
        .children
        .as_ref()
        .expect("graphicData should have children after reparse");
    let tex = nested.iter().find(|c| c.name == "texPath").unwrap();
    assert_eq!(tex.text_value.as_deref(), Some("Items/New"));
}

#[test]
fn set_nested_element_text_updates_existing_one_level_child() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/Old</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            value: "Items/New".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<texPath>Items/New</texPath>"),
        "new value must be present: {}",
        out
    );
    assert!(
        !out.contains("<texPath>Items/Old</texPath>"),
        "old value must be gone: {}",
        out
    );
}

#[test]
fn set_nested_element_text_creates_missing_field_in_existing_object() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/X</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    let mut nested_field_orders = std::collections::HashMap::new();
    nested_field_orders.insert(
        "graphicData".to_string(),
        vec!["texPath".to_string(), "graphicClass".to_string()],
    );
    let context = XmlEditContext {
        field_order: vec![],
        nested_field_orders,
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "graphicClass".to_string(),
            value: "Graphic_Single".to_string(),
            field_order: vec![],
        },
        &context,
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<graphicClass>Graphic_Single</graphicClass>"),
        "graphicClass must be created: {}",
        out
    );
    assert!(
        out.contains("<texPath>Items/X</texPath>"),
        "texPath must be preserved: {}",
        out
    );
    let tex_pos = out.find("<texPath>").unwrap();
    let class_pos = out.find("<graphicClass>").unwrap();
    assert!(
        tex_pos < class_pos,
        "texPath must precede graphicClass per field order: {}",
        out
    );
}

#[test]
fn set_nested_element_text_creates_missing_top_level_object() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    let context = XmlEditContext {
        field_order: vec!["defName".to_string(), "graphicData".to_string()],
        nested_field_orders: std::collections::HashMap::new(),
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "graphicClass".to_string(),
            value: "Graphic_Single".to_string(),
            field_order: vec![],
        },
        &context,
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<graphicData>"),
        "graphicData must be created: {}",
        out
    );
    assert!(
        out.contains("<graphicClass>Graphic_Single</graphicClass>"),
        "graphicClass must be created: {}",
        out
    );

    let gfx_open = out.find("<graphicData>").unwrap();
    let class_pos = out.find("<graphicClass>").unwrap();
    let gfx_close = out.find("</graphicData>").unwrap();
    assert!(
        gfx_open < class_pos,
        "graphicClass must be inside graphicData: {}",
        out
    );
    assert!(
        class_pos < gfx_close,
        "graphicClass must precede </graphicData>: {}",
        out
    );

    // Verify round-trip: serialized XML must re-parse without errors.
    let doc2 = parse_to_document("test.xml", &out);
    assert!(
        doc2.parse_diagnostics.is_empty(),
        "re-parsed doc must have no errors: {:?}",
        doc2.parse_diagnostics
    );
    let view = build_editor_view(&doc2);
    let gfx = view.defs[0]
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    let nested = gfx.children.as_ref().unwrap();
    assert!(nested.iter().any(|c| c.name == "graphicClass"));
}

#[test]
fn set_nested_element_text_creates_missing_deep_parent_objects() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string(), "shadowData".to_string()],
            field_name: "volume".to_string(),
            value: "(0.8, 0, 0.6)".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<graphicData>"),
        "graphicData must be created: {}",
        out
    );
    assert!(
        out.contains("<shadowData>"),
        "shadowData must be created: {}",
        out
    );
    assert!(
        out.contains("<volume>(0.8, 0, 0.6)</volume>"),
        "volume must be created: {}",
        out
    );
    assert!(
        out.contains("</shadowData>"),
        "shadowData must have closing tag: {}",
        out
    );
    assert!(
        out.contains("</graphicData>"),
        "graphicData must have closing tag: {}",
        out
    );

    let gfx_open = out.find("<graphicData>").unwrap();
    let shadow_open = out.find("<shadowData>").unwrap();
    let vol_pos = out.find("<volume>").unwrap();
    let shadow_close = out.find("</shadowData>").unwrap();
    let gfx_close = out.find("</graphicData>").unwrap();
    assert!(gfx_open < shadow_open);
    assert!(shadow_open < vol_pos);
    assert!(vol_pos < shadow_close);
    assert!(shadow_close < gfx_close);

    // Round-trip
    let doc2 = parse_to_document("test.xml", &out);
    assert!(
        doc2.parse_diagnostics.is_empty(),
        "re-parsed doc must have no errors: {:?}",
        doc2.parse_diagnostics
    );
    let view = build_editor_view(&doc2);
    let gfx = view.defs[0]
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    let nested = gfx.children.as_ref().unwrap();
    let shadow = nested.iter().find(|c| c.name == "shadowData").unwrap();
    let shadow_children = shadow.children.as_ref().unwrap();
    let vol = shadow_children.iter().find(|c| c.name == "volume").unwrap();
    assert_eq!(vol.text_value.as_deref(), Some("(0.8, 0, 0.6)"));
}

#[test]
fn set_nested_element_text_on_self_closing_object_creates_child() {
    let src =
        "<Defs>\n  <ThingDef>\n    <defName>X</defName>\n    <graphicData/>\n  </ThingDef>\n</Defs>";
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "graphicClass".to_string(),
            value: "Graphic_Single".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<graphicData/>"),
        "self-closing form must be gone: {}",
        out
    );
    assert!(
        out.contains("<graphicData>"),
        "graphicData must have opening tag: {}",
        out
    );
    assert!(
        out.contains("</graphicData>"),
        "graphicData must have closing tag: {}",
        out
    );
    assert!(
        out.contains("<graphicClass>Graphic_Single</graphicClass>"),
        "graphicClass must be created: {}",
        out
    );
}

#[test]
fn legacy_set_nested_object_field_text_still_deserializes_and_updates() {
    let edit: XmlEdit = serde_json::from_str(
        r#"{
          "type": "setNestedObjectFieldText",
          "parentNodeId": 1,
          "objectName": "graphicData",
          "fieldName": "texPath",
          "value": "Items/Updated"
        }"#,
    )
    .unwrap();

    match &edit {
        XmlEdit::SetNestedObjectFieldText {
            parent_node_id,
            object_name,
            field_name,
            value,
        } => {
            assert_eq!(*parent_node_id, 1);
            assert_eq!(object_name, "graphicData");
            assert_eq!(field_name, "texPath");
            assert_eq!(value, "Items/Updated");
        }
        _ => panic!("expected SetNestedObjectFieldText"),
    }

    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/Old</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    // Rebuild with the correct parent_node_id from the parsed document.
    let runtime_edit = XmlEdit::SetNestedObjectFieldText {
        parent_node_id: def_id,
        object_name: "graphicData".to_string(),
        field_name: "texPath".to_string(),
        value: "Items/Updated".to_string(),
    };
    apply_xml_edit(&mut doc, runtime_edit, &XmlEditContext::default()).unwrap();

    // Legacy variant should update through SetNestedElementText delegation.
    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<texPath>Items/Updated</texPath>"),
        "legacy edit must update value: {}",
        out
    );
}

#[test]
fn set_nested_element_text_deserializes_frontend_camel_case_fields() {
    let edit: XmlEdit = serde_json::from_str(
        r#"{
          "type": "setNestedElementText",
          "parentNodeId": 1,
          "objectPath": ["graphicData", "shadowData"],
          "fieldName": "volume",
          "value": "(0.8, 0, 0.6)"
        }"#,
    )
    .unwrap();

    match edit {
        XmlEdit::SetNestedElementText {
            parent_node_id,
            object_path,
            field_name,
            value,
            ..
        } => {
            assert_eq!(parent_node_id, 1);
            assert_eq!(object_path, vec!["graphicData", "shadowData"]);
            assert_eq!(field_name, "volume");
            assert_eq!(value, "(0.8, 0, 0.6)");
        }
        _ => panic!("expected SetNestedElementText"),
    }
}

#[test]
fn remove_nested_element_prunes_empty_parent_when_requested() {
    // graphicData contains only texPath - removing it with pruning should remove graphicData too.
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
      <texPath>Things/Fixture</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>"),
        "texPath should be removed: {out}"
    );
    assert!(
        !out.contains("<graphicData>"),
        "empty graphicData should be pruned: {out}"
    );
    assert!(
        out.contains("<defName>Test</defName>"),
        "defName should be preserved: {out}"
    );
}

#[test]
fn remove_nested_element_prunes_empty_nested_parent_chain() {
    // graphicData > shadowData > volume - removing volume with pruning should remove both
    // shadowData and graphicData if no other content remains.
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
      <shadowData>
        <volume>0.5</volume>
      </shadowData>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string(), "shadowData".to_string()],
            field_name: "volume".to_string(),
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(!out.contains("<volume>"), "volume should be removed: {out}");
    assert!(
        !out.contains("<shadowData>"),
        "empty shadowData should be pruned: {out}"
    );
    assert!(
        !out.contains("<graphicData>"),
        "empty graphicData should be pruned: {out}"
    );
    assert!(
        out.contains("<defName>Test</defName>"),
        "defName should be preserved: {out}"
    );
}

#[test]
fn remove_nested_element_with_prune_preserves_non_empty_parent() {
    // graphicData has texPath and graphicClass - removing texPath should leave graphicData
    // and graphicClass intact.
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
      <texPath>Things/Fixture</texPath>
      <graphicClass>Graphic_Single</graphicClass>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>"),
        "texPath should be removed: {out}"
    );
    assert!(
        out.contains("<graphicData>"),
        "non-empty graphicData should be preserved: {out}"
    );
    assert!(
        out.contains("<graphicClass>Graphic_Single</graphicClass>"),
        "graphicClass should be preserved: {out}"
    );
}

#[test]
fn remove_nested_element_with_prune_preserves_unknown_sibling() {
    // graphicData has texPath and an unknown child - removing texPath should leave graphicData
    // and the unknown child intact because unknown children count as meaningful content.
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
      <texPath>Things/Fixture</texPath>
      <unknownChild>some value</unknownChild>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>"),
        "texPath should be removed: {out}"
    );
    assert!(
        out.contains("<graphicData>"),
        "graphicData with unknown child should be preserved: {out}"
    );
    assert!(
        out.contains("<unknownChild>some value</unknownChild>"),
        "unknownChild should be preserved: {out}"
    );
}

#[test]
fn remove_nested_element_with_prune_missing_field_does_not_prune() {
    // graphicData exists but texPath is absent. With pruning enabled, this must be a
    // no-op - the empty (whitespace-only) graphicData container must NOT be removed.
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    assert_eq!(serialize_xml_document(&doc), src);
}

#[test]
fn remove_nested_element_without_prune_keeps_empty_parent() {
    // prune_empty_ancestors: false - removing the only child should leave the empty container.
    let src = r#"<Defs>
  <ThingDef>
    <defName>Test</defName>
    <graphicData>
      <texPath>Things/Fixture</texPath>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNestedElement {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "texPath".to_string(),
            prune_empty_ancestors: false,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<texPath>"),
        "texPath should be removed: {out}"
    );
    assert!(
        out.contains("<graphicData>"),
        "graphicData should be kept when pruning disabled: {out}"
    );
}

#[test]
fn set_nested_element_attribute_updates_existing_object_attribute() {
    let src = r#"<Defs>
  <QuestScriptDef>
    <defName>SomeQuest</defName>
    <root Class="QuestNode_Sequence">
      <nodes>
        <li Class="QuestNode_Set" />
      </nodes>
    </root>
  </QuestScriptDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementAttribute {
            parent_node_id: def_id,
            object_path: vec!["root".to_string()],
            attribute_name: "Class".to_string(),
            value: "QuestNode_Chance".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains(r#"<root Class="QuestNode_Chance""#),
        "Class attribute should be updated: {out}"
    );
    assert!(
        out.contains("<nodes>"),
        "child nodes should be preserved: {out}"
    );
}

#[test]
fn set_nested_element_attribute_creates_missing_object_path() {
    let src = r#"<Defs>
  <QuestScriptDef>
    <defName>SomeQuest</defName>
  </QuestScriptDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementAttribute {
            parent_node_id: def_id,
            object_path: vec!["root".to_string()],
            attribute_name: "Class".to_string(),
            value: "QuestNode_Sequence".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains(r#"Class="QuestNode_Sequence""#),
        "attribute should be set on the created element: {out}"
    );
    assert!(
        out.contains("<root"),
        "root element should be created: {out}"
    );
}
