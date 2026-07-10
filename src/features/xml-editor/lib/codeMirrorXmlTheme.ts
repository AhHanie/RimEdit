import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

export const rimEditCodeMirrorTheme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--text-primary)",
    backgroundColor: "var(--surface-editor)",
    fontSize: "12.5px",
  },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: "1.65",
  },
  ".cm-content": {
    padding: "16px",
    caretColor: "var(--text-primary)",
  },
  ".cm-gutters": {
    backgroundColor: "var(--surface-editor)",
    color: "var(--text-muted)",
    borderRight: "1px solid var(--border-subtle)",
  },
  ".cm-activeLine": {
    backgroundColor: "var(--surface-hover)",
  },
  ".cm-selectionBackground": {
    backgroundColor: "var(--accent-muted) !important",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--text-primary)",
  },
});

export const rimEditXmlHighlighting = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.tagName, color: "var(--syntax-xml-tag)" },
    { tag: tags.attributeName, color: "var(--syntax-xml-attribute)" },
    { tag: tags.attributeValue, color: "var(--syntax-xml-attribute-value)" },
    { tag: tags.comment, color: "var(--syntax-xml-comment)" },
    { tag: tags.processingInstruction, color: "var(--syntax-xml-processing)" },
    { tag: tags.angleBracket, color: "var(--syntax-xml-punctuation)" },
    { tag: tags.character, color: "var(--syntax-xml-entity)" },
  ]),
);
