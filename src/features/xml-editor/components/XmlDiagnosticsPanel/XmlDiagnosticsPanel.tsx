import { useState } from "react";
import { AlertTriangle, ChevronDown, ChevronRight } from "lucide-react";
import type { ParseDiagnostic, ValidationDiagnostic } from "../../types/xmlDocument";
import styles from "./XmlDiagnosticsPanel.module.css";

interface Props {
  diagnostics: Array<ParseDiagnostic | ValidationDiagnostic>;
}

export function XmlDiagnosticsPanel({ diagnostics }: Props) {
  const [expanded, setExpanded] = useState(false);

  if (diagnostics.length === 0) return null;

  const normalized = diagnostics.map(normalizeDiagnostic);
  const errorCount = normalized.filter((d) => d.severity === "Error").length;
  const warningCount = normalized.filter((d) => d.severity === "Warning").length;

  return (
    <div className={styles.root}>
      <button
        className={styles.header}
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        <AlertTriangle size={12} className={styles.icon} />
        <span className={styles.count}>
          {diagnostics.length} {diagnostics.length === 1 ? "issue" : "issues"}
          {errorCount > 0 && `, ${errorCount} ${errorCount === 1 ? "error" : "errors"}`}
          {warningCount > 0 &&
            `, ${warningCount} ${warningCount === 1 ? "warning" : "warnings"}`}
        </span>
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
      </button>
      {expanded && (
        <ul className={styles.list} role="list">
          {normalized.map((d, i) => (
            <li key={i} className={`${styles.item} ${styles[`severity${d.severity}`]}`}>
              <span className={styles.badge}>{d.source}</span>
              <span className={styles.badge}>{d.severity}</span>
              {d.line != null && (
                <span className={styles.location}>
                  line {d.line}
                  {d.column != null && `:${d.column}`}
                </span>
              )}
              <span className={styles.message}>
                {d.defName && <span className={styles.context}>{d.defName}: </span>}
                {d.fieldPath && <span className={styles.context}>{d.fieldPath}: </span>}
                {d.message}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

interface UiDiagnostic {
  severity: "Error" | "Warning" | "Info";
  source: "Parse" | "Validation";
  line: number | null;
  column: number | null;
  message: string;
  defName: string | null;
  fieldPath: string | null;
}

function normalizeDiagnostic(diagnostic: ParseDiagnostic | ValidationDiagnostic): UiDiagnostic {
  if ("severity" in diagnostic) {
    return {
      severity: diagnostic.severity,
      source: "Validation",
      line: diagnostic.line,
      column: diagnostic.column,
      message: diagnostic.message,
      defName: diagnostic.defName,
      fieldPath: diagnostic.fieldPath,
    };
  }

  return {
    severity: "Error",
    source: "Parse",
    line: diagnostic.line,
    column: diagnostic.column,
    message: diagnostic.message,
    defName: null,
    fieldPath: null,
  };
}
