/** TypeScript mirror of `src-tauri/src/patches/xpath.rs`'s wire shape, as produced by the
 * `complete_patch_operation_xpath` Tauri command. See that module's doc comment for the
 * conservative-subset philosophy (what's completed vs. merely left editable). */

import type { DiagnosticArgs } from "../../../lib/diagnostics";
import type { FieldSchema } from "../../schema-catalog";

/** Mirrors `patches::impact_graph::XPathTarget`. `"unsupported"` covers both "not rooted at Defs"
 * and "valid but outside the conservative completion subset"; `"noXPath"` does not occur here (it
 * only applies to non-pathed operation kinds at the AST/index layer). */
export type XPathTarget =
  | { kind: "def"; defType: string; defName: string }
  | { kind: "defType"; defType: string }
  | { kind: "defs"; defType: string; defNames: string[] }
  | { kind: "unsupported" }
  | { kind: "noXPath" };

export type XPathCompletionItemKind =
  | "root"
  | "defType"
  | "predicateKey"
  | "defName"
  | "field"
  | "fieldAlias";

/** One completion suggestion. Apply it by splicing `insertText` in place of everything from
 * `replaceFrom` (a byte offset into the xpath string) to the end -- no client-side XPath parsing
 * needed. */
export interface XPathCompletionItem {
  insertText: string;
  label: string;
  detail: string | null;
  kind: XPathCompletionItemKind;
}

export type XPathDiagnosticSeverity = "error" | "warning";

export interface XPathDiagnostic {
  severity: XPathDiagnosticSeverity;
  code: string;
  message: string;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

/** The field a fully- (or mostly-) typed XPath resolves to, for a structured value subform. Only
 * ever set for a field declared directly on the resolved Def type. */
export interface XPathResolvedField {
  defType: string;
  fieldName: string;
  field: FieldSchema;
}

export interface XPathCompletionResult {
  replaceFrom: number;
  items: XPathCompletionItem[];
  diagnostics: XPathDiagnostic[];
  target: XPathTarget;
  resolvedField: XPathResolvedField | null;
}
