import { invoke } from "@tauri-apps/api/core";
import type {
  PatchPreviewRequest,
  PatchPreviewResult,
  PatchPreviewTarget,
} from "../types/patchPreview";

/** Previews one Def's post-patch, post-inheritance XML with preview-only enable/disable/reorder
 * overrides applied. Never modifies any patch file -- see `services::patch_preview` on the
 * backend. `projectId` is only the active editable project used as preview context; `target`
 * identifies the exact Def the caller opened (its file origin and in-file ordinal), which may
 * belong to a read-only source rather than `projectId`. */
export function previewDefPatches(
  projectId: string,
  target: PatchPreviewTarget,
  request: PatchPreviewRequest,
): Promise<PatchPreviewResult> {
  return invoke("preview_def_patches", { projectId, target, request });
}
