import { isBuiltInOperationClass } from "../types/patchFile";
import type { SchemaCatalog } from "../../schema-catalog/types";
import type { PatchDiagnostic, PatchOperationNode } from "../types/patchFile";

/** Client-side companion to the backend's `PatchOperationClassification`
 * (`patches::index::PatchOperationClassification`): flags `unknown`-kind operations (a `Class`
 * RimEdit's parser did not recognize as one of the 13 built-in classes) with a diagnostic
 * distinguishing "known custom, but preview cannot execute it" from "genuinely unrecognized" --
 * the same two cases `patches::index`'s `Custom`/`Unknown` classification already distinguishes
 * for the (whole-project) patch index and Def preview dialog, computed here at the single-file
 * level so a typo'd or unsupported `Class=` is visible while directly editing a patch file, not
 * only when later opening a Def's patch preview.
 *
 * Purely additive to the parser's own `patchFile.diagnostics` (invalid XML, wrong root, missing
 * fields, etc.) -- callers should render both lists together. */
export function classificationDiagnostics(
  operations: PatchOperationNode[],
  catalog: SchemaCatalog | null,
): PatchDiagnostic[] {
  const diagnostics: PatchDiagnostic[] = [];
  const visit = (node: PatchOperationNode) => {
    // A built-in class name that parsed as `unknown` didn't fail classification -- it fell back
    // to raw-XML preservation because of an unrecognized extra field (`patches::parser`'s
    // `unrecognized_field_name` path), which already pushes its own explanatory
    // `patchFile.diagnostics` entry. Built-in classes are also shipped as schema-pack metadata
    // (issue 03, so the form renderer is data-driven), so without this guard `metadata` below
    // would resolve and this would wrongly claim a real built-in is "a custom operation ...
    // preview cannot execute it".
    if (node.kind.type === "unknown" && node.className !== "" && !isBuiltInOperationClass(node.className)) {
      const metadata = catalog?.patchOperations?.[node.className];
      diagnostics.push(
        metadata
          ? {
              line: null,
              column: null,
              message: `'${node.className}' is a custom operation defined by schema-pack metadata; preview cannot execute it: ${
                metadata.preview.message || "unsupported"
              }`,
            }
          : {
              line: null,
              column: null,
              message: `'${node.className}' is not a recognized built-in patch operation class and has no schema-pack metadata describing it; it will be preserved as raw XML but cannot be validated or previewed`,
            },
      );
    }
    if (node.kind.type === "sequence") {
      node.kind.data.forEach(visit);
    } else if (node.kind.type === "findMod" || node.kind.type === "conditional") {
      if (node.kind.data.matchOp) visit(node.kind.data.matchOp);
      if (node.kind.data.nomatchOp) visit(node.kind.data.nomatchOp);
    }
  };
  operations.forEach(visit);
  return diagnostics;
}
