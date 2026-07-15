use crate::diagnostics::diagnostic_args;
use crate::xml_document::about::{
    build_about_metadata_view, count_children, element_name, find_root_element, KNOWN_SCALAR_FIELDS,
};
use crate::xml_document::diagnostics::ValidationDiagnostic;
use crate::xml_document::model::{XmlDocument, XmlNodeId};

fn node_line_col(doc: &XmlDocument, id: XmlNodeId) -> (Option<usize>, Option<usize>) {
    doc.nodes
        .get(id)
        .map(|n| (Some(n.span.line), Some(n.span.column)))
        .unwrap_or((None, None))
}

fn error_at(
    doc: &XmlDocument,
    node_id: XmlNodeId,
    code: &str,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    let (line, column) = node_line_col(doc, node_id);
    ValidationDiagnostic::error(
        doc.relative_path.clone(),
        Some(node_id),
        line,
        column,
        code,
        message,
    )
}

fn warning_at(
    doc: &XmlDocument,
    node_id: XmlNodeId,
    code: &str,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    let (line, column) = node_line_col(doc, node_id);
    ValidationDiagnostic::warning(
        doc.relative_path.clone(),
        Some(node_id),
        line,
        column,
        code,
        message,
    )
}

/// Mirrors RimWorld's `ModMetaData` packageId regex:
/// `(?=.{1,60}$)^(?!\.)(?=.*?[.])(?!.*([.])\1+)[a-zA-Z0-9.]{1,}[a-zA-Z0-9]{1}$`
/// -- 1 to 60 chars, alphanumeric-and-dot only, doesn't start with a dot, has at
/// least one dot, no consecutive dots, and ends with an alphanumeric character.
fn is_valid_package_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 60 {
        return false;
    }
    if id.starts_with('.') {
        return false;
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
        return false;
    }
    if !id.contains('.') || id.contains("..") {
        return false;
    }
    matches!(id.chars().last(), Some(c) if c.is_ascii_alphanumeric())
}

fn is_ludeon_package_id(package_id: &str) -> bool {
    let lower = package_id.to_ascii_lowercase();
    lower == "ludeon.rimworld" || lower.starts_with("ludeon.rimworld.")
}

/// Strict `Major.Minor` parse: exactly two non-negative integer components.
fn parse_version_strict(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    Some((major, minor))
}

fn strip_leading_v(key: &str) -> &str {
    key.strip_prefix('v')
        .or_else(|| key.strip_prefix('V'))
        .unwrap_or(key)
}

/// A versioned-override element name (e.g. `v1.6`) is valid if, after stripping an
/// optional leading `v`/`V`, it parses as strict `Major.Minor`.
fn is_valid_versioned_key(key: &str) -> bool {
    parse_version_strict(strip_leading_v(key)).is_some()
}

/// Lenient `Major.Minor` extraction: takes the first two dot-separated numeric
/// components and ignores any further ones (build/revision, e.g. `1.6.4491`).
/// Used only for the LoadFolders cross-check, which -- like RimWorld's own
/// `LoadFolders.xml` resolver (`rimworld_load_folders::parse_key_to_major_minor`)
/// -- treats `v1.6.4491` and `1.6` as the same version.
fn parse_major_minor_loose(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    Some((major, minor))
}

/// Validates a parsed `About.xml` (`ModMetaData`) document against RimWorld's
/// runtime expectations. `load_folders_versions` is the set of version keys
/// declared in a sibling `LoadFolders.xml`, if one was read; pass `None` to skip
/// that non-blocking cross-check (e.g. on the fast save-token validation path).
pub fn validate_about_metadata_document(
    doc: &XmlDocument,
    load_folders_versions: Option<&[String]>,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();

    let Some(root_id) = find_root_element(doc) else {
        return diagnostics;
    };

    if element_name(doc, root_id) != Some("ModMetaData") {
        diagnostics.push(error_at(
            doc,
            root_id,
            "about_invalid_root",
            "About.xml root element must be <ModMetaData>.",
        ));
        return diagnostics;
    }

    for &name in KNOWN_SCALAR_FIELDS {
        if count_children(doc, root_id, name) > 1 {
            diagnostics.push(
                error_at(
                    doc,
                    root_id,
                    "about_duplicate_field",
                    format!(
                        "Multiple <{name}> elements found; RimWorld will read only one, ambiguously."
                    ),
                )
                .with_field_path(name)
                .with_args(diagnostic_args([("fieldName", name.into())])),
            );
        }
    }

    let Some(view) = build_about_metadata_view(doc) else {
        return diagnostics;
    };
    let fields = &view.fields;

    match &fields.package_id.value {
        None => diagnostics.push(
            warning_at(
                doc,
                root_id,
                "about_missing_package_id",
                "packageId is missing.",
            )
            .with_field_path("packageId"),
        ),
        Some(package_id) => {
            if !is_valid_package_id(package_id) {
                diagnostics.push(
                    error_at(
                        doc,
                        root_id,
                        "about_invalid_package_id",
                        format!("packageId '{package_id}' is not a valid RimWorld package id."),
                    )
                    .with_field_path("packageId")
                    .with_args(diagnostic_args([("packageId", package_id.as_str().into())])),
                );
            }
            let lower = package_id.to_ascii_lowercase();
            if lower.contains("ludeon") && !is_ludeon_package_id(package_id) {
                diagnostics.push(
                    error_at(
                        doc,
                        root_id,
                        "about_reserved_package_id",
                        "packageId cannot contain 'Ludeon' unless it is an official Ludeon Studios package.",
                    )
                    .with_field_path("packageId")
                    .with_args(diagnostic_args([("packageId", package_id.as_str().into())])),
                );
            }
        }
    }

    let is_core_package = fields
        .package_id
        .value
        .as_deref()
        .map(is_ludeon_package_id)
        .unwrap_or(false);

    if fields.supported_versions.present {
        if fields.supported_versions.items.is_empty() {
            diagnostics.push(
                error_at(
                    doc,
                    root_id,
                    "about_empty_supported_versions",
                    "supportedVersions is present but empty.",
                )
                .with_field_path("supportedVersions"),
            );
        }
        for v in &fields.supported_versions.items {
            if parse_version_strict(v).is_none() {
                diagnostics.push(
                    warning_at(
                        doc,
                        root_id,
                        "about_malformed_supported_version",
                        format!(
                            "Supported version '{v}' is not a well-formed Major.Minor version."
                        ),
                    )
                    .with_field_path("supportedVersions")
                    .with_args(diagnostic_args([("version", v.as_str().into())])),
                );
            }
        }
    } else if !is_core_package {
        diagnostics.push(
            warning_at(
                doc,
                root_id,
                "about_missing_supported_versions",
                "supportedVersions is missing.",
            )
            .with_field_path("supportedVersions"),
        );
    }

    if fields.target_version.value.is_some() {
        diagnostics.push(
            warning_at(
                doc,
                root_id,
                "about_obsolete_target_version",
                "targetVersion is obsolete; use supportedVersions instead.",
            )
            .with_field_path("targetVersion"),
        );
    }

    for dep in &fields.mod_dependencies {
        let dep_valid = dep.package_id.as_deref().is_some_and(is_valid_package_id);
        if !dep_valid {
            diagnostics.push(error_at(
                doc,
                dep.node_id,
                "about_invalid_dependency_package_id",
                "Dependency has a missing or invalid packageId.",
            ));
        }
        if dep.display_name.as_deref().unwrap_or("").trim().is_empty() {
            diagnostics.push(error_at(
                doc,
                dep.node_id,
                "about_dependency_missing_display_name",
                "Dependency is missing displayName.",
            ));
        }
        let dep_is_ludeon = dep.package_id.as_deref().is_some_and(is_ludeon_package_id);
        if !dep_is_ludeon && dep.download_url.is_none() && dep.steam_workshop_url.is_none() {
            diagnostics.push(warning_at(
                doc,
                dep.node_id,
                "about_dependency_missing_url",
                "Dependency has no downloadUrl or steamWorkshopUrl.",
            ));
        }
    }

    for unknown in &view.unknown_children {
        diagnostics.push(
            warning_at(
                doc,
                unknown.node_id,
                "about_unknown_top_level_element",
                format!("Unknown top-level element <{}>.", unknown.name),
            )
            .with_field_path(&unknown.name)
            .with_args(diagnostic_args([(
                "elementName",
                unknown.name.as_str().into(),
            )])),
        );
    }

    for entry in &fields.descriptions_by_version {
        check_versioned_key(doc, root_id, &entry.version, &mut diagnostics);
    }
    for entry in &fields.mod_dependencies_by_version {
        check_versioned_key(doc, root_id, &entry.version, &mut diagnostics);
    }
    for entry in &fields.load_before_by_version {
        check_versioned_key(doc, root_id, &entry.version, &mut diagnostics);
    }
    for entry in &fields.load_after_by_version {
        check_versioned_key(doc, root_id, &entry.version, &mut diagnostics);
    }
    for entry in &fields.incompatible_with_by_version {
        check_versioned_key(doc, root_id, &entry.version, &mut diagnostics);
    }

    if let Some(lf_versions) = load_folders_versions {
        // Compare by (major, minor) rather than raw string so a LoadFolders.xml key with a
        // build/revision suffix (e.g. `v1.6.4491`, which `read_load_folders_version_keys`
        // passes through as `1.6.4491`) correctly matches a supportedVersions entry of `1.6`.
        let supported: std::collections::HashSet<(u32, u32)> = fields
            .supported_versions
            .items
            .iter()
            .filter_map(|v| parse_major_minor_loose(v))
            .collect();
        for v in lf_versions {
            let Some(major_minor) = parse_major_minor_loose(strip_leading_v(v)) else {
                continue;
            };
            if !supported.contains(&major_minor) {
                diagnostics.push(warning_at(
                    doc,
                    root_id,
                    "about_load_folders_version_mismatch",
                    format!(
                        "LoadFolders.xml references version '{v}' which is not listed in supportedVersions."
                    ),
                ).with_args(diagnostic_args([("version", v.as_str().into())])));
            }
        }
    }

    diagnostics
}

fn check_versioned_key(
    doc: &XmlDocument,
    root_id: XmlNodeId,
    key: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if !is_valid_versioned_key(key) {
        diagnostics.push(warning_at(
            doc,
            root_id,
            "about_invalid_versioned_key",
            format!("Versioned override key '{key}' is not a well-formed vMajor.Minor or Major.Minor key."),
        ).with_args(diagnostic_args([("key", key.into())])));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml_document::parse_to_document;

    fn diagnose(xml: &str) -> Vec<ValidationDiagnostic> {
        let doc = parse_to_document("About/About.xml", xml);
        validate_about_metadata_document(&doc, None)
    }

    fn has_code(diags: &[ValidationDiagnostic], code: &str) -> bool {
        diags.iter().any(|d| d.code == code)
    }

    #[test]
    fn valid_minimal_about_produces_no_diagnostics() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <name>Foo</name>
  <author>Foo Author</author>
  <supportedVersions>
    <li>1.6</li>
  </supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
    }

    #[test]
    fn invalid_package_id_format_is_blocking() {
        let xml = r#"<ModMetaData>
  <packageId>not_valid</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_invalid_package_id")
            .expect("expected invalid package id diagnostic");
        assert!(diag.blocking);
    }

    #[test]
    fn ludeon_reserved_word_in_non_official_package_id_is_blocking() {
        let xml = r#"<ModMetaData>
  <packageId>someone.ludeon.fake</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_reserved_package_id")
            .expect("expected reserved package id diagnostic");
        assert!(diag.blocking);
    }

    #[test]
    fn official_ludeon_package_id_is_allowed() {
        let xml = r#"<ModMetaData>
  <packageId>ludeon.rimworld</packageId>
  <author>Ludeon Studios</author>
</ModMetaData>"#;
        let diags = diagnose(xml);
        assert!(
            !has_code(&diags, "about_reserved_package_id"),
            "official core package id should not trigger reserved-word check: {:?}",
            diags
        );
        assert!(
            !has_code(&diags, "about_missing_supported_versions"),
            "core package should not warn about missing supportedVersions: {:?}",
            diags
        );
    }

    #[test]
    fn missing_supported_versions_warns_for_non_core_package() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_missing_supported_versions")
            .expect("expected missing supportedVersions diagnostic");
        assert!(!diag.blocking);
    }

    #[test]
    fn empty_supported_versions_list_is_blocking() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions></supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_empty_supported_versions")
            .expect("expected empty supportedVersions diagnostic");
        assert!(diag.blocking);
    }

    #[test]
    fn malformed_supported_version_warns() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6.4491</li></supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_malformed_supported_version")
            .expect("expected malformed version diagnostic");
        assert!(!diag.blocking);
    }

    #[test]
    fn obsolete_target_version_warns() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <targetVersion>1.4.0</targetVersion>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        assert!(has_code(&diags, "about_obsolete_target_version"));
    }

    #[test]
    fn dependency_missing_display_name_and_package_id_is_blocking() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
  <modDependencies>
    <li>
      <downloadUrl>https://example.com</downloadUrl>
    </li>
  </modDependencies>
</ModMetaData>"#;
        let diags = diagnose(xml);
        assert!(has_code(&diags, "about_invalid_dependency_package_id"));
        let display_name_diag = diags
            .iter()
            .find(|d| d.code == "about_dependency_missing_display_name")
            .expect("expected missing displayName diagnostic");
        assert!(display_name_diag.blocking);
    }

    #[test]
    fn dependency_without_url_warns_unless_ludeon() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
  <modDependencies>
    <li>
      <packageId>ludeon.rimworld.royalty</packageId>
      <displayName>Royalty</displayName>
    </li>
    <li>
      <packageId>some.other.mod</packageId>
      <displayName>Other Mod</displayName>
    </li>
  </modDependencies>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let url_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "about_dependency_missing_url")
            .collect();
        assert_eq!(
            url_warnings.len(),
            1,
            "only the non-Ludeon dependency should warn: {:?}",
            diags
        );
    }

    #[test]
    fn duplicate_singleton_field_is_blocking() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <packageId>foo.baz</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
        let diags = diagnose(xml);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_duplicate_field")
            .expect("expected duplicate field diagnostic");
        assert!(diag.blocking);
    }

    #[test]
    fn unknown_top_level_element_warns() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
  <somethingMade Up="true" />
</ModMetaData>"#;
        let diags = diagnose(xml);
        assert!(has_code(&diags, "about_unknown_top_level_element"));
    }

    #[test]
    fn malformed_versioned_override_key_warns() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
  <descriptionsByVersion>
    <notAVersion>Oops</notAVersion>
  </descriptionsByVersion>
</ModMetaData>"#;
        let diags = diagnose(xml);
        assert!(has_code(&diags, "about_invalid_versioned_key"));
    }

    #[test]
    fn wrong_root_element_for_about_path_is_blocking() {
        let doc = parse_to_document(
            "About/About.xml",
            "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>",
        );
        let diags = validate_about_metadata_document(&doc, None);
        let diag = diags
            .iter()
            .find(|d| d.code == "about_invalid_root")
            .expect("expected invalid root diagnostic");
        assert!(diag.blocking);
    }

    #[test]
    fn load_folders_version_not_in_supported_versions_warns() {
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
        let doc = parse_to_document("About/About.xml", xml);
        let load_folders_versions = vec!["v1.5".to_string(), "v1.6".to_string()];
        let diags = validate_about_metadata_document(&doc, Some(&load_folders_versions));
        let diag = diags
            .iter()
            .find(|d| d.code == "about_load_folders_version_mismatch")
            .expect("expected load folders mismatch diagnostic");
        assert!(!diag.blocking);
    }

    #[test]
    fn load_folders_version_with_build_number_matches_plain_supported_version() {
        // `read_load_folders_version_keys` passes build/revision-suffixed keys (e.g.
        // `v1.6.4491`) through as `1.6.4491`, stripped only of the leading `v`. The
        // cross-check must still match these against a plain `1.6` supportedVersions
        // entry by (major, minor), the same way RimWorld's own LoadFolders resolver does.
        let xml = r#"<ModMetaData>
  <packageId>foo.bar</packageId>
  <supportedVersions><li>1.6</li></supportedVersions>
</ModMetaData>"#;
        let doc = parse_to_document("About/About.xml", xml);
        let load_folders_versions = vec!["1.6.4491".to_string()];
        let diags = validate_about_metadata_document(&doc, Some(&load_folders_versions));
        assert!(
            !has_code(&diags, "about_load_folders_version_mismatch"),
            "build-number LoadFolders key should match major.minor supportedVersions entry: {:?}",
            diags
        );
    }
}
