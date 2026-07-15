// Shared pure logic for scripts/i18n/check-hardcoded-jsx-strings.mjs. Kept filesystem-free so it
// is easy to unit test with in-memory source fixtures (matches the scripts/i18n/shared.mjs
// pattern: a thin CLI wrapper around a pure, directly-testable core).
//
// Walks the TypeScript/JSX AST (rather than a regex) so multi-line JSX, nested expressions, and
// attribute strings are all found precisely instead of pattern-matched. This is a source-level
// "does this look like untranslated human text" heuristic, not a semantic check: it flags any JSX
// text node or curated user-facing attribute string literal that contains a letter, on the theory
// that translatable UI copy always contains letters and everything else (icons-only punctuation,
// numbers, technical identifiers passed as literal `className`/`data-*` values) is exempt by
// attribute name already.

import ts from "typescript";

/** JSX attribute names whose string-literal values are user-facing text (and therefore should
 * flow through `t(...)`), not technical/DOM plumbing. Deliberately excludes `className`, `id`,
 * `key`, `data-*`, `htmlFor`, `type`, `name`, `href`, `src`, `role`, `style`, and similar
 * non-translatable attributes. */
const USER_FACING_ATTRIBUTES = new Set(["title", "aria-label", "aria-description", "placeholder", "alt"]);

/** True when `text` contains at least one Unicode letter -- the signal used throughout this
 * checker to distinguish "looks like translatable copy" from punctuation/whitespace/numeric
 * literals (dividers like "|"/"•", spacers, formatting glue) that never need a translation key. */
export function containsLetter(text) {
  return /\p{L}/u.test(text);
}

/**
 * Finds bare (non-`t(...)`-derived) user-facing string literals in one TSX source file.
 *
 * Returns an array of `{ line, column, kind, text }` violations, 1-indexed `line`/`column` to
 * match editor/IDE conventions.
 */
export function findHardcodedJsxStrings(fileName, sourceText) {
  const sourceFile = ts.createSourceFile(fileName, sourceText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const violations = [];

  function report(node, kind, text) {
    const { line, character } = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    violations.push({ line: line + 1, column: character + 1, kind, text: text.trim() });
  }

  function visit(node) {
    if (ts.isJsxText(node)) {
      const text = node.getText(sourceFile);
      if (containsLetter(text)) {
        report(node, "jsx-text", text);
      }
    } else if (ts.isJsxAttribute(node)) {
      const attrName = node.name.getText(sourceFile);
      if (
        USER_FACING_ATTRIBUTES.has(attrName) &&
        node.initializer &&
        ts.isStringLiteral(node.initializer) &&
        containsLetter(node.initializer.text)
      ) {
        report(node.initializer, `jsx-attribute:${attrName}`, node.initializer.text);
      }
    }
    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
  return violations;
}
