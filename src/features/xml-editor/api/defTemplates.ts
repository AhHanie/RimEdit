import { invoke } from "@tauri-apps/api/core";
import type { CreateDefResult } from "../types/createDef";
import type {
  DeleteUserDefTemplateResult,
  UserDefTemplate,
  UserDefTemplateSummary,
} from "../types/defTemplates";

export function listUserDefTemplates(
  projectId: string,
  defType: string,
): Promise<UserDefTemplateSummary[]> {
  return invoke("list_user_def_templates", { projectId, defType });
}

export function saveUserDefTemplate(
  projectId: string,
  relativePath: string,
  rawXml: string,
  nodeId: number,
  name: string,
): Promise<UserDefTemplate> {
  return invoke("save_user_def_template", {
    projectId,
    relativePath,
    rawXml,
    nodeId,
    name,
  });
}

export function createDefFromUserTemplate(
  projectId: string,
  relativePath: string,
  rawXml: string,
  templateId: string,
  defName: string,
): Promise<CreateDefResult> {
  return invoke("create_def_from_user_template", {
    projectId,
    relativePath,
    rawXml,
    templateId,
    defName,
  });
}

export function deleteUserDefTemplate(
  projectId: string,
  templateId: string,
): Promise<DeleteUserDefTemplateResult> {
  return invoke("delete_user_def_template", { projectId, templateId });
}

export function createDefFromIndexedDef(
  projectId: string,
  relativePath: string,
  rawXml: string,
  sourceLocationId: string,
  sourceRelativePath: string,
  sourceDefType: string,
  sourceDefName: string,
  sourceNodeId: number | null,
  defName: string,
): Promise<CreateDefResult> {
  return invoke("create_def_from_indexed_def", {
    projectId,
    relativePath,
    rawXml,
    sourceLocationId,
    sourceRelativePath,
    sourceDefType,
    sourceDefName,
    sourceNodeId,
    defName,
  });
}
