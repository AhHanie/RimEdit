import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, ChevronDown, ChevronRight } from "lucide-react";
import {
  renderDiagnostic,
  renderDiagnosticCountSummary,
  renderDiagnosticLocation,
  renderDiagnosticSeverity,
  renderDiagnosticSource,
} from "../../../../i18n/diagnostics";
import type { DiagnosticArgs } from "../../../../lib/diagnostics";
import type { ParseDiagnostic, ValidationDiagnostic } from "../../types/xmlDocument";
import styles from "./XmlDiagnosticsPanel.module.css";

interface Props {
  diagnostics: Array<ParseDiagnostic | ValidationDiagnostic>;
}

export function XmlDiagnosticsPanel({ diagnostics }: Props) {
  const [expanded, setExpanded] = useState(false);
  const { t, i18n } = useTranslation(["diagnostics", "common"]);

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
        aria-label={t("diagnostics:panel.toggleAriaLabel")}
      >
        <AlertTriangle size={12} className={styles.icon} />
        <span className={styles.count}>
          {renderDiagnosticCountSummary({ total: diagnostics.length, errorCount, warningCount }, i18n)}
        </span>
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
      </button>
      {expanded && (
        <ul className={styles.list} role="list">
          {normalized.map((d, i) => {
            const location = renderDiagnosticLocation({ line: d.line, column: d.column }, i18n);
            return (
              <li key={i} className={`${styles.item} ${styles[`severity${d.severity}`]}`}>
                <span className={styles.badge}>{renderDiagnosticSource(d.source, i18n)}</span>
                <span className={styles.badge}>{renderDiagnosticSeverity(d.severity, i18n)}</span>
                {location && <span className={styles.location}>{location}</span>}
                <span className={styles.message}>
                  {d.defName && <span className={styles.context}>{d.defName}: </span>}
                  {d.fieldPath && <span className={styles.context}>{d.fieldPath}: </span>}
                  {renderDiagnostic(d, i18n)}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

interface UiDiagnostic {
  severity: "Error" | "Warning" | "Info";
  source: "parse" | "validation";
  line: number | null;
  column: number | null;
  code?: string;
  args?: DiagnosticArgs;
  message: string;
  defName: string | null;
  fieldPath: string | null;
}

function normalizeDiagnostic(diagnostic: ParseDiagnostic | ValidationDiagnostic): UiDiagnostic {
  if ("severity" in diagnostic) {
    return {
      severity: diagnostic.severity,
      source: "validation",
      line: diagnostic.line,
      column: diagnostic.column,
      code: diagnostic.code,
      args: diagnostic.args,
      message: diagnostic.message,
      defName: diagnostic.defName,
      fieldPath: diagnostic.fieldPath,
    };
  }

  return {
    severity: "Error",
    source: "parse",
    line: diagnostic.line,
    column: diagnostic.column,
    code: diagnostic.code,
    args: diagnostic.args,
    message: diagnostic.message,
    defName: null,
    fieldPath: null,
  };
}
