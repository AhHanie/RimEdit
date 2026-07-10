import type { SchemaCatalog } from "../../schema-catalog";
import type { PatchOperationNode } from "../types/patchFile";

/** Best-effort xpath (or other identifying detail) for a row's collapsed summary line. */
function summaryDetail(node: PatchOperationNode): string | undefined {
  const kind = node.kind;
  switch (kind.type) {
    case "add":
    case "insert":
    case "remove":
    case "replace":
    case "attributeAdd":
    case "attributeSet":
    case "attributeRemove":
    case "addModExtension":
    case "setName":
    case "test":
      return kind.data.xpath ?? undefined;
    case "conditional":
      return kind.data.xpath ?? undefined;
    case "findMod":
      return kind.data.mods.length > 0 ? kind.data.mods.join(", ") : undefined;
    case "sequence":
      return `${kind.data.length} operation${kind.data.length === 1 ? "" : "s"}`;
    case "unknown":
      return undefined;
  }
}

export function operationTitle(node: PatchOperationNode, catalog: SchemaCatalog | null): string {
  return catalog?.patchOperations?.[node.className]?.label || node.className;
}

export function operationSubtitle(node: PatchOperationNode): string | undefined {
  return summaryDetail(node);
}
