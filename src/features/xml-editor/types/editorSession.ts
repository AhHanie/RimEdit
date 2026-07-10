import type { SavePreview } from "./projectSave";
import type { ParseDiagnostic, ValidationDiagnostic } from "./xmlDocument";
import type { XmlEditorDocumentView } from "./xmlDocument";

export type XmlEditorMode = "form" | "raw";

export interface XmlEditorSnapshot {
  rawXml: string;
  parsed: XmlEditorDocumentView | null;
  parseDiagnostics: ParseDiagnostic[];
  validationDiagnostics: ValidationDiagnostic[];
  selectedDefNodeId: number | null;
}

export interface XmlEditorSession {
  projectId: string;
  relativePath: string;
  baseRawXml: string;
  currentRawXml: string;
  lastValidSnapshot: XmlEditorSnapshot | null;
  mode: XmlEditorMode;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  savePreview: SavePreview | null;
  saveError: string | null;
  loading: boolean;
  loadError: string | null;
}
