import { invoke } from "@tauri-apps/api/core";
import type { XPathCompletionResult } from "../types/xpathCompletion";

/** Schema- and Def-index-aware XPath completions/target inference for `PatchPathInput`. */
export function completePatchOperationXPath(
  projectId: string,
  xpath: string,
): Promise<XPathCompletionResult> {
  return invoke("complete_patch_operation_xpath", { projectId, xpath });
}
