/** TypeScript mirror of `src-tauri/src/patches/model.rs`'s wire shape, as produced/consumed by
 * the `parse_patch_operations`/`serialize_patch_operations` Tauri commands. `PatchOperationKind`
 * is adjacently tagged (`{ type, data }`) because `Sequence` wraps a bare `Vec`, which serde
 * cannot represent under an internally-tagged (`{ type, ...fields }`) enum. */

export type PatchOperationId = number;

export interface PatchSpan {
  start: number;
  end: number;
  line: number;
  column: number;
}

export interface XmlAttributeModel {
  name: string;
  value: string;
}

export type PatchSuccessMode = "normal" | "invert" | "always" | "never";

export type PatchOrderMode = "append" | "prepend";

export interface PatchDiagnostic {
  line: number | null;
  column: number | null;
  message: string;
}

export interface PathedOperationData {
  xpath: string | null;
}

export interface PathedValueOperationData {
  xpath: string | null;
  valueXml: string | null;
}

export interface PathedValueOrderOperationData {
  xpath: string | null;
  valueXml: string | null;
  order: PatchOrderMode | null;
}

export interface AttributeValueOperationData {
  xpath: string | null;
  attribute: string | null;
  value: string | null;
}

export interface AttributeOperationData {
  xpath: string | null;
  attribute: string | null;
}

export interface SetNameOperationData {
  xpath: string | null;
  name: string | null;
}

export interface FindModData {
  mods: string[];
  matchOp: PatchOperationNode | null;
  nomatchOp: PatchOperationNode | null;
}

export interface ConditionalData {
  xpath: string | null;
  matchOp: PatchOperationNode | null;
  nomatchOp: PatchOperationNode | null;
}

export interface UnknownOperationData {
  rawXml: string;
}

export type PatchOperationKind =
  | { type: "add"; data: PathedValueOrderOperationData }
  | { type: "insert"; data: PathedValueOrderOperationData }
  | { type: "remove"; data: PathedOperationData }
  | { type: "replace"; data: PathedValueOperationData }
  | { type: "attributeAdd"; data: AttributeValueOperationData }
  | { type: "attributeSet"; data: AttributeValueOperationData }
  | { type: "attributeRemove"; data: AttributeOperationData }
  | { type: "addModExtension"; data: PathedValueOperationData }
  | { type: "setName"; data: SetNameOperationData }
  | { type: "sequence"; data: PatchOperationNode[] }
  | { type: "findMod"; data: FindModData }
  | { type: "conditional"; data: ConditionalData }
  | { type: "test"; data: PathedOperationData }
  | { type: "unknown"; data: UnknownOperationData };

export interface PatchOperationNode {
  id: PatchOperationId;
  className: string;
  success: PatchSuccessMode;
  attributes: XmlAttributeModel[];
  kind: PatchOperationKind;
  span: PatchSpan | null;
}

export interface PatchFile {
  relativePath: string;
  xmlDeclaration: string | null;
  operations: PatchOperationNode[];
  diagnostics: PatchDiagnostic[];
  hadFatalParseError: boolean;
}

/** Built-in operation `Class` names RimEdit understands structurally (mirrors
 * `patches::model::BUILT_IN_OPERATION_CLASSES`). Any other class name parses as `kind: "unknown"`. */
export const BUILT_IN_OPERATION_CLASSES = [
  "PatchOperationAdd",
  "PatchOperationInsert",
  "PatchOperationRemove",
  "PatchOperationReplace",
  "PatchOperationAttributeAdd",
  "PatchOperationAttributeSet",
  "PatchOperationAttributeRemove",
  "PatchOperationAddModExtension",
  "PatchOperationSetName",
  "PatchOperationSequence",
  "PatchOperationFindMod",
  "PatchOperationConditional",
  "PatchOperationTest",
] as const;

export type BuiltInOperationClass = (typeof BUILT_IN_OPERATION_CLASSES)[number];

export function isBuiltInOperationClass(className: string): className is BuiltInOperationClass {
  return (BUILT_IN_OPERATION_CLASSES as readonly string[]).includes(className);
}
