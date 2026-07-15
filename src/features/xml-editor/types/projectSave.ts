export type DiffLineKind = "unchanged" | "added" | "removed" | "gap";

export interface DiffLine {
  kind: DiffLineKind;
  oldLine: number | null;
  newLine: number | null;
  text: string;
  /** Set only for `kind === "gap"`: the number of elided unchanged lines this marker stands
   * in for. Machine-readable so the frontend can pluralize/format it via `t()` instead of
   * rendering a pre-formatted English sentence from the backend. */
  count?: number;
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
