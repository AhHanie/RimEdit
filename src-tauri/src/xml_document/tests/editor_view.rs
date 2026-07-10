use super::*;

#[test]
fn build_editor_view_extracts_element_children() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Steel</defName>
    <label>steel</label>
    <description>A material.</description>
  </ThingDef>
</Defs>"#;
    let doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let view = build_editor_view(&doc);
    assert_eq!(view.defs.len(), 1);
    let def = &view.defs[0];
    assert_eq!(def.def_type, "ThingDef");
    assert_eq!(def.def_name.as_deref(), Some("Steel"));
    assert_eq!(def.children.len(), 3);
    let child_names: Vec<&str> = def.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(child_names, ["defName", "label", "description"]);
    assert!(matches!(def.children[0].xml_shape, XmlChildShape::Element));
    assert_eq!(def.children[0].text_value.as_deref(), Some("Steel"));
}

#[test]
fn build_editor_view_classifies_list_of_li() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <ingredients>
      <li>Steel</li>
      <li>Wood</li>
    </ingredients>
  </ThingDef>
</Defs>"#;
    let doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let view = build_editor_view(&doc);
    let def = &view.defs[0];
    let ingr = def
        .children
        .iter()
        .find(|c| c.name == "ingredients")
        .unwrap();
    assert!(matches!(ingr.xml_shape, XmlChildShape::ListOfLi));
    assert!(ingr.text_value.is_none());
    assert_eq!(ingr.list_items, vec!["Steel", "Wood"]);
}

#[test]
fn build_editor_view_classifies_object() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/Steel</texPath>
      <graphicClass>Graphic_Single</graphicClass>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let doc = parse_to_document("test.xml", src);
    let view = build_editor_view(&doc);
    let def = &view.defs[0];
    let gfx = def
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    assert!(matches!(gfx.xml_shape, XmlChildShape::Object));
    assert!(gfx.text_value.is_none());
}

#[test]
fn build_editor_view_exposes_nested_object_children() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <texPath>Items/Steel</texPath>
      <graphicClass>Graphic_Single</graphicClass>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let view = build_editor_view(&doc);
    let def = &view.defs[0];
    let gfx = def
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    assert!(matches!(gfx.xml_shape, XmlChildShape::Object));
    let nested = gfx
        .children
        .as_ref()
        .expect("graphicData should have children");
    let names: Vec<&str> = nested.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["texPath", "graphicClass"]);
    let tex = nested.iter().find(|c| c.name == "texPath").unwrap();
    assert_eq!(tex.text_value.as_deref(), Some("Items/Steel"));
    let gfx_class = nested.iter().find(|c| c.name == "graphicClass").unwrap();
    assert_eq!(gfx_class.text_value.as_deref(), Some("Graphic_Single"));
    assert!(tex.line.is_some());
    assert!(tex.column.is_some());
}

#[test]
fn build_editor_view_exposes_deep_nested_object_children() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <shadowData>
        <volume>(0.8, 0, 0.6)</volume>
      </shadowData>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let doc = parse_to_document("test.xml", src);
    assert!(doc.parse_diagnostics.is_empty());
    let view = build_editor_view(&doc);
    let def = &view.defs[0];
    let gfx = def
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    let nested = gfx
        .children
        .as_ref()
        .expect("graphicData should have children");
    let shadow = nested.iter().find(|c| c.name == "shadowData").unwrap();
    assert!(matches!(shadow.xml_shape, XmlChildShape::Object));
    let shadow_children = shadow
        .children
        .as_ref()
        .expect("shadowData should have children");
    let volume = shadow_children.iter().find(|c| c.name == "volume").unwrap();
    assert!(matches!(volume.xml_shape, XmlChildShape::Element));
    assert_eq!(volume.text_value.as_deref(), Some("(0.8, 0, 0.6)"));
}

#[test]
fn object_list_view_exposes_li_children() {
    let doc = parse_to_document("test.xml", GRAPHIC_DATA_009_XML);
    assert!(
        doc.parse_diagnostics.is_empty(),
        "unexpected parse errors: {:?}",
        doc.parse_diagnostics
    );
    let view = build_editor_view(&doc);
    let def = &view.defs[0];

    let gfx = def
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .unwrap();
    let nested = gfx
        .children
        .as_ref()
        .expect("graphicData should have children");

    // attachments: object list - li_object_items should expose the li's children.
    let attachments = nested
        .iter()
        .find(|c| c.name == "attachments")
        .expect("attachments");
    assert!(
        matches!(attachments.xml_shape, XmlChildShape::ListOfLi),
        "attachments must be ListOfLi"
    );
    assert!(
        !attachments.li_object_items.is_empty(),
        "attachments must have li_object_items"
    );
    let first_li = &attachments.li_object_items[0];
    assert!(
        !first_li.is_empty(),
        "first <li> in attachments must expose element children"
    );
    let names: Vec<&str> = first_li.iter().map(|c| c.name.as_str()).collect();
    assert!(
        names.contains(&"texPath"),
        "texPath must be in li children: {:?}",
        names
    );

    // linkFlags: scalar list - li_object_items should be empty vecs for each item.
    let link_flags = nested
        .iter()
        .find(|c| c.name == "linkFlags")
        .expect("linkFlags");
    assert!(
        matches!(link_flags.xml_shape, XmlChildShape::ListOfLi),
        "linkFlags must be ListOfLi"
    );
    assert!(
        !link_flags.li_object_items.is_empty(),
        "linkFlags must have li_object_items"
    );
    for slot in &link_flags.li_object_items {
        assert!(
            slot.is_empty(),
            "scalar <li> items must yield empty inner vecs"
        );
    }

    // damageData.scratches: scalar list - li_object_items should be empty vecs.
    let damage_data = nested
        .iter()
        .find(|c| c.name == "damageData")
        .expect("damageData");
    let damage_nested = damage_data
        .children
        .as_ref()
        .expect("damageData must have children");
    let scratches = damage_nested
        .iter()
        .find(|c| c.name == "scratches")
        .expect("scratches");
    assert!(
        matches!(scratches.xml_shape, XmlChildShape::ListOfLi),
        "scratches must be ListOfLi"
    );
    for slot in &scratches.li_object_items {
        assert!(
            slot.is_empty(),
            "scalar scratch items must yield empty inner vecs"
        );
    }
}
