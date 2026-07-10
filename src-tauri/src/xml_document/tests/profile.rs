use super::*;
use crate::xml_document::model::XmlDocumentProfile;

#[test]
fn about_xml_path_with_mod_meta_data_root_is_about_profile() {
    let src = "<ModMetaData><packageId>foo.bar</packageId></ModMetaData>";
    let doc = parse_to_document("About/About.xml", src);
    assert_eq!(doc.profile, XmlDocumentProfile::About);
    assert!(
        doc.def_summaries.is_empty(),
        "ModMetaData must not be treated as a Def candidate"
    );
}

#[test]
fn mod_meta_data_root_is_about_profile_regardless_of_path() {
    // Detection also triggers on the root element alone, so a ModMetaData document
    // opened from a nonstandard relative path is still routed correctly.
    let src = "<ModMetaData><packageId>foo.bar</packageId></ModMetaData>";
    let doc = parse_to_document("weird/location.xml", src);
    assert_eq!(doc.profile, XmlDocumentProfile::About);
}

#[test]
fn about_path_with_wrong_root_is_still_about_profile() {
    // Path match alone is enough so a malformed About.xml still routes through
    // About validation (which reports "root is not ModMetaData") instead of
    // silently falling through to the generic Def-candidate path.
    let src = "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>";
    let doc = parse_to_document("About/About.xml", src);
    assert_eq!(doc.profile, XmlDocumentProfile::About);
}

#[test]
fn patch_root_is_patch_profile() {
    let src = "<Patch><Operation Class=\"PatchOperationAdd\"></Operation></Patch>";
    let doc = parse_to_document("Patches/foo.xml", src);
    assert_eq!(doc.profile, XmlDocumentProfile::Patch);
}

#[test]
fn defs_wrapper_root_is_defs_profile() {
    let src = "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>";
    let doc = parse_to_document("Defs/rock.xml", src);
    assert_eq!(doc.profile, XmlDocumentProfile::Defs);
    assert_eq!(doc.def_summaries.len(), 1);
}

#[test]
fn standalone_def_root_is_still_defs_profile() {
    // A lone Def without a <Defs> wrapper keeps its existing "single Def candidate"
    // behavior -- only ModMetaData/Patch roots are special-cased.
    let doc = parse_to_document("Defs/rock.xml", SINGLE_DEF_XML);
    assert_eq!(doc.profile, XmlDocumentProfile::Defs);
    assert_eq!(doc.def_summaries.len(), 1);
}

#[test]
fn editor_view_exposes_about_metadata_for_about_profile() {
    let src = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <name>Foo</name>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
    let doc = parse_to_document("About/About.xml", src);
    let view = build_editor_view(&doc);
    assert_eq!(view.profile, XmlDocumentProfile::About);
    assert!(view.defs.is_empty());
    let about = view.about.expect("about view should be present");
    assert_eq!(about.fields.package_id.value.as_deref(), Some("foo.bar"));
    assert_eq!(about.fields.name.value.as_deref(), Some("Foo"));
}

#[test]
fn editor_view_omits_about_metadata_for_defs_profile() {
    let doc = parse_to_document(
        "Defs/rock.xml",
        "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>",
    );
    let view = build_editor_view(&doc);
    assert_eq!(view.profile, XmlDocumentProfile::Defs);
    assert!(view.about.is_none());
}
