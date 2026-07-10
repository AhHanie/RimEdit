import { invoke } from "@tauri-apps/api/core";
import type { PatchPreviewRequest, PatchPreviewResult } from "../types/patchPreview";

/** Previews one Def's post-patch, post-inheritance XML with preview-only enable/disable/reorder
 * overrides applied. Never modifies any patch file -- see `services::patch_preview` on the
 * backend. */
export function previewDefPatches(
  projectId: string,
  defType: string,
  defName: string,
  request: PatchPreviewRequest,
): Promise<PatchPreviewResult> {
  return invoke("preview_def_patches", { projectId, defType, defName, request });
}
