import { invoke } from "@tauri-apps/api/core";
import { measureAsync } from "../../../instrumentation";
import type { SavePreview, SaveResult } from "../types/projectSave";

export function previewProjectXmlSave(
  projectId: string,
  relativePath: string,
  proposedXml: string,
  traceId?: string,
  fullDiff?: boolean,
): Promise<SavePreview> {
  return measureAsync(
    "xmlEditor.savePreview",
    () =>
      invoke("preview_project_xml_save", {
        projectId,
        relativePath,
        proposedXml,
        traceId: traceId ?? null,
        fullDiff: fullDiff ?? false,
      }),
    { relativePath, xmlBytes: proposedXml.length, ...(traceId != null ? { traceId } : {}) },
  );
}

export function saveProjectXmlFile(
  projectId: string,
  relativePath: string,
  proposedXml: string,
  validationToken?: string,
  traceId?: string,
): Promise<SaveResult> {
  return measureAsync(
    "xmlEditor.saveFile",
    () =>
      invoke("save_project_xml_file", {
        projectId,
        relativePath,
        proposedXml,
        validationToken: validationToken ?? null,
        traceId: traceId ?? null,
      }),
    {
      relativePath,
      xmlBytes: proposedXml.length,
      tokenProvided: validationToken != null,
      ...(traceId != null ? { traceId } : {}),
    },
  );
}
