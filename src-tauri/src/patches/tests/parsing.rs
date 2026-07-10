use super::*;

fn only_op(source: &str) -> PatchOperationNode {
    let file = parse_patch_file("test.xml", source);
    assert!(
        file.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        file.diagnostics
    );
    assert_eq!(file.operations.len(), 1);
    file.operations.into_iter().next().unwrap()
}

#[test]
fn parses_add_operation() {
    let op = only_op(ADD_XML);
    assert_eq!(op.class_name, "PatchOperationAdd");
    assert_eq!(
        op.attributes,
        vec![XmlAttributeModel {
            name: "MayRequire".to_string(),
            value: "SomeMod.PackageId".to_string(),
        }]
    );
    match op.kind {
        PatchOperationKind::Add(inner) => {
            assert_eq!(
                inner.xpath.as_deref(),
                Some(r#"Defs/ThingDef[defName="Wall"]/statBases"#)
            );
            assert!(inner
                .value_xml
                .unwrap()
                .contains("<MoveSpeed>1</MoveSpeed>"));
            assert_eq!(inner.order, None);
        }
        other => panic!("expected Add, got {:?}", other),
    }
}

#[test]
fn parses_insert_operation_with_explicit_order() {
    let op = only_op(INSERT_XML);
    match op.kind {
        PatchOperationKind::Insert(inner) => {
            assert!(inner.xpath.unwrap().ends_with("li[last()]"));
            assert!(inner.value_xml.unwrap().contains("<li>Steel</li>"));
            assert_eq!(inner.order, Some(PatchOrderMode::Append));
        }
        other => panic!("expected Insert, got {:?}", other),
    }
}

#[test]
fn parses_remove_operation() {
    let op = only_op(REMOVE_XML);
    match op.kind {
        PatchOperationKind::Remove(inner) => {
            assert_eq!(
                inner.xpath.as_deref(),
                Some(r#"Defs/ThingDef[defName="Wall"]/comps"#)
            );
        }
        other => panic!("expected Remove, got {:?}", other),
    }
}

#[test]
fn parses_replace_operation() {
    let op = only_op(REPLACE_XML);
    match op.kind {
        PatchOperationKind::Replace(inner) => {
            assert!(inner
                .value_xml
                .unwrap()
                .contains("<label>reinforced wall</label>"));
        }
        other => panic!("expected Replace, got {:?}", other),
    }
}

#[test]
fn parses_attribute_add_operation() {
    let op = only_op(ATTRIBUTE_ADD_XML);
    match op.kind {
        PatchOperationKind::AttributeAdd(inner) => {
            assert_eq!(inner.attribute.as_deref(), Some("Abstract"));
            assert_eq!(inner.value.as_deref(), Some("True"));
        }
        other => panic!("expected AttributeAdd, got {:?}", other),
    }
}

#[test]
fn parses_attribute_set_operation() {
    let op = only_op(ATTRIBUTE_SET_XML);
    match op.kind {
        PatchOperationKind::AttributeSet(inner) => {
            assert_eq!(inner.attribute.as_deref(), Some("ParentName"));
            assert_eq!(inner.value.as_deref(), Some("BaseWall"));
        }
        other => panic!("expected AttributeSet, got {:?}", other),
    }
}

#[test]
fn parses_attribute_remove_operation() {
    let op = only_op(ATTRIBUTE_REMOVE_XML);
    match op.kind {
        PatchOperationKind::AttributeRemove(inner) => {
            assert_eq!(inner.attribute.as_deref(), Some("ParentName"));
        }
        other => panic!("expected AttributeRemove, got {:?}", other),
    }
}

#[test]
fn parses_add_mod_extension_operation() {
    let op = only_op(ADD_MOD_EXTENSION_XML);
    match op.kind {
        PatchOperationKind::AddModExtension(inner) => {
            assert!(inner
                .value_xml
                .unwrap()
                .contains(r#"<li Class="SomeMod.ThingExtension">"#));
        }
        other => panic!("expected AddModExtension, got {:?}", other),
    }
}

#[test]
fn parses_set_name_operation() {
    let op = only_op(SET_NAME_XML);
    match op.kind {
        PatchOperationKind::SetName(inner) => {
            assert_eq!(inner.name.as_deref(), Some("costs"));
        }
        other => panic!("expected SetName, got {:?}", other),
    }
}

#[test]
fn parses_test_operation() {
    let op = only_op(TEST_OPERATION_XML);
    assert!(matches!(op.kind, PatchOperationKind::Test(_)));
}

#[test]
fn parses_sequence_with_stable_ids() {
    let op = only_op(SEQUENCE_XML);
    assert_eq!(op.id, 0);
    assert_eq!(op.success, PatchSuccessMode::Always);
    match op.kind {
        PatchOperationKind::Sequence(children) => {
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].id, 1);
            assert!(matches!(children[0].kind, PatchOperationKind::Test(_)));
            assert_eq!(children[1].id, 2);
            assert!(matches!(children[1].kind, PatchOperationKind::Add(_)));
        }
        other => panic!("expected Sequence, got {:?}", other),
    }
}

#[test]
fn parses_find_mod_with_match_and_nomatch() {
    let op = only_op(FIND_MOD_XML);
    match op.kind {
        PatchOperationKind::FindMod {
            mods,
            match_op,
            nomatch_op,
        } => {
            assert_eq!(
                mods,
                vec![
                    "Humanoid Alien Races".to_string(),
                    "Alien Vs Predator".to_string()
                ]
            );
            assert!(matches!(
                match_op.unwrap().kind,
                PatchOperationKind::Remove(_)
            ));
            assert!(matches!(
                nomatch_op.unwrap().kind,
                PatchOperationKind::Test(_)
            ));
        }
        other => panic!("expected FindMod, got {:?}", other),
    }
}

#[test]
fn parses_conditional_match_only() {
    let op = only_op(CONDITIONAL_MATCH_ONLY_XML);
    match op.kind {
        PatchOperationKind::Conditional {
            match_op,
            nomatch_op,
            ..
        } => {
            assert!(match_op.is_some());
            assert!(nomatch_op.is_none());
        }
        other => panic!("expected Conditional, got {:?}", other),
    }
}

#[test]
fn parses_conditional_nomatch_only() {
    let op = only_op(CONDITIONAL_NOMATCH_ONLY_XML);
    match op.kind {
        PatchOperationKind::Conditional {
            match_op,
            nomatch_op,
            ..
        } => {
            assert!(match_op.is_none());
            assert!(nomatch_op.is_some());
        }
        other => panic!("expected Conditional, got {:?}", other),
    }
}

#[test]
fn parses_conditional_both() {
    let op = only_op(CONDITIONAL_BOTH_XML);
    match op.kind {
        PatchOperationKind::Conditional {
            match_op,
            nomatch_op,
            ..
        } => {
            assert!(match_op.is_some());
            assert!(nomatch_op.is_some());
        }
        other => panic!("expected Conditional, got {:?}", other),
    }
}

#[test]
fn parses_custom_operation_as_unknown_and_preserves_raw_xml() {
    let op = only_op(CUSTOM_OPERATION_XML);
    assert_eq!(op.class_name, "MyMod.PatchOperationFoo");
    assert!(!op.is_known_class());
    match op.kind {
        PatchOperationKind::Unknown(unknown) => {
            assert!(unknown.raw_xml.contains("<customField>"));
            assert!(unknown.raw_xml.contains("<nested>value</nested>"));
            assert!(unknown.raw_xml.contains(r#"MayRequire="MyMod.PackageId""#));
        }
        other => panic!("expected Unknown, got {:?}", other),
    }
}
