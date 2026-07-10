export type DiffLineKind = "unchanged" | "added" | "removed" | "gap";

export interface DiffLine {
  kind: DiffLineKind;
  oldLine: number | null;
  newLine: number | null;
  text: string;
}

export interface SavePreview {
  projectId: string;
  relativePath: string;
  currentHash: string;
  proposedHash: string;
  changed: boolean;
  diff: DiffLine[];
  validationToken: string;
}

export interface SaveResult {
  projectId: string;
  relativePath: string;
  backupPath: string;
  bytesWritten: number;
  currentHash: string;
}
