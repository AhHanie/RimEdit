use serde::Serialize;

use super::model::{XmlDocument, XmlNodeId, XmlNodeKind};

pub(crate) const KNOWN_SCALAR_FIELDS: &[&str] = &[
    "packageId",
    "name",
    "shortName",
    "author",
    "modIconPath",
    "modVersion",
    "url",
    "description",
    "steamAppId",
    "targetVersion",
];

pub(crate) const KNOWN_LIST_FIELDS: &[&str] = &[
    "authors",
    "supportedVersions",
    "loadBefore",
    "loadAfter",
    "forceLoadBefore",
    "forceLoadAfter",
    "incompatibleWith",
];

pub(crate) const KNOWN_OBJECT_FIELDS: &[&str] = &["modDependencies"];

pub(crate) const KNOWN_VERSIONED_FIELDS: &[&str] = &[
    "descriptionsByVersion",
    "modDependenciesByVersion",
    "loadBeforeByVersion",
    "loadAfterByVersion",
    "incompatibleWithByVersion",
];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutScalarField {
    pub value: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutListField {
    pub items: Vec<String>,
    /// True when the container element exists in the XML, even if it has no `<li>` children.
    /// Distinguishes "present but empty" (invalid for `supportedVersions`) from "absent".
    pub present: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutDependency {
    pub node_id: XmlNodeId,
    pub package_id: Option<String>,
    pub alternative_package_ids: Vec<String>,
    pub display_name: Option<String>,
    pub download_url: Option<String>,
    pub steam_workshop_url: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutVersionedTextEntry {
    /// The raw XML element name used as the version key (e.g. `"v1.6"`).
    pub version: String,
    pub value: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutVersionedListEntry {
    pub version: String,
    pub items: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutVersionedDependenciesEntry {
    pub version: String,
    pub dependencies: Vec<AboutDependency>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutMetadataFields {
    pub package_id: AboutScalarField,
    pub name: AboutScalarField,
    pub short_name: AboutScalarField,
    pub author: AboutScalarField,
    pub authors: AboutListField,
    pub mod_icon_path: AboutScalarField,
    pub mod_version: AboutScalarField,
    pub url: AboutScalarField,
    pub description: AboutScalarField,
    pub steam_app_id: AboutScalarField,
    /// Obsolete field, kept only so the UI can show a warning and offer removal.
    pub target_version: AboutScalarField,
    pub supported_versions: AboutListField,
    pub load_before: AboutListField,
    pub load_after: AboutListField,
    pub force_load_before: AboutListField,
    pub force_load_after: AboutListField,
    pub incompatible_with: AboutListField,
    pub mod_dependencies: Vec<AboutDependency>,
    pub descriptions_by_version: Vec<AboutVersionedTextEntry>,
    pub mod_dependencies_by_version: Vec<AboutVersionedDependenciesEntry>,
    pub load_before_by_version: Vec<AboutVersionedListEntry>,
    pub load_after_by_version: Vec<AboutVersionedListEntry>,
    pub incompatible_with_by_version: Vec<AboutVersionedListEntry>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutUnknownElement {
    pub node_id: XmlNodeId,
    pub name: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutMetadataView {
    pub root_node_id: XmlNodeId,
    pub fields: AboutMetadataFields,
    pub unknown_children: Vec<AboutUnknownElement>,
}

pub(crate) fn find_root_element(doc: &XmlDocument) -> Option<XmlNodeId> {
    doc.top_level_nodes
        .iter()
        .copied()
        .find(|&id| matches!(doc.nodes[id].kind, XmlNodeKind::Element(_)))
}

pub(crate) fn element_name(doc: &XmlDocument, id: XmlNodeId) -> Option<&str> {
    match &doc.nodes[id].kind {
        XmlNodeKind::Element(el) => Some(el.name.as_str()),
        _ => None,
    }
}

fn find_child(doc: &XmlDocument, parent: XmlNodeId, name: &str) -> Option<XmlNodeId> {
    doc.nodes[parent]
        .children
        .iter()
        .copied()
        .find(|&id| element_name(doc, id) == Some(name))
}

pub(crate) fn find_children(doc: &XmlDocument, parent: XmlNodeId, name: &str) -> Vec<XmlNodeId> {
    doc.nodes[parent]
        .children
        .iter()
        .copied()
        .filter(|&id| element_name(doc, id) == Some(name))
        .collect()
}

pub(crate) fn count_children(doc: &XmlDocument, parent: XmlNodeId, name: &str) -> usize {
    find_children(doc, parent, name).len()
}

fn text_value(doc: &XmlDocument, id: XmlNodeId) -> Option<String> {
    let mut buf = String::new();
    for &child in &doc.nodes[id].children {
        match &doc.nodes[child].kind {
            XmlNodeKind::Text(t) | XmlNodeKind::CData(t) => buf.push_str(&t.value),
            _ => {}
        }
    }
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn li_texts(doc: &XmlDocument, list_id: XmlNodeId) -> Vec<String> {
    find_children(doc, list_id, "li")
        .into_iter()
        .filter_map(|id| text_value(doc, id))
        .collect()
}

fn scalar_field(doc: &XmlDocument, parent: XmlNodeId, name: &str) -> AboutScalarField {
    AboutScalarField {
        value: find_child(doc, parent, name).and_then(|id| text_value(doc, id)),
    }
}

fn list_field(doc: &XmlDocument, parent: XmlNodeId, name: &str) -> AboutListField {
    match find_child(doc, parent, name) {
        Some(id) => AboutListField {
            items: li_texts(doc, id),
            present: true,
        },
        None => AboutListField {
            items: vec![],
            present: false,
        },
    }
}

fn build_dependency(doc: &XmlDocument, li_id: XmlNodeId) -> AboutDependency {
    AboutDependency {
        node_id: li_id,
        package_id: find_child(doc, li_id, "packageId").and_then(|id| text_value(doc, id)),
        alternative_package_ids: find_child(doc, li_id, "alternativePackageIds")
            .map(|id| li_texts(doc, id))
            .unwrap_or_default(),
        display_name: find_child(doc, li_id, "displayName").and_then(|id| text_value(doc, id)),
        download_url: find_child(doc, li_id, "downloadUrl").and_then(|id| text_value(doc, id)),
        steam_workshop_url: find_child(doc, li_id, "steamWorkshopUrl")
            .and_then(|id| text_value(doc, id)),
    }
}

fn mod_dependencies(doc: &XmlDocument, parent: XmlNodeId) -> Vec<AboutDependency> {
    find_child(doc, parent, "modDependencies")
        .map(|id| {
            find_children(doc, id, "li")
                .into_iter()
                .map(|li| build_dependency(doc, li))
                .collect()
        })
        .unwrap_or_default()
}

fn versioned_text_entries(
    doc: &XmlDocument,
    parent: XmlNodeId,
    container_name: &str,
) -> Vec<AboutVersionedTextEntry> {
    let Some(container_id) = find_child(doc, parent, container_name) else {
        return vec![];
    };
    doc.nodes[container_id]
        .children
        .iter()
        .filter_map(|&id| {
            let name = element_name(doc, id)?.to_string();
            let value = text_value(doc, id).unwrap_or_default();
            Some(AboutVersionedTextEntry {
                version: name,
                value,
            })
        })
        .collect()
}

fn versioned_list_entries(
    doc: &XmlDocument,
    parent: XmlNodeId,
    container_name: &str,
) -> Vec<AboutVersionedListEntry> {
    let Some(container_id) = find_child(doc, parent, container_name) else {
        return vec![];
    };
    doc.nodes[container_id]
        .children
        .iter()
        .filter_map(|&id| {
            let name = element_name(doc, id)?.to_string();
            Some(AboutVersionedListEntry {
                version: name,
                items: li_texts(doc, id),
            })
        })
        .collect()
}

fn versioned_dependencies_entries(
    doc: &XmlDocument,
    parent: XmlNodeId,
    container_name: &str,
) -> Vec<AboutVersionedDependenciesEntry> {
    let Some(container_id) = find_child(doc, parent, container_name) else {
        return vec![];
    };
    doc.nodes[container_id]
        .children
        .iter()
        .filter_map(|&id| {
            let name = element_name(doc, id)?.to_string();
            let dependencies = find_children(doc, id, "li")
                .into_iter()
                .map(|li| build_dependency(doc, li))
                .collect();
            Some(AboutVersionedDependenciesEntry {
                version: name,
                dependencies,
            })
        })
        .collect()
}

fn is_known_top_level_name(name: &str) -> bool {
    KNOWN_SCALAR_FIELDS.contains(&name)
        || KNOWN_LIST_FIELDS.contains(&name)
        || KNOWN_OBJECT_FIELDS.contains(&name)
        || KNOWN_VERSIONED_FIELDS.contains(&name)
}

/// Builds a typed view of a `<ModMetaData>` document's fields, for the About.xml
/// editor UI. Returns `None` if the document has no root element or the root
/// element is not `ModMetaData` -- callers that need to surface a diagnostic for
/// a wrong root should do so via `validate_about_metadata_document`, which checks
/// this explicitly before calling into this builder.
pub fn build_about_metadata_view(doc: &XmlDocument) -> Option<AboutMetadataView> {
    let root_id = find_root_element(doc)?;
    if element_name(doc, root_id) != Some("ModMetaData") {
        return None;
    }

    let unknown_children = doc.nodes[root_id]
        .children
        .iter()
        .filter_map(|&id| {
            let name = element_name(doc, id)?;
            if is_known_top_level_name(name) {
                return None;
            }
            Some(AboutUnknownElement {
                node_id: id,
                name: name.to_string(),
                line: Some(doc.nodes[id].span.line),
                column: Some(doc.nodes[id].span.column),
            })
        })
        .collect();

    let fields = AboutMetadataFields {
        package_id: scalar_field(doc, root_id, "packageId"),
        name: scalar_field(doc, root_id, "name"),
        short_name: scalar_field(doc, root_id, "shortName"),
        author: scalar_field(doc, root_id, "author"),
        authors: list_field(doc, root_id, "authors"),
        mod_icon_path: scalar_field(doc, root_id, "modIconPath"),
        mod_version: scalar_field(doc, root_id, "modVersion"),
        url: scalar_field(doc, root_id, "url"),
        description: scalar_field(doc, root_id, "description"),
        steam_app_id: scalar_field(doc, root_id, "steamAppId"),
        target_version: scalar_field(doc, root_id, "targetVersion"),
        supported_versions: list_field(doc, root_id, "supportedVersions"),
        load_before: list_field(doc, root_id, "loadBefore"),
        load_after: list_field(doc, root_id, "loadAfter"),
        force_load_before: list_field(doc, root_id, "forceLoadBefore"),
        force_load_after: list_field(doc, root_id, "forceLoadAfter"),
        incompatible_with: list_field(doc, root_id, "incompatibleWith"),
        mod_dependencies: mod_dependencies(doc, root_id),
        descriptions_by_version: versioned_text_entries(doc, root_id, "descriptionsByVersion"),
        mod_dependencies_by_version: versioned_dependencies_entries(
            doc,
            root_id,
            "modDependenciesByVersion",
        ),
        load_before_by_version: versioned_list_entries(doc, root_id, "loadBeforeByVersion"),
        load_after_by_version: versioned_list_entries(doc, root_id, "loadAfterByVersion"),
        incompatible_with_by_version: versioned_list_entries(
            doc,
            root_id,
            "incompatibleWithByVersion",
        ),
    };

    Some(AboutMetadataView {
        root_node_id: root_id,
        fields,
        unknown_children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml_document::parse_to_document;

    #[test]
    fn parses_core_scalar_and_list_fields() {
        let xml = r#"<ModMetaData>
  <packageId>brrainz.harmony</packageId>
  <name>Harmony</name>
  <author>Brrainz</author>
  <supportedVersions>
    <li>1.5</li>
    <li>1.6</li>
  </supportedVersions>
</ModMetaData>"#;
        let doc = parse_to_document("About/About.xml", xml);
        let view = build_about_metadata_view(&doc).expect("view should build");

        assert_eq!(
            view.fields.package_id.value.as_deref(),
            Some("brrainz.harmony")
        );
        assert_eq!(view.fields.name.value.as_deref(), Some("Harmony"));
        assert!(view.fields.short_name.value.is_none());
        assert!(view.fields.supported_versions.present);
        assert_eq!(
            view.fields.supported_versions.items,
            vec!["1.5".to_string(), "1.6".to_string()]
        );
        assert!(!view.fields.authors.present);
        assert!(view.unknown_children.is_empty());
    }

    #[test]
    fn parses_dependencies_with_alternative_package_ids() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <modDependencies>
    <li>
      <packageId>brrainz.harmony</packageId>
      <displayName>Harmony</displayName>
      <downloadUrl>https://example.com</downloadUrl>
      <alternativePackageIds>
        <li>harmony.old.id</li>
      </alternativePackageIds>
    </li>
  </modDependencies>
</ModMetaData>"#;
        let doc = parse_to_document("About/About.xml", xml);
        let view = build_about_metadata_view(&doc).expect("view should build");

        assert_eq!(view.fields.mod_dependencies.len(), 1);
        let dep = &view.fields.mod_dependencies[0];
        assert_eq!(dep.package_id.as_deref(), Some("brrainz.harmony"));
        assert_eq!(dep.display_name.as_deref(), Some("Harmony"));
        assert_eq!(dep.download_url.as_deref(), Some("https://example.com"));
        assert_eq!(
            dep.alternative_package_ids,
            vec!["harmony.old.id".to_string()]
        );
    }

    #[test]
    fn parses_versioned_overrides_and_unknown_elements() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <descriptionsByVersion>
    <v1.6>1.6 specific description</v1.6>
  </descriptionsByVersion>
  <loadBeforeByVersion>
    <v1.5>
      <li>SomeMod</li>
    </v1.5>
  </loadBeforeByVersion>
  <someWeirdField>surprise</someWeirdField>
</ModMetaData>"#;
        let doc = parse_to_document("About/About.xml", xml);
        let view = build_about_metadata_view(&doc).expect("view should build");

        assert_eq!(view.fields.descriptions_by_version.len(), 1);
        assert_eq!(view.fields.descriptions_by_version[0].version, "v1.6");
        assert_eq!(
            view.fields.descriptions_by_version[0].value,
            "1.6 specific description"
        );

        assert_eq!(view.fields.load_before_by_version.len(), 1);
        assert_eq!(
            view.fields.load_before_by_version[0].items,
            vec!["SomeMod".to_string()]
        );

        assert_eq!(view.unknown_children.len(), 1);
        assert_eq!(view.unknown_children[0].name, "someWeirdField");
    }

    #[test]
    fn returns_none_for_non_mod_meta_data_root() {
        let xml = "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>";
        let doc = parse_to_document("About/About.xml", xml);
        assert!(build_about_metadata_view(&doc).is_none());
    }
}
