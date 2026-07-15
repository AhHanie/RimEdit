import { useEffect } from "react";
import { useTranslation } from "react-i18next";
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
    code: d.code ?? "patch_diagnostic",
    args: d.args,
  }));
}

/** Patch editor "form" mode body: an operation tree editor for `<Patch>` files, standing in for
 * `XmlFormEditor` when the open file's root element is `<Patch>` (see `XmlEditorPane`'s routing).
 * Reuses the session's raw XML buffer/undo/save-preview flow unchanged -- every tree edit
 * reserializes to XML text and calls `onChangeRawXml`, exactly like the raw XML editor's
 * `onChange`.
 *
 * Form Views (`features/form-views`, Plan.md section 11) intentionally do not apply here and
 * have no wiring into this component or `patchValueTarget.ts`. A complete Def's `formViews`
 * hide/show *canonical top-level `DefTypeSchema` fields*, but this tree edits an operation AST
 * (`xpath`, raw `valueXml`, nested operations) whose "value" fields are XPath-derived fragments
 * that only *sometimes* correspond 1:1 with a target Def's direct schema fields -- see
 * `patchValueTarget.ts`'s `listDirectDefTypeFields`, which already special-cases "direct schema
 * fields only" and `modExtensions` for exactly this reason. Reusing a Def's `formViews` to filter
 * operation fields or `<value>` payloads would silently hide/misrepresent XPath targets that
 * don't correspond to a single top-level field. A patch-specific Form View design (its own
 * schema-pack metadata, e.g. a hypothetical `patchOperationFormViews`) is deferred; do not wire
 * `useFormViews`/`FormViewSelector` into this pane until that design exists. */
export function PatchEditorPane({ relativePath, rawXml, readOnly, catalog, projectId, onChangeRawXml, registerFlush }: Props) {
  const { t } = useTranslation("patches");
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
          <span>{t("editorPane.parsing")}</span>
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
