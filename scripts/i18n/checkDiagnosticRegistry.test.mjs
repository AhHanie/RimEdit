import { describe, it, expect } from "vitest";
import {
  findCodeLikeStringLiterals,
  findOrphanedCatalogCodes,
  findMissingCatalogCodes,
  findProducedDiagnosticCodes,
  isRustTestPath,
  looksLikeDiagnosticCode,
  stripCfgTestModules,
} from "./checkDiagnosticRegistry.mjs";

describe("looksLikeDiagnosticCode", () => {
  it("accepts flat snake_case with at least one underscore", () => {
    expect(looksLikeDiagnosticCode("validation_missing_required_field")).toBe(true);
  });

  it("accepts SCREAMING_SNAKE_CASE", () => {
    expect(looksLikeDiagnosticCode("TOKEN_NOT_FOUND")).toBe(true);
  });

  it("rejects a single word with no underscore", () => {
    expect(looksLikeDiagnosticCode("packageId")).toBe(false);
    expect(looksLikeDiagnosticCode("hello")).toBe(false);
  });

  it("rejects mixed-case snake_case", () => {
    expect(looksLikeDiagnosticCode("Validation_missing")).toBe(false);
  });

  it("rejects an empty string", () => {
    expect(looksLikeDiagnosticCode("")).toBe(false);
  });
});

describe("findCodeLikeStringLiterals", () => {
  it("finds a simple double-quoted code literal", () => {
    const found = findCodeLikeStringLiterals('let code = "validation_missing_required_field";');
    expect(found.has("validation_missing_required_field")).toBe(true);
  });

  it("ignores non-code-shaped string literals", () => {
    const found = findCodeLikeStringLiterals('let label = "Hello";');
    expect(found.size).toBe(0);
  });

  it("survives a backslash-newline line continuation inside an earlier string literal", () => {
    // Mirrors src-tauri/src/form_views/error.rs's `#[error("... \` continuation pattern -- a
    // naive `\\.`-based tokenizer desyncs here because `.` never matches a line terminator.
    const source = [
      '#[error(',
      '    "The Form View store was saved by a newer version of RimEdit (schema version {0}); \\',
      '     opening read-only with no custom views until the app is upgraded."',
      ")]",
      "UnsupportedNewerVersion(u32),",
      "",
      'let code = "form_view_unsupported_version";',
    ].join("\n");
    const found = findCodeLikeStringLiterals(source);
    expect(found.has("form_view_unsupported_version")).toBe(true);
  });
});

describe("findOrphanedCatalogCodes", () => {
  it("returns catalog codes with no matching literal in any scanned set", () => {
    const orphaned = findOrphanedCatalogCodes(
      ["known_code", "stale_code"],
      [new Set(["known_code"]), new Set(["other_code"])],
    );
    expect(orphaned).toEqual(["stale_code"]);
  });

  it("returns an empty array when every catalog code is found", () => {
    const orphaned = findOrphanedCatalogCodes(["a_code", "b_code"], [new Set(["a_code"]), new Set(["b_code"])]);
    expect(orphaned).toEqual([]);
  });
});

describe("isRustTestPath", () => {
  it("flags a file under a tests/ directory", () => {
    expect(isRustTestPath("src-tauri/src/xml_document/tests/validation_core.rs")).toBe(true);
    expect(isRustTestPath("src-tauri\\src\\xml_document\\tests\\validation_core.rs")).toBe(true);
  });

  it("flags a _test.rs file", () => {
    expect(isRustTestPath("src-tauri/src/foo_test.rs")).toBe(true);
  });

  it("does not flag ordinary production source", () => {
    expect(isRustTestPath("src-tauri/src/xml_document/validation/about.rs")).toBe(false);
  });
});

describe("stripCfgTestModules", () => {
  it("removes a #[cfg(test)] mod body but keeps surrounding code", () => {
    const source = [
      "fn real() -> &'static str { \"real_code\" }",
      "",
      "#[cfg(test)]",
      "mod tests {",
      "    fn fake() { let code = \"fake_code\"; }",
      "}",
      "",
      "fn after() -> &'static str { \"after_code\" }",
    ].join("\n");
    const stripped = stripCfgTestModules(source);
    expect(stripped).toContain("real_code");
    expect(stripped).toContain("after_code");
    expect(stripped).not.toContain("fake_code");
  });

  it("handles nested braces inside the test module correctly", () => {
    const source = [
      "#[cfg(test)]",
      "mod tests {",
      "    fn nested() { if true { let code = \"fake_code\"; } }",
      "}",
      "fn real() -> &'static str { \"real_code\" }",
    ].join("\n");
    const stripped = stripCfgTestModules(source);
    expect(stripped).not.toContain("fake_code");
    expect(stripped).toContain("real_code");
  });

  it("returns the source unchanged when there is no #[cfg(test)] marker", () => {
    const source = "fn real() -> &'static str { \"real_code\" }";
    expect(stripCfgTestModules(source)).toBe(source);
  });
});

describe("findProducedDiagnosticCodes", () => {
  it("finds a code from a struct-literal field", () => {
    const source = 'AppError { code: "io_error".to_string(), message: msg, details: None, args };';
    expect(findProducedDiagnosticCodes(source).has("io_error")).toBe(true);
  });

  it("finds a code from DiagnosticRef::code(...)", () => {
    const source = 'crate::diagnostics::DiagnosticRef::code("project_not_found").with_arg("projectId", id)';
    expect(findProducedDiagnosticCodes(source).has("project_not_found")).toBe(true);
  });

  it("finds a code from .with_code(...)", () => {
    const source = 'diag.with_code("patch_missing_required_field")';
    expect(findProducedDiagnosticCodes(source).has("patch_missing_required_field")).toBe(true);
  });

  it("finds a code from a family constructor with code first (SchemaLoadDiagnostic::error)", () => {
    const source = 'SchemaLoadDiagnostic::error(\n    "schema_pack_locale_json_invalid",\n    format!("JSON parse error: {}", e),\n)';
    expect(findProducedDiagnosticCodes(source).has("schema_pack_locale_json_invalid")).toBe(true);
  });

  it("finds a code from a family constructor with a path first (ValidationDiagnostic::error)", () => {
    const source =
      'ValidationDiagnostic::error(doc.relative_path.clone(), Some(node_id), line, column, "validation_missing_required_field", message)';
    expect(findProducedDiagnosticCodes(source).has("validation_missing_required_field")).toBe(true);
  });

  it("finds a code from an error_at_node helper call", () => {
    const source = [
      "diag::error_at_node(",
      "    doc,",
      "    field_node_id,",
      "    &summary.def_type,",
      "    summary.def_name.as_deref(),",
      '    "validation_field_type_mismatch",',
      "    format!(\"...\"),",
      ")",
    ].join("\n");
    expect(findProducedDiagnosticCodes(source).has("validation_field_type_mismatch")).toBe(true);
  });

  it("finds every code from a `let code = match` block", () => {
    const source = [
      "let code = match &e {",
      '    ProjectFileError::ProjectNotFound(_) => "project_not_found",',
      '    ProjectFileError::ScanFailed(_) => "project_file_scan_failed",',
      "};",
    ].join("\n");
    const found = findProducedDiagnosticCodes(source);
    expect(found.has("project_not_found")).toBe(true);
    expect(found.has("project_file_scan_failed")).toBe(true);
  });

  it("ignores codes constructed only inside a #[cfg(test)] module", () => {
    const source = [
      "#[cfg(test)]",
      "mod tests {",
      '    let diag = ValidationDiagnostic::error("x.xml", None, None, None, "some_code", "message");',
      "}",
    ].join("\n");
    expect(findProducedDiagnosticCodes(source).has("some_code")).toBe(false);
  });

  it("ignores an unrelated code:-shaped field on a non-diagnostic struct passed a filePath excluded by convention", () => {
    const source = 'LoadFolderDiagnostic { code: "load_folders_read_failed".to_string(), message: m }';
    const found = findProducedDiagnosticCodes(source, "src-tauri/src/rimworld_load_folders.rs");
    expect(found.size).toBe(0);
  });

  it("does not match an unrelated ::new( call on a non-diagnostic type", () => {
    const source = 'let v = Vec::new(); let s = "not_a_diagnostic_code_either";';
    // `Vec` is not in the closed DIAGNOSTIC_FAMILY_TYPES allowlist, so this must not match.
    expect(findProducedDiagnosticCodes(source).has("not_a_diagnostic_code_either")).toBe(false);
  });

  // --- GraphicPreviewWarning::new(...) (and, by the same Warning/Diagnostic/Error suffix
  // generalization, any future type following the same naming convention) must be recognized as a
  // code-attaching constructor even though it is outside the closed DIAGNOSTIC_FAMILY_TYPES list
  // and uses `::new(`, not `::error(`/`::warning(`. ---

  it("finds a code from GraphicPreviewWarning::new(...), which is outside the closed family list", () => {
    const source = [
      "GraphicPreviewWarning::new(",
      '    "graphic_preview_dds_unsupported",',
      '    format!("Resolved DDS texture {}", candidate),',
      ")",
    ].join("\n");
    expect(findProducedDiagnosticCodes(source).has("graphic_preview_dds_unsupported")).toBe(true);
  });

  it("finds a code from any future Warning/Diagnostic/Error-suffixed type's ::new(...) without a scanner update", () => {
    const source = 'FormViewStoreWarning::new("form_view_hypothetical_future_code", message)';
    expect(
      findProducedDiagnosticCodes(source).has("form_view_hypothetical_future_code"),
    ).toBe(true);
  });

  it("does not match ::new( on a type that merely contains, but doesn't end in, Warning/Diagnostic/Error", () => {
    // PathBuf::new(...) is a real call shape used throughout this codebase (see Path::new(...) in
    // findProducedDiagnosticCodes's own doc comment). Neither "Path" nor "PathBuf" end in
    // Warning/Diagnostic/Error, so the suffix-based pattern must not match this call at all, and a
    // '/'-containing path literal wouldn't look code-shaped even if it somehow did.
    const source = 'let p = PathBuf::new("some/relative/path.xml");';
    expect(findProducedDiagnosticCodes(source).size).toBe(0);
  });

  it("matches ::new( on an Error-suffixed type outside the closed list but adds nothing when no argument is code-shaped", () => {
    // A hypothetical ParseIntError-like type: the suffix-based constructor pattern matches the
    // call shape (it ends in "Error"), but with no code-shaped literal anywhere in its arguments,
    // nothing is added to the produced set -- the same graceful fallback every other constructor
    // pattern already relies on (see the family-constructor tests above).
    const source = 'SomeWeirdError::new(invalid_digit_position, "not a code, has spaces")';
    expect(findProducedDiagnosticCodes(source).size).toBe(0);
  });
});

describe("findMissingCatalogCodes", () => {
  it("returns produced codes with no catalog entry, sorted", () => {
    const missing = findMissingCatalogCodes(new Set(["b_code", "a_code", "known_code"]), ["known_code"]);
    expect(missing).toEqual(["a_code", "b_code"]);
  });

  it("returns an empty array when every produced code is cataloged", () => {
    const missing = findMissingCatalogCodes(new Set(["a_code"]), ["a_code", "b_code"]);
    expect(missing).toEqual([]);
  });
});
