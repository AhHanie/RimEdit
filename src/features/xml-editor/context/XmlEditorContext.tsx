import { createContext, useContext } from "react";
import type { SchemaCatalog } from "../../schema-catalog";
import type { XmlEditorFileRef } from "../hooks/useXmlEditorSession";

interface XmlEditorContextValue {
  projectId?: string;
  readOnly: boolean;
  catalog?: SchemaCatalog | null;
  onNavigateDef?: (fileRef: XmlEditorFileRef, nodeId: number | null) => void;
}

const XmlEditorContext = createContext<XmlEditorContextValue | null>(null);

export function XmlEditorContextProvider({
  value,
  children,
}: {
  value: XmlEditorContextValue;
  children: React.ReactNode;
}) {
  return (
    <XmlEditorContext.Provider value={value}>
      {children}
    </XmlEditorContext.Provider>
  );
}

export function useXmlEditorContext(): XmlEditorContextValue {
  const value = useContext(XmlEditorContext);
  if (!value) {
    throw new Error("useXmlEditorContext must be used inside XmlEditorContextProvider.");
  }
  return value;
}
