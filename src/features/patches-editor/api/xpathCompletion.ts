import { invoke } from "@tauri-apps/api/core";
import type { XPathCompletionResult } from "../types/xpathCompletion";

/** Schema- and Def-index-aware XPath completions/target inference for `PatchPathInput`. `locale`
 * is the frontend's active UI locale (`useLocale()`'s current value), passed explicitly -- like
 * `loadSchemaCatalog` -- rather than left for the backend to read from persisted settings, so a
 * runtime locale switch that has not finished persisting yet can never race a completion request
 * into serving a stale locale's labels (issue 06's explicit-locale-argument contract). */
export function completePatchOperationXPath(
  projectId: string,
  xpath: string,
  locale?: string,
): Promise<XPathCompletionResult> {
  return invoke("complete_patch_operation_xpath", { projectId, xpath, locale });
}
