export type OpenFileSourceKind = "project" | "source";

export type OpenFileEditorKind = "xml" | "unsupported";

export interface OpenFileRef {
  locationId: string;
  locationName?: string;
  sourceKind: OpenFileSourceKind;
  readOnly: boolean;
  relativePath: string;
  /**
   * Overrides the scan-derived editor kind used when first opening this file as a
   * tab. Needed right after creating a file: the project scan is refreshed
   * asynchronously, so it may not yet contain the new entry when `openTab` runs.
   */
  editorKindHint?: OpenFileEditorKind;
}

export function makeOpenFileTabKey(
  file: Pick<OpenFileRef, "locationId" | "relativePath">,
): string {
  return `${file.locationId}:${file.relativePath}`;
}

export interface OpenFileTab extends OpenFileRef {
  tabKey: string;
  fileName: string;
  folderPath: string;
  dirty: boolean;
  editorKind: OpenFileEditorKind;
  selectedDefNodeId?: number;
  selectionRequestId?: number;
}

export interface ActiveEditorCommands {
  undo: () => void;
  redo: () => void;
  save: () => Promise<void>;
  close: () => Promise<void>;
  canUndo: boolean;
  canRedo: boolean;
  canSave: boolean;
  canClose: boolean;
}
