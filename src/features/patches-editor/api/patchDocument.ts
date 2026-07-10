import { invoke } from "@tauri-apps/api/core";
import type { PatchFile } from "../types/patchFile";

/** Parse a `<Patch>` file's raw XML text into its editable operation AST. Stateless -- the
 * patches editor reuses `useXmlEditorSession`'s raw XML buffer, undo/redo, and save/save-preview
 * flow (like every other XML file), calling this only to derive a structured tree from the
 * current buffer. */
export function parsePatchOperations(relativePath: string, rawXml: string): Promise<PatchFile> {
  return invoke("parse_patch_operations", { relativePath, rawXml });
}

/** Serialize an edited patch operation AST back to RimWorld-compatible `<Patch>` XML text. The
 * caller is responsible for feeding the result into the session's raw XML buffer
 * (`updateRawXml`), which flows through the normal save/save-preview commands unchanged. */
export function serializePatchOperations(patchFile: PatchFile): Promise<string> {
  return invoke("serialize_patch_operations", { patchFile });
}
