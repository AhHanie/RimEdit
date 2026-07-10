import { useEffect } from "react";
import { Loader2 } from "lucide-react";
import type { SchemaCatalog } from "../../../schema-catalog";
import type { ParseDiagnostic } from "../../../xml-editor/types/xmlDocument";
import { XmlDiagnosticsPanel } from "../../../xml-editor/components/XmlDiagnosticsPanel/XmlDiagnosticsPanel";
import { usePatchOperationTree } from "../../hooks/usePatchOperationTree";
import { classificationDiagnostics } from "../../lib/operationClassificationDiagnostics";
import { PatchOperationTree } from "../PatchOperationTree/PatchOperationTree";
import type { PatchDiagnostic } from "../../types/patchFile";
import styles from "./PatchEditorPane.module.css";

interface Props {
  relativePath: string;
  rawXml: string;
  readOnly: boolean;
  catalog: SchemaCatalog | null;
  /** Absent when the open file has no project context (see `PatchPathInput`'s `projectId` prop). */
  projectId: string | null;
  onChangeRawXml: (xml: string) => void;
  /** Called once with a flush function the parent must await before saving, switching mode, or
   * navigating away -- mirrors `useXmlFormController`'s `flushAll`, guarding against a debounced
   * field commit (here: an in-flight serialize-and-propagate) losing the latest edit. */
  registerFlush?: (flush: () => Promise<void>) => void;
}

function toParseDiagnostics(diagnostics: PatchDiagnostic[], relativePath: string): ParseDiagnostic[] {
  return diagnostics.map((d) => ({
    relativePath,
    line: d.line,
    column: d.column,
    byteOffset: null,
    message: d.message,
  }));
}

/** Patch editor "form" mode body: an operation tree editor for `<Patch>` files, standing in for
 * `XmlFormEditor` when the open file's root element is `<Patch>` (see `XmlEditorPane`'s routing).
 * Reuses the session's raw XML buffer/undo/save-preview flow unchanged -- every tree edit
 * reserializes to XML text and calls `onChangeRawXml`, exactly like the raw XML editor's
 * `onChange`. */
export function PatchEditorPane({ relativePath, rawXml, readOnly, catalog, projectId, onChangeRawXml, registerFlush }: Props) {
  const { patchFile, loading, error, setOperations, generateId, flush } = usePatchOperationTree({
    relativePath,
    rawXml,
    readOnly,
    onChangeRawXml,
  });

  useEffect(() => {
    registerFlush?.(flush);
  }, [registerFlush, flush]);

  if (loading && !patchFile) {
    return (
      <div className={styles.root}>
        <div className="state-loading">
          <Loader2 size={14} className="spin" />
          <span>Parsing patch file…</span>
        </div>
      </div>
    );
  }

  if (error && !patchFile) {
    return (
      <div className={styles.root}>
        <p className="state-empty-text" style={{ color: "var(--text-error)" }}>
          {error}
        </p>
      </div>
    );
  }

  if (!patchFile) return null;

  return (
    <div className={styles.root}>
      <PatchOperationTree
        operations={patchFile.operations}
        catalog={catalog}
        readOnly={readOnly}
        projectId={projectId}
        generateId={generateId}
        setOperations={setOperations}
      />
      <XmlDiagnosticsPanel
        diagnostics={toParseDiagnostics(
          [...patchFile.diagnostics, ...classificationDiagnostics(patchFile.operations, catalog)],
          relativePath,
        )}
      />
    </div>
  );
}
