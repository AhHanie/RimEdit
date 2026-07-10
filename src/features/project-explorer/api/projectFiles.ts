import { invoke } from "@tauri-apps/api/core";
import { measureAsync } from "../../../instrumentation";
import type {
  ProjectFileScan,
  ProjectFileContent,
  ProjectFileEntry,
  ProjectFolderEntry,
  ProjectPathMutationResult,
} from "../types";

export function scanProjectFiles(projectId: string): Promise<ProjectFileScan> {
  return measureAsync("projectExplorer.scanFiles", () =>
    invoke("scan_project_files", { projectId }),
  );
}

export function readProjectXmlFile(
  projectId: string,
  relativePath: string,
): Promise<ProjectFileContent> {
  return invoke("read_project_xml_file", { projectId, relativePath });
}

export function createProjectFile(
  projectId: string,
  parentPath: string,
  fileName: string,
  contents?: string,
): Promise<ProjectFileEntry> {
  return invoke("create_project_file_cmd", { projectId, parentPath, fileName, contents });
}

export function createProjectFolder(
  projectId: string,
  parentPath: string,
  folderName: string,
): Promise<ProjectFolderEntry> {
  return invoke("create_project_folder_cmd", { projectId, parentPath, folderName });
}

export function renameProjectPath(
  projectId: string,
  relativePath: string,
  newName: string,
  kind: "file" | "folder",
): Promise<ProjectPathMutationResult> {
  return invoke("rename_project_path_cmd", { projectId, relativePath, newName, kind });
}

export function deleteProjectPath(
  projectId: string,
  relativePath: string,
  kind: "file" | "folder",
): Promise<ProjectPathMutationResult> {
  return invoke("delete_project_path_cmd", { projectId, relativePath, kind });
}
