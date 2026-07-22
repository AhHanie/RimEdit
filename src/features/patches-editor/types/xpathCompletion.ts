/** TypeScript mirror of `src-tauri/src/patches/xpath.rs`'s wire shape, as produced by the
 * `complete_patch_operation_xpath` Tauri command. See that module's doc comment for the
 * conservative-subset philosophy (what's completed vs. merely left editable) and for how field
 * completion descends to unlimited depth through nested object/list/map schema shapes. */

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

/** `listItem`/`mapEntry` are structural XML container names (the literal `li` that opens a
 * `listOfLi`/`keyedObjectMap` entry, or the literal `key`/`value` inside a `keyedObjectMap`
 * entry) -- kept distinct from `field`/`fieldAlias` since they don't name a schema field. */
export type XPathCompletionItemKind =
  | "root"
  | "defType"
  | "predicateKey"
  | "defName"
  | "field"
  | "fieldAlias"
  | "listItem"
  | "mapEntry";

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

/** The field a fully- (or mostly-) typed XPath resolves to, for a structured value subform. This
 * is the *terminal* field on the path -- for a nested path (e.g. `graphicData/texPath`) it's the
 * deepest resolved field, not its root container. `defType` always stays the root Def type for
 * wire compatibility, regardless of how deep `fieldName`/`field` themselves are nested. */
export interface XPathResolvedField {
  defType: string;
  fieldName: string;
  field: FieldSchema;
}

export interface XPathCompletionResult {
  replaceFrom: number;
  /** The bounded, display-ready suggestion list -- see `totalMatches`/`isTruncated`. */
  items: XPathCompletionItem[];
  /** How many suggestions matched before truncation; always `>= items.length`. */
  totalMatches: number;
  /** Whether `items` was truncated to a server-side cap. */
  isTruncated: boolean;
  diagnostics: XPathDiagnostic[];
  target: XPathTarget;
  resolvedField: XPathResolvedField | null;
}
