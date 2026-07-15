import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, extname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  findCodeLikeStringLiterals,
  findOrphanedCatalogCodes,
  findMissingCatalogCodes,
  findProducedDiagnosticCodes,
  isRustTestPath,
} from "./checkDiagnosticRegistry.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const rustSrcRoot = join(repoRoot, "src-tauri", "src");
const frontendSrcRoot = join(repoRoot, "src");
const diagnosticsJsonPath = join(repoRoot, "src", "i18n", "resources", "en", "diagnostics.json");

function listFiles(dir, extensions) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      out.push(...listFiles(full, extensions));
    } else if (extensions.includes(extname(entry))) {
      out.push(full);
    }
  }
  return out;
}

// Diagnostic codes closed by an exhaustive sweep of the codebase: every one of these is genuinely
// emitted toward the frontend (a Tauri command rejection, an AppError-family struct field, or an
// index/schema-load diagnostic reachable from normal UI flows) and previously had no
// `diagnostics:codes.*` entry, so `renderDiagnostic` silently fell back to raw backend English
// `message` for it. Kept as an explicit regression list (in addition to the general "every
// produced code has a catalog entry" test below) so this specific, already-confirmed-reachable set
// can never silently regress even if the general producer-detection heuristic's coverage ever
// changes. Regresses that sweep's fix as one data-driven assertion instead of 49 hand-written
// `it()` blocks.
const ROUND_6_CATALOGED_CODES = [
  // Def-index / patch-index cache and build errors.
  "def_index_cache_write_failed",
  "def_index_load_failed",
  "def_index_rebuild_failed",
  "def_index_location_scan_failed",
  "def_index_file_read_failed",
  "def_index_parse_error",
  "def_index_overlay_location_missing",
  "patch_index_cache_write_failed",
  "patch_index_load_failed",
  "patch_index_rebuild_failed",
  "patch_index_location_scan_failed",
  "patch_index_file_read_failed",
  // Commands/services.
  "def_not_indexed",
  "file_read_error",
  "file_read_failed",
  "xml_edit_failed",
  "indexing_already_started",
  "form_view_path_failed",
  "form_view_not_found",
  "form_view_read_failed",
  "form_view_write_failed",
  "form_view_invalid_name",
  // Save.
  "save_file_outside_root",
  "save_unsupported_file",
  "save_invalid_xml",
  "save_backup_failed",
  "save_temp_write_failed",
  "save_replace_failed",
  // Schema pack / sidecars.
  "schema_pack_game_version_unresolvable",
  "schema_pack_game_version_mismatch",
  "schema_pack_duplicate_pack_id",
  "schema_pack_patch_operation_file_too_large",
  "schema_pack_patch_operation_file_read_failed",
  "schema_pack_field_order_unknown",
  "schema_pack_object_field_order_unknown",
  "schema_pack_patch_operation_field_order_unknown",
  "schema_pack_validation_rule_unknown_field",
  "schema_pack_validation_rule_unknown_condition_field",
  "schema_pack_unknown_discriminator_variant_target",
  "schema_pack_unknown_object_schema_ref",
  "schema_pack_unknown_list_item_schema_ref",
  "schema_pack_form_view_amendment_without_base",
  "schema_pack_form_view_unknown_field_reference",
  "schema_pack_locale_json_invalid",
  "schema_pack_locale_non_string_value",
  "schema_pack_locale_unknown_key",
  "schema_pack_locale_unresolved_key",
  "schema_pack_locale_wrong_owner",
  "patch_operation_metadata_unknown_preview_kind",
];

describe("check-diagnostic-registry (real tree)", () => {
  it("every diagnostics:codes.* entry has a matching literal in src-tauri/src or src", () => {
    const diagnosticsJson = JSON.parse(readFileSync(diagnosticsJsonPath, "utf8"));
    const catalogCodes = Object.keys(diagnosticsJson.codes ?? {});

    const rustFiles = listFiles(rustSrcRoot, [".rs"]);
    const frontendFiles = listFiles(frontendSrcRoot, [".ts", ".tsx"]);
    const codeLikeStringSets = [...rustFiles, ...frontendFiles].map((file) =>
      findCodeLikeStringLiterals(readFileSync(file, "utf8")),
    );

    expect(findOrphanedCatalogCodes(catalogCodes, codeLikeStringSets)).toEqual([]);
  });

  it("every round-6 previously-missing diagnostic code now has a catalog entry", () => {
    const diagnosticsJson = JSON.parse(readFileSync(diagnosticsJsonPath, "utf8"));
    const catalogCodes = new Set(Object.keys(diagnosticsJson.codes ?? {}));

    const stillMissing = ROUND_6_CATALOGED_CODES.filter((code) => !catalogCodes.has(code));
    expect(stillMissing).toEqual([]);
  });

  // The reverse direction of the check above -- a diagnostic code actually constructed in
  // src-tauri/src (via a known family constructor, `.with_code(`, a `code: "..."` struct field, or
  // a `let code = match` block; see findProducedDiagnosticCodes) with NO entry in
  // diagnostics.json's `codes` object at all. This is what closes the recurring bug class the
  // list above was a one-time cleanup of: any *future* missing-catalog-entry code now fails this
  // test (and `pnpm i18n:check`) instead of relying on someone to notice it manually.
  it("every diagnostic code constructed in src-tauri/src (outside test code) has a catalog entry", () => {
    const diagnosticsJson = JSON.parse(readFileSync(diagnosticsJsonPath, "utf8"));
    const catalogCodes = Object.keys(diagnosticsJson.codes ?? {});

    const rustFiles = listFiles(rustSrcRoot, [".rs"]).filter((file) => !isRustTestPath(file));
    const producedCodes = new Set();
    for (const file of rustFiles) {
      for (const code of findProducedDiagnosticCodes(readFileSync(file, "utf8"), file)) {
        producedCodes.add(code);
      }
    }

    expect(findMissingCatalogCodes(producedCodes, catalogCodes)).toEqual([]);
  });
});
