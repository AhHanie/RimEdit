import type { TFunction } from "i18next";
import type { SchemaCatalog } from "../../schema-catalog";
import type { PatchOperationNode } from "../types/patchFile";

/** Best-effort xpath (or other identifying detail) for a row's collapsed summary line. */
function summaryDetail(node: PatchOperationNode, t: TFunction<"patches">): string | undefined {
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
      return t("operationSummary.operationCount", { count: kind.data.length });
    case "unknown":
      return undefined;
  }
}

export function operationTitle(node: PatchOperationNode, catalog: SchemaCatalog | null): string {
  return catalog?.patchOperations?.[node.className]?.label || node.className;
}

export function operationSubtitle(node: PatchOperationNode, t: TFunction<"patches">): string | undefined {
  return summaryDetail(node, t);
}
