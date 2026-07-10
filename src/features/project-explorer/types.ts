export type ProjectFileKind = "xml" | "text" | "binary" | "unknown";

export interface ProjectFileEntry {
  relativePath: string;
  folderPath: string;
  fileName: string;
  extension: string;
  sizeBytes: number;
  fileKind: ProjectFileKind;
  /** Whether this XML file is included in the active game-version load set. Absent for non-XML. */
  activeForGameVersion?: boolean;
  locationId?: string;
  locationName?: string;
  sourceKind?: "project" | "source";
  readOnly?: boolean;
}

export interface ProjectFolderEntry {
  relativePath: string;
  folderName: string;
  parentPath: string;
}

export interface ProjectFileScan {
  projectId: string;
  projectRoot: string;
  folders: ProjectFolderEntry[];
  files: ProjectFileEntry[];
}

export interface ProjectFileContent {
  projectId: string;
  relativePath: string;
  contents: string;
}

export interface ProjectPathMutationResult {
  oldPath: string;
  newPath: string;
}

export interface FileTreeFolderNode {
  type: "folder";
  id: string;
  name: string;
  relativePath: string;
  children: FileTreeNode[];
}

export interface FileTreeFileNode {
  type: "file";
  id: string;
  name: string;
  relativePath: string;
  folderPath: string;
  extension: string;
  sizeBytes: number;
  fileKind: ProjectFileKind;
  activeForGameVersion?: boolean;
}

export type FileTreeNode = FileTreeFolderNode | FileTreeFileNode;
