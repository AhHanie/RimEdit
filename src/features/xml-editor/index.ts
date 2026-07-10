export { XmlEditorPane } from "./components/XmlEditorPane/XmlEditorPane";
export type { XmlEditorMode, XmlEditorSnapshot } from "./types/editorSession";
export type { XmlEditorDocumentView, ParseDiagnostic } from "./types/xmlDocument";

// Reusable schema-to-form descriptor helpers, exposed for `patches-editor`'s `PatchValueEditor`
// (issue 06), which builds structured subforms for patch operation `<value>` payloads using the
// same shape dispatch and ObjectFieldValue tree the Def form editor uses, rather than a second,
// parallel implementation.
export {
  buildObjectFieldValue,
  fieldSchemaToControl,
  getAllObjectFields,
  resolveObjectSchema,
} from "./lib/objectDescriptors";
export { objectFieldValueToInitialElement } from "./lib/formValues";
export type { FormControlKind, ObjectFieldValue, ObjectListItemValue } from "./types/editorForm";
export type {
  XmlAttributeView,
  XmlChildShape,
  XmlChildView,
  XmlInitialElement,
  XmlListItemView,
  XmlNestedChildView,
} from "./types/xmlDocument";
