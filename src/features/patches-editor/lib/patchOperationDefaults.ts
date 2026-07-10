import type {
  PatchFile,
  PatchOperationId,
  PatchOperationKind,
  PatchOperationNode,
  PatchSuccessMode,
} from "../types/patchFile";

/** One past the highest operation id anywhere in `operations` (including nested `sequence`,
 * `match`, and `nomatch` operations), or `0` for an empty tree. New operations must be assigned
 * ids from this counter so they never collide with ids the backend parser assigned. */
export function nextOperationId(operations: PatchOperationNode[]): PatchOperationId {
  let max = -1;
  const visit = (node: PatchOperationNode) => {
    if (node.id > max) max = node.id;
    const { kind } = node;
    if (kind.type === "sequence") {
      kind.data.forEach(visit);
    } else if (kind.type === "findMod" || kind.type === "conditional") {
      if (kind.data.matchOp) visit(kind.data.matchOp);
      if (kind.data.nomatchOp) visit(kind.data.nomatchOp);
    }
  };
  operations.forEach(visit);
  return max + 1;
}

export function nextOperationIdForFile(file: PatchFile): PatchOperationId {
  return nextOperationId(file.operations);
}

/** Deep-clones `node` and every nested operation it contains, assigning each a fresh id via
 * `generateId` so a duplicated (or reparsed-from-raw-XML) subtree never shares ids with the
 * original tree. */
export function cloneWithFreshIds(
  node: PatchOperationNode,
  generateId: () => PatchOperationId,
): PatchOperationNode {
  const id = generateId();
  const kind = node.kind;
  let newKind: PatchOperationKind;
  if (kind.type === "sequence") {
    newKind = { type: "sequence", data: kind.data.map((child) => cloneWithFreshIds(child, generateId)) };
  } else if (kind.type === "findMod") {
    newKind = {
      type: "findMod",
      data: {
        mods: [...kind.data.mods],
        matchOp: kind.data.matchOp ? cloneWithFreshIds(kind.data.matchOp, generateId) : null,
        nomatchOp: kind.data.nomatchOp ? cloneWithFreshIds(kind.data.nomatchOp, generateId) : null,
      },
    };
  } else if (kind.type === "conditional") {
    newKind = {
      type: "conditional",
      data: {
        xpath: kind.data.xpath,
        matchOp: kind.data.matchOp ? cloneWithFreshIds(kind.data.matchOp, generateId) : null,
        nomatchOp: kind.data.nomatchOp ? cloneWithFreshIds(kind.data.nomatchOp, generateId) : null,
      },
    };
  } else {
    newKind = { ...kind, data: { ...kind.data } } as PatchOperationKind;
  }
  return {
    ...node,
    id,
    attributes: node.attributes.map((a) => ({ ...a })),
    kind: newKind,
    span: null,
  };
}

function defaultKindForClass(className: BuiltInDefaultClass): PatchOperationKind {
  switch (className) {
    case "PatchOperationAdd":
      return { type: "add", data: { xpath: null, valueXml: null, order: null } };
    case "PatchOperationInsert":
      return { type: "insert", data: { xpath: null, valueXml: null, order: null } };
    case "PatchOperationRemove":
      return { type: "remove", data: { xpath: null } };
    case "PatchOperationReplace":
      return { type: "replace", data: { xpath: null, valueXml: null } };
    case "PatchOperationAttributeAdd":
      return { type: "attributeAdd", data: { xpath: null, attribute: null, value: null } };
    case "PatchOperationAttributeSet":
      return { type: "attributeSet", data: { xpath: null, attribute: null, value: null } };
    case "PatchOperationAttributeRemove":
      return { type: "attributeRemove", data: { xpath: null, attribute: null } };
    case "PatchOperationAddModExtension":
      return { type: "addModExtension", data: { xpath: null, valueXml: null } };
    case "PatchOperationSetName":
      return { type: "setName", data: { xpath: null, name: null } };
    case "PatchOperationSequence":
      return { type: "sequence", data: [] };
    case "PatchOperationFindMod":
      return { type: "findMod", data: { mods: [], matchOp: null, nomatchOp: null } };
    case "PatchOperationConditional":
      return { type: "conditional", data: { xpath: null, matchOp: null, nomatchOp: null } };
    case "PatchOperationTest":
      return { type: "test", data: { xpath: null } };
  }
}

export const BUILT_IN_DEFAULT_CLASSES = [
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

export type BuiltInDefaultClass = (typeof BUILT_IN_DEFAULT_CLASSES)[number];

export function isBuiltInDefaultClass(className: string): className is BuiltInDefaultClass {
  return (BUILT_IN_DEFAULT_CLASSES as readonly string[]).includes(className);
}

/** Builds a blank structured operation node for a built-in class, ready to insert into the tree
 * and edit via `PatchOperationForm`. */
export function createBuiltInOperation(
  className: BuiltInDefaultClass,
  id: PatchOperationId,
  success: PatchSuccessMode = "normal",
): PatchOperationNode {
  return {
    id,
    className,
    success,
    attributes: [],
    kind: defaultKindForClass(className),
    span: null,
  };
}
