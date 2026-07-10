import type { XmlEditorDocumentLoadResult } from "./xmlDocument";

export type { TemplateFieldValue } from "../../schema-catalog/types";

export interface CreateDefResult {
  editorDocument: XmlEditorDocumentLoadResult;
  insertedNodeId: number | null;
  insertedDefType: string;
  insertedDefName: string | null;
}
