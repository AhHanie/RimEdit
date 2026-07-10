use super::*;

#[test]
fn set_list_items_replaces_li_children() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <ingredients>
      <li>OldItem</li>
    </ingredients>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetListItems {
            parent_node_id: def_id,
            child_name: "ingredients".to_string(),
            items: vec!["Alpha".to_string(), "Beta".to_string()],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(!out.contains("OldItem"), "old item must be removed");
    assert!(out.contains("<li>Alpha</li>"));
    assert!(out.contains("<li>Beta</li>"));
    // Order: Alpha before Beta
    assert!(out.find("<li>Alpha</li>").unwrap() < out.find("<li>Beta</li>").unwrap());
}

#[test]
fn set_nested_list_items_replaces_existing_items() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <linkFlags>
        <li>Wall</li>
        <li>Rock</li>
      </linkFlags>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedListItems {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "linkFlags".to_string(),
            items: vec!["Wall".to_string(), "PowerConduit".to_string()],
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<li>Rock</li>"),
        "old item must be removed: {}",
        out
    );
    assert!(
        out.contains("<li>Wall</li>"),
        "Wall must be present: {}",
        out
    );
    assert!(
        out.contains("<li>PowerConduit</li>"),
        "PowerConduit must be present: {}",
        out
    );
    let wall_pos = out.find("<li>Wall</li>").unwrap();
    let conduit_pos = out.find("<li>PowerConduit</li>").unwrap();
    assert!(
        wall_pos < conduit_pos,
        "Wall must precede PowerConduit: {}",
        out
    );
}

#[test]
fn set_nested_list_items_creates_missing_parent_objects() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedListItems {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            field_name: "linkFlags".to_string(),
            items: vec!["Wall".to_string()],
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
        out.contains("<linkFlags>"),
        "linkFlags must be created: {}",
        out
    );
    assert!(
        out.contains("<li>Wall</li>"),
        "item must be present: {}",
        out
    );

    let gfx_open = out.find("<graphicData>").unwrap();
    let flags_open = out.find("<linkFlags>").unwrap();
    let li_pos = out.find("<li>Wall</li>").unwrap();
    let flags_close = out.find("</linkFlags>").unwrap();
    let gfx_close = out.find("</graphicData>").unwrap();
    assert!(gfx_open < flags_open);
    assert!(flags_open < li_pos);
    assert!(li_pos < flags_close);
    assert!(flags_close < gfx_close);
}

#[test]
fn set_list_items_on_self_closing_container_expands_to_open_close_tags() {
    // Simulates <tradeTags Inherit="False" /> - a self-closing list container with an attribute.
    // Adding items must produce <tradeTags Inherit="False"><li>X</li></tradeTags>, not discard them.
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <tradeTags Inherit="False" />
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetListItems {
            parent_node_id: def_id,
            child_name: "tradeTags".to_string(),
            items: vec!["WeaponMelee".to_string(), "Apparel".to_string()],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<li>WeaponMelee</li>"),
        "first item must appear: {out}"
    );
    assert!(
        out.contains("<li>Apparel</li>"),
        "second item must appear: {out}"
    );
    assert!(!out.contains("/>"), "self-closing form must be gone: {out}");
    // Inherit attribute must be preserved.
    assert!(
        out.contains("Inherit="),
        "Inherit attribute must be preserved: {out}"
    );
    let open_pos = out.find("<tradeTags").unwrap();
    let close_pos = out.find("</tradeTags>").unwrap();
    assert!(open_pos < close_pos, "must have open/close tags: {out}");
}

#[test]
fn set_list_items_on_self_closing_container_empty_list_preserves_attributes() {
    // Setting an empty list on a self-closing element must expand the tag but remain empty.
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <tradeTags Inherit="False" />
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetListItems {
            parent_node_id: def_id,
            child_name: "tradeTags".to_string(),
            items: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    // No items should be emitted, but the element should still expand.
    assert!(!out.contains("<li>"), "no li items expected: {out}");
    assert!(
        out.contains("Inherit="),
        "Inherit attribute must survive: {out}"
    );
}
