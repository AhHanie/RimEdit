import { invoke } from "@tauri-apps/api/core";
import { measureAsync } from "../../../instrumentation";
import type { TemplateFieldValue } from "../../schema-catalog/types";
import type { CreateDefResult } from "../types/createDef";
import type { GraphicPreviewAssetResult } from "../types/graphicPreview";
import type {
  XmlDocumentLoadResult,
  XmlEdit,
  XmlEditContext,
  XmlEditorDocumentLoadResult,
} from "../types/xmlDocument";

export function readProjectXmlDocument(
  projectId: string,
  relativePath: string,
): Promise<XmlDocumentLoadResult> {
  return invoke("read_project_xml_document", { projectId, relativePath });
}

export function readProjectXmlEditorDocument(
  projectId: string,
  relativePath: string,
): Promise<XmlEditorDocumentLoadResult> {
  return measureAsync(
    "xmlEditor.loadProjectDocument",
    () => invoke("read_project_xml_editor_document", { projectId, relativePath }),
    { sourceKind: "project", relativePath },
  );
}

export function readLocationXmlEditorDocument(
  projectId: string,
  locationId: string,
  relativePath: string,
): Promise<XmlEditorDocumentLoadResult> {
  return measureAsync(
    "xmlEditor.loadLocationDocument",
    () => invoke("read_location_xml_editor_document", { projectId, locationId, relativePath }),
    { sourceKind: "source", relativePath },
  );
}

export function parseXmlEditorBuffer(
  projectId: string,
  relativePath: string,
  rawXml: string,
): Promise<XmlEditorDocumentLoadResult> {
  return measureAsync(
    "xmlEditor.parseBuffer",
    () => invoke("parse_xml_editor_buffer", { projectId, relativePath, rawXml }),
    { relativePath },
  );
}

export function applyXmlEditorEdit(
  projectId: string,
  relativePath: string,
  rawXml: string,
  edit: XmlEdit,
  editContext?: XmlEditContext,
): Promise<XmlEditorDocumentLoadResult> {
  return measureAsync(
    "xmlEditor.applyEdit",
    () => invoke("apply_xml_editor_edit", { projectId, relativePath, rawXml, edit, editContext }),
    { relativePath },
  );
}

export function applyXmlEditorEdits(
  projectId: string,
  relativePath: string,
  rawXml: string,
  edits: XmlEdit[],
  editContext?: XmlEditContext,
): Promise<XmlEditorDocumentLoadResult> {
  return measureAsync(
    "xmlEditor.applyEdits",
    () =>
      invoke("apply_xml_editor_edits", { projectId, relativePath, rawXml, edits, editContext }),
    { relativePath, batchSize: edits.length },
  );
}

export function resolveGraphicPreviewAssets(
  projectId: string,
  texPath: string,
  graphicClass: string,
  maskPath?: string,
): Promise<GraphicPreviewAssetResult> {
  return measureAsync("graphicPreview.resolveAssets", () =>
    invoke("resolve_graphic_preview_assets", {
      projectId,
      texPath,
      graphicClass,
      maskPath,
    }),
  );
}

export function createDefFromTemplate(
  projectId: string,
  relativePath: string,
  rawXml: string,
  defType: string,
  templateId: string | null,
  fieldValues: Record<string, TemplateFieldValue>,
): Promise<CreateDefResult> {
  return invoke("create_def_from_template", {
    projectId,
    relativePath,
    rawXml,
    defType,
    templateId,
    fieldValues,
  });
}
