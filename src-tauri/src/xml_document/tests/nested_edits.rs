use super::*;

#[test]
fn generic_nested_object_list_edit_serializes() {
    // Inserting a <li> into a list that is inside a nested object element should
    // navigate the object path and create the container if absent.
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>X</defName>
    <config>
    </config>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::InsertObjectListItem {
            parent_node_id: def_id,
            object_path: vec!["config".to_string()],
            list_name: "actions".to_string(),
            class_attribute: Some("Action_Number".to_string()),
            after_item_node_id: None,
            initial_child_fields: vec![
                NameValuePair {
                    name: "keyword".to_string(),
                    value: "Digit".to_string(),
                },
                NameValuePair {
                    name: "range".to_string(),
                    value: "0~9".to_string(),
                },
            ],
            field_order: vec!["keyword".to_string(), "range".to_string()],
            initial_children: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<actions>"),
        "list container must be created: {out}"
    );
    assert!(
        out.contains("Class=\"Action_Number\""),
        "Class attribute must be set: {out}"
    );
    assert!(
        out.contains("<keyword>Digit</keyword>"),
        "keyword child must be present: {out}"
    );
    assert!(
        out.contains("<range>0~9</range>"),
        "range child must be present: {out}"
    );

    let config_open = out.find("<config>").unwrap();
    let actions_open = out.find("<actions>").unwrap();
    let actions_close = out.find("</actions>").unwrap();
    let config_close = out.find("</config>").unwrap();
    assert!(
        config_open < actions_open && actions_close < config_close,
        "<actions> must be nested inside <config>: {out}"
    );
}

#[test]
fn nested_object_list_remove_last_item_prunes_empty_ancestors() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>X</defName>
    <config>
      <actions>
        <li Class="Action_A">
          <keyword>test</keyword>
        </li>
      </actions>
    </config>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());

    // Collect the <li> node id.
    let li_id = {
        let view = build_editor_view(&doc);
        let config = view.defs[0]
            .children
            .iter()
            .find(|c| c.name == "config")
            .unwrap();
        let actions = config
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|c| c.name == "actions")
            .unwrap();
        actions.li_items[0].node_id
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveObjectListItem {
            list_item_node_id: li_id,
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<actions>"),
        "empty <actions> must be pruned: {out}"
    );
    assert!(
        !out.contains("<config>"),
        "empty <config> must be pruned: {out}"
    );
    assert!(
        out.contains("<defName>X</defName>"),
        "defName must remain: {out}"
    );
}

#[test]
fn nested_object_list_remove_nonlast_item_does_not_prune() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>X</defName>
    <config>
      <actions>
        <li Class="Action_A"><keyword>first</keyword></li>
        <li Class="Action_B"><keyword>second</keyword></li>
      </actions>
    </config>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());

    let li_id = {
        let view = build_editor_view(&doc);
        let config = view.defs[0]
            .children
            .iter()
            .find(|c| c.name == "config")
            .unwrap();
        let actions = config
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|c| c.name == "actions")
            .unwrap();
        actions.li_items[0].node_id
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveObjectListItem {
            list_item_node_id: li_id,
            prune_empty_ancestors: true,
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<actions>"),
        "<actions> must remain when still has items: {out}"
    );
    assert!(
        out.contains("<config>"),
        "<config> must remain when still has children: {out}"
    );
    assert!(out.contains("Action_B"), "second item must remain: {out}");
}

#[test]
fn generic_discriminator_unknown_class_validation() {
    // An unknown Class with allowUnknown=true must not emit an object-class warning,
    // but should still validate base fields that are known.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <knownDiscriminated>
      <li Class="CustomModClass">
        <baseField>value</baseField>
      </li>
    </knownDiscriminated>
  </TestDef>
</Defs>"#;
    let diags = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "validation_unknown_object_class"),
        "allowUnknown=true must suppress object class warning: {diags:?}"
    );
    assert!(
        diags.iter().all(|d| !d.blocking),
        "unknown class with allowUnknown=true must produce no blocking diagnostic: {diags:?}"
    );
}

#[test]
fn validation_accepts_all_known_nested_fields() {
    let xml = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <knownNested>
      <nestedKnown>hello</nestedKnown>
    </knownNested>
    <knownList>
      <li>
        <itemKnown>world</itemKnown>
      </li>
    </knownList>
    <knownDiscriminated>
      <li Class="VariantA">
        <variantAField>foo</variantAField>
      </li>
      <li Class="VariantB">
        <variantBField>bar</variantBField>
      </li>
    </knownDiscriminated>
  </TestDef>
</Defs>"#;
    let diags = validate_test_xml_with_fixture(xml, "generic_validation", &empty_def_index());
    let unknown_fields: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.code == "validation_unknown_field" || d.code == "validation_unknown_object_field"
        })
        .collect();
    assert!(
        unknown_fields.is_empty(),
        "known nested fields must not produce unknown-field diagnostics: {unknown_fields:?}"
    );
}

#[test]
fn validation_unknown_discriminator_class_is_not_blocking() {
    let xml = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <knownDiscriminated>
      <li Class="UnknownVariant">
        <baseField>x</baseField>
      </li>
    </knownDiscriminated>
  </TestDef>
</Defs>"#;
    let diags = validate_test_xml_with_fixture(xml, "generic_validation", &empty_def_index());
    let blocking: Vec<_> = diags.iter().filter(|d| d.blocking).collect();
    assert!(
        blocking.is_empty(),
        "unknown discriminator class must produce no blocking diagnostic: {blocking:?}"
    );
}

#[test]
fn nested_element_edit_updates_existing_value() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>EditTest</defName>
    <outerSection>
      <textField>original-value</textField>
    </outerSection>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["outerSection".to_string()],
            field_name: "textField".to_string(),
            value: "updated-value".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("original-value"),
        "old value must be gone: {out}"
    );
    assert!(
        out.contains("<textField>updated-value</textField>"),
        "new value must be present: {out}"
    );
    let field_pos = out.find("<textField>").unwrap();
    let section_open = out.find("<outerSection>").unwrap();
    let section_close = out.find("</outerSection>").unwrap();
    assert!(
        section_open < field_pos && field_pos < section_close,
        "textField must be inside outerSection: {out}"
    );
}

#[test]
fn nested_element_edit_creates_missing_field() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>EditTest</defName>
    <outerSection>
      <textField>some-value</textField>
    </outerSection>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let def_id = doc.def_summaries[0].node_id;

    let context = XmlEditContext {
        field_order: vec!["defName".to_string(), "outerSection".to_string()],
        nested_field_orders: {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "outerSection".to_string(),
                vec!["textField".to_string(), "enumField".to_string()],
            );
            m
        },
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["outerSection".to_string()],
            field_name: "enumField".to_string(),
            value: "Option_A".to_string(),
            field_order: vec![],
        },
        &context,
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<enumField>Option_A</enumField>"),
        "enumField must be created: {out}"
    );
    let text_pos = out.find("<textField>").unwrap();
    let enum_pos = out.find("<enumField>").unwrap();
    assert!(
        text_pos < enum_pos,
        "textField must appear before enumField: {out}"
    );
}

#[test]
fn nested_element_edit_creates_deep_nested_section() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>EditTest</defName>
    <outerSection>
      <textField>value</textField>
    </outerSection>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: def_id,
            object_path: vec!["outerSection".to_string(), "innerSection".to_string()],
            field_name: "vecField".to_string(),
            value: "(1, 0, 0)".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<innerSection>"),
        "innerSection must be created: {out}"
    );
    assert!(
        out.contains("<vecField>(1, 0, 0)</vecField>"),
        "vecField must be created: {out}"
    );
    assert!(
        out.contains("</innerSection>"),
        "innerSection must have closing tag: {out}"
    );
    assert!(
        out.contains("</outerSection>"),
        "outerSection must have closing tag: {out}"
    );

    let inner_open = out.find("<innerSection>").unwrap();
    let vec_pos = out.find("<vecField>").unwrap();
    let inner_close = out.find("</innerSection>").unwrap();
    let outer_close = out.find("</outerSection>").unwrap();
    assert!(
        inner_open < vec_pos && vec_pos < inner_close && inner_close < outer_close,
        "nesting order must be correct: {out}"
    );
}

#[test]
fn nested_list_edit_round_trips() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>EditTest</defName>
    <outerSection>
      <flagsList>
        <li>FlagA</li>
        <li>FlagB</li>
      </flagsList>
    </outerSection>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());

    let section_id = {
        let view = build_editor_view(&doc);
        view.defs[0]
            .children
            .iter()
            .find(|c| c.name == "outerSection")
            .unwrap()
            .node_id
    };

    let new_flags = vec![
        "FlagC".to_string(),
        "FlagA".to_string(),
        "FlagD".to_string(),
    ];
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetListItems {
            parent_node_id: section_id,
            child_name: "flagsList".to_string(),
            items: new_flags.clone(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    let positions: Vec<usize> = ["FlagC", "FlagA", "FlagD"]
        .iter()
        .filter_map(|name| out.find(&format!("<li>{name}</li>")))
        .collect();
    assert_eq!(positions.len(), 3, "all three flags must be present: {out}");
    assert!(
        positions[0] < positions[1] && positions[1] < positions[2],
        "flags must appear in requested order: {out}"
    );
}

#[test]
fn insert_object_list_item_with_recursive_initial_children() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <SoundDef>
    <defName>Snd</defName>
    <subSounds />
  </SoundDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::InsertObjectListItem {
            parent_node_id: def_id,
            object_path: vec![],
            list_name: "subSounds".to_string(),
            class_attribute: None,
            after_item_node_id: None,
            initial_child_fields: vec![],
            field_order: vec![],
            initial_children: vec![
                InitialElement {
                    name: "name".to_string(),
                    value: Some("Explosion".to_string()),
                    attributes: vec![],
                    children: vec![],
                    li_items: vec![],
                },
                InitialElement {
                    name: "grains".to_string(),
                    value: None,
                    attributes: vec![],
                    children: vec![],
                    li_items: vec![InitialElement {
                        name: "li".to_string(),
                        value: None,
                        attributes: vec![NameValuePair {
                            name: "Class".to_string(),
                            value: "AudioGrain_Clip".to_string(),
                        }],
                        children: vec![InitialElement {
                            name: "clipPath".to_string(),
                            value: Some("Things/Sounds/Exp".to_string()),
                            attributes: vec![],
                            children: vec![],
                            li_items: vec![],
                        }],
                        li_items: vec![],
                    }],
                },
            ],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(out.contains("<name>Explosion</name>"), "scalar name: {out}");
    assert!(out.contains("<grains>"), "grains container: {out}");
    assert!(
        out.contains("Class=\"AudioGrain_Clip\""),
        "Class on nested li: {out}"
    );
    assert!(
        out.contains("<clipPath>Things/Sounds/Exp</clipPath>"),
        "deep child: {out}"
    );
    let grains_open = out.find("<grains>").unwrap();
    let clip_pos = out.find("<clipPath>").unwrap();
    let grains_close = out.find("</grains>").unwrap();
    assert!(
        grains_open < clip_pos && clip_pos < grains_close,
        "clipPath inside grains: {out}"
    );
}

#[test]
fn set_nested_element_text_relative_to_item_node_id() {
    // When the parent_node_id is a list-item (<li>) node and object_path navigates into a
    // sub-element of that item, SetNestedElementText must update the leaf field relative to
    // the li, not to the Def root.
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <SoundDef>
    <defName>Snd</defName>
    <subSounds>
      <li>
        <soundParams>
          <name>Old</name>
        </soundParams>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());

    let li_id = {
        let view = build_editor_view(&doc);
        let sub_sounds = view.defs[0]
            .children
            .iter()
            .find(|c| c.name == "subSounds")
            .unwrap();
        sub_sounds.li_items[0].node_id
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: li_id,
            object_path: vec!["soundParams".to_string()],
            field_name: "name".to_string(),
            value: "New".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<name>New</name>"),
        "new name must be present: {out}"
    );
    assert!(
        !out.contains("<name>Old</name>"),
        "old name must be gone: {out}"
    );
    let li_open = out.find("<li>").unwrap();
    let name_pos = out.find("<name>New</name>").unwrap();
    let li_close = out.find("</li>").unwrap();
    assert!(
        li_open < name_pos && name_pos < li_close,
        "name must be inside <li>: {out}"
    );
}

#[test]
fn nested_child_depth_above_old_cap_is_visible() {
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <TestDef>
    <defName>Deep</defName>
    <level1>
      <level2>
        <level3>
          <level4>
            <level5>
              <level6>deep_value</level6>
            </level5>
          </level4>
        </level3>
      </level2>
    </level1>
  </TestDef>
</Defs>"#;
    let doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());

    let view = build_editor_view(&doc);
    let level1 = view.defs[0]
        .children
        .iter()
        .find(|c| c.name == "level1")
        .unwrap();
    let level2 = level1
        .children
        .as_ref()
        .expect("level2 must be visible")
        .iter()
        .find(|c| c.name == "level2")
        .unwrap();
    let level3 = level2
        .children
        .as_ref()
        .expect("level3 must be visible")
        .iter()
        .find(|c| c.name == "level3")
        .unwrap();
    let level4 = level3
        .children
        .as_ref()
        .expect("level4 must be visible")
        .iter()
        .find(|c| c.name == "level4")
        .unwrap();
    let level5 = level4
        .children
        .as_ref()
        .expect("level5 must be visible - depth cap must be >= 5")
        .iter()
        .find(|c| c.name == "level5")
        .unwrap();
    let level6 = level5
        .children
        .as_ref()
        .expect("level6 must be visible - MAX_NESTED_CHILD_DEPTH must be >= 6")
        .iter()
        .find(|c| c.name == "level6")
        .unwrap();
    assert_eq!(
        level6.text_value.as_deref(),
        Some("deep_value"),
        "level6 text value must reach the view"
    );
}

#[test]
fn set_nested_element_text_field_order_positions_new_field_correctly() {
    // When field_order is provided, a newly created field is inserted at the correct position
    // relative to its siblings within the nested object. The parent_node_id is a list-item
    // node to simulate the item-relative edit path emitted by the TypeScript diffing layer.
    let src = r#"<?xml version="1.0" encoding="utf-8" ?>
<Defs>
  <SoundDef>
    <defName>Snd</defName>
    <subSounds>
      <li>
        <soundParams>
          <name>Boom</name>
          <volumeRange>0.8~1</volumeRange>
        </soundParams>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());

    let li_id = {
        let view = build_editor_view(&doc);
        let sub_sounds = view.defs[0]
            .children
            .iter()
            .find(|c| c.name == "subSounds")
            .unwrap();
        sub_sounds.li_items[0].node_id
    };

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: li_id,
            object_path: vec!["soundParams".to_string()],
            field_name: "newField".to_string(),
            value: "x".to_string(),
            field_order: vec![
                "name".to_string(),
                "newField".to_string(),
                "volumeRange".to_string(),
            ],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<newField>x</newField>"),
        "newField must be created: {out}"
    );
    let name_pos = out.find("<name>Boom</name>").unwrap();
    let new_pos = out.find("<newField>x</newField>").unwrap();
    let vol_pos = out.find("<volumeRange>0.8~1</volumeRange>").unwrap();
    assert!(name_pos < new_pos, "newField must come after <name>: {out}");
    assert!(
        new_pos < vol_pos,
        "newField must come before <volumeRange>: {out}"
    );
}
