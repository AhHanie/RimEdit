use super::*;

#[test]
fn set_named_map_entry_preserves_arbitrary_child_name() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <shaderParameters>
        <_DistortionStrength>0.35</_DistortionStrength>
      </shaderParameters>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    // Set a new key.
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNamedMapEntry {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            map_name: "shaderParameters".to_string(),
            key: "_DistortionTex".to_string(),
            value: "/Other/Ripples".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<_DistortionTex>/Other/Ripples</_DistortionTex>"),
        "new entry must be added: {}",
        out
    );
    assert!(
        out.contains("<_DistortionStrength>0.35</_DistortionStrength>"),
        "existing entry must be preserved: {}",
        out
    );

    // Update existing key.
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNamedMapEntry {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            map_name: "shaderParameters".to_string(),
            key: "_DistortionStrength".to_string(),
            value: "0.75".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out2 = serialize_xml_document(&doc);
    assert!(
        out2.contains("<_DistortionStrength>0.75</_DistortionStrength>"),
        "entry must be updated: {}",
        out2
    );
    assert!(!out2.contains(">0.35<"), "old value must be gone: {}", out2);
}

#[test]
fn remove_named_map_entry_removes_only_target_key() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <shaderParameters>
        <_DistortionTex>/Other/Ripples</_DistortionTex>
        <_DistortionStrength>0.35</_DistortionStrength>
      </shaderParameters>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNamedMapEntry {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            map_name: "shaderParameters".to_string(),
            key: "_DistortionTex".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<_DistortionTex>"),
        "removed entry must be gone: {}",
        out
    );
    assert!(
        out.contains("<_DistortionStrength>0.35</_DistortionStrength>"),
        "sibling entry must be preserved: {}",
        out
    );

    // Removing again must be idempotent.
    apply_xml_edit(
        &mut doc,
        XmlEdit::RemoveNamedMapEntry {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            map_name: "shaderParameters".to_string(),
            key: "_DistortionTex".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();
}

#[test]
fn rename_named_map_entry_changes_element_name_and_preserves_value() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <shaderParameters>
        <_OldParam>0.5</_OldParam>
      </shaderParameters>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    apply_xml_edit(
        &mut doc,
        XmlEdit::RenameNamedMapEntry {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            map_name: "shaderParameters".to_string(),
            old_key: "_OldParam".to_string(),
            new_key: "_NewParam".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<_OldParam>"),
        "old element name must be gone: {}",
        out
    );
    assert!(
        out.contains("<_NewParam>0.5</_NewParam>"),
        "renamed entry must have old value: {}",
        out
    );
}

#[test]
fn rename_named_map_entry_rejects_duplicate_key() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <graphicData>
      <shaderParameters>
        <_ParamA>0.5</_ParamA>
        <_ParamB>1.0</_ParamB>
      </shaderParameters>
    </graphicData>
  </ThingDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    let result = apply_xml_edit(
        &mut doc,
        XmlEdit::RenameNamedMapEntry {
            parent_node_id: def_id,
            object_path: vec!["graphicData".to_string()],
            map_name: "shaderParameters".to_string(),
            old_key: "_ParamA".to_string(),
            new_key: "_ParamB".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    );

    assert!(
        matches!(result, Err(XmlEditError::DuplicateMapKey(_))),
        "expected DuplicateMapKey error, got: {:?}",
        result
    );
}

#[test]
fn replace_keyed_value_list_entries_supports_duplicate_keys() {
    let src = r#"<Defs>
  <ThoughtDef>
    <defName>X</defName>
    <nullifyingTraitDegrees>
      <DrugDesire>0</DrugDesire>
    </nullifyingTraitDegrees>
  </ThoughtDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    // Replace with two entries for the same key - not allowed by SetNamedMapEntry but
    // valid for repeatable (keyedValueList) fields.
    apply_xml_edit(
        &mut doc,
        XmlEdit::ReplaceKeyedValueListEntries {
            parent_node_id: def_id,
            object_path: vec![],
            map_name: "nullifyingTraitDegrees".to_string(),
            entries: vec![
                KeyValuePair {
                    key: "DrugDesire".to_string(),
                    value: "0".to_string(),
                },
                KeyValuePair {
                    key: "DrugDesire".to_string(),
                    value: "1".to_string(),
                },
                KeyValuePair {
                    key: "Psychopath".to_string(),
                    value: "0".to_string(),
                },
            ],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    // Both DrugDesire entries should be present (duplicate keys allowed).
    let count = out.matches("<DrugDesire>").count();
    assert_eq!(
        count, 2,
        "expected 2 DrugDesire entries, got {count}: {out}"
    );
    assert!(
        out.contains("<Psychopath>0</Psychopath>"),
        "Psychopath entry must be present: {out}"
    );
}

#[test]
fn replace_keyed_value_list_entries_clears_all_existing_children() {
    let src = r#"<Defs>
  <ThoughtDef>
    <defName>X</defName>
    <nullifyingTraitDegrees>
      <OldTrait>0</OldTrait>
      <AnotherTrait>1</AnotherTrait>
    </nullifyingTraitDegrees>
  </ThoughtDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    // Replace with an entirely different set.
    apply_xml_edit(
        &mut doc,
        XmlEdit::ReplaceKeyedValueListEntries {
            parent_node_id: def_id,
            object_path: vec![],
            map_name: "nullifyingTraitDegrees".to_string(),
            entries: vec![KeyValuePair {
                key: "NewTrait".to_string(),
                value: "0".to_string(),
            }],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        !out.contains("<OldTrait>"),
        "OldTrait must be removed: {out}"
    );
    assert!(
        !out.contains("<AnotherTrait>"),
        "AnotherTrait must be removed: {out}"
    );
    assert!(
        out.contains("<NewTrait>0</NewTrait>"),
        "NewTrait must be present: {out}"
    );
}

#[test]
fn replace_keyed_value_list_entries_empty_value_produces_empty_element() {
    // When the frontend saves a keyed-value-list entry whose value is the schema default
    // (represented as an empty string), the serialized output must contain an empty element
    // rather than any non-empty text content.  Re-parsing that output must then succeed.
    let src = r#"<Defs>
  <CreepJoinerBenefitDef>
    <defName>X</defName>
    <skills>
      <Shooting>14~18</Shooting>
    </skills>
  </CreepJoinerBenefitDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    let def_id = doc.def_summaries[0].node_id;

    // Save with empty string value (user cleared the field; schema default applies).
    apply_xml_edit(
        &mut doc,
        XmlEdit::ReplaceKeyedValueListEntries {
            parent_node_id: def_id,
            object_path: vec![],
            map_name: "skills".to_string(),
            entries: vec![KeyValuePair {
                key: "Shooting".to_string(),
                value: "".to_string(),
            }],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    // The empty value must not produce a non-empty text node like <Shooting>0~0</Shooting>.
    assert!(
        !out.contains("<Shooting>14~18</Shooting>"),
        "old value must be gone: {out}"
    );
    // The element must still be present.
    assert!(
        out.contains("<Shooting"),
        "Shooting key must still be present: {out}"
    );
    // The serialized XML must be parseable without error.
    let reparsed = parse_to_document("test.xml", &out);
    assert_eq!(
        reparsed.def_summaries.len(),
        1,
        "re-parsed doc must have one def: {out}"
    );
}
