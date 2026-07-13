import { useEffect, useRef, useState } from "react";
import { X, ChevronUp, ChevronDown, RotateCcw, Loader2 } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import { previewDefPatches } from "../../api/patchPreview";
import type {
  ApplyDiagnostic,
  InheritanceDiagnostic,
  OperationTraceStatus,
  PatchOperationKey,
  PatchPreviewConflictDiagnostic,
  PatchPreviewOperationSummary,
  PatchPreviewResult,
  PatchPreviewTarget,
} from "../../types/patchPreview";
import { patchOperationKeyToString, samePatchOperationKey } from "../../types/patchPreview";
import { applyLocalReorder } from "../../lib/previewOperationOrder";
import styles from "./PatchPreviewDialog.module.css";

interface Props {
  projectId: string;
  target: PatchPreviewTarget;
  onClose: () => void;
}

export function PatchPreviewDialog({ projectId, target, onClose }: Props) {
  const [disabledKeys, setDisabledKeys] = useState<PatchOperationKey[]>([]);
  const [orderKeys, setOrderKeys] = useState<PatchOperationKey[] | null>(null);
  const [result, setResult] = useState<PatchPreviewResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestSeq = useRef(0);

  useEffect(() => {
    const seq = ++requestSeq.current;
    let cancelled = false;
    setLoading(true);
    setError(null);
    previewDefPatches(projectId, target, {
      disabled: disabledKeys,
      order: orderKeys ?? [],
    })
      .then((r) => {
        if (cancelled || requestSeq.current !== seq) return;
        setResult(r);
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (cancelled || requestSeq.current !== seq) return;
        setError(formatError(e));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, target, disabledKeys, orderKeys]);

  function toggleDisabled(key: PatchOperationKey) {
    setDisabledKeys((prev) =>
      prev.some((k) => samePatchOperationKey(k, key))
        ? prev.filter((k) => !samePatchOperationKey(k, key))
        : [...prev, key],
    );
  }

  function reorderableDefaultOrder(): PatchOperationKey[] {
    return (result?.visibleOperations ?? [])
      .filter((op) => op.canReorder)
      .map((op) => op.key);
  }

  function moveReorderable(key: PatchOperationKey, direction: -1 | 1) {
    const current = orderKeys ?? reorderableDefaultOrder();
    const index = current.findIndex((k) => samePatchOperationKey(k, key));
    const target = index + direction;
    if (index < 0 || target < 0 || target >= current.length) return;
    const next = current.slice();
    [next[index], next[target]] = [next[target], next[index]];
    setOrderKeys(next);
  }

  function resetOrder() {
    setOrderKeys(null);
  }

  const visibleOperations = result?.visibleOperations ?? [];
  // `visibleOperations` is already in default (backend) order -- unknown-impact operations are
  // never `canReorder`, so keeping them in this same default-order list before splitting doesn't
  // affect the reorder slot-filling math below.
  const normalOperations = visibleOperations.filter((op) => op.target.kind !== "unsupported");
  const unknownImpactOperations = visibleOperations.filter(
    (op) => op.target.kind === "unsupported",
  );
  const orderedNormalOperations = applyLocalReorder(normalOperations, orderKeys);

  return (
    <div className={styles.overlay} role="dialog" aria-modal="true" aria-label="Patch preview">
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>
            Patch preview - {target.defType}:{target.identity}
          </span>
          <button className={styles.closeBtn} onClick={onClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        {loading && (
          <div className={styles.statusBanner}>
            <Loader2 size={13} className="spin" />
            Loading preview…
          </div>
        )}

        {error && <p className={styles.error}>{error}</p>}

        {!loading && !error && result && (
          <>
            <div
              className={`${styles.statusBanner} ${
                result.isPartial ? styles.statusPartial : styles.statusComplete
              }`}
            >
              {!result.defFound
                ? (result.applyDiagnostics.find(
                    (d) =>
                      d.code === "patch_preview_target_not_found" ||
                      d.code === "patch_preview_target_removed",
                  )?.message ?? "Def not found in the combined document.")
                : result.isPartial
                  ? "Partial preview - some operations could not be fully previewed."
                  : "Complete preview."}
            </div>

            <div className={styles.body}>
              <div className={styles.opsColumn}>
                <div className={styles.opsHeader}>
                  <span>Patch operations ({orderedNormalOperations.length})</span>
                  <button
                    className={styles.resetBtn}
                    onClick={resetOrder}
                    disabled={orderKeys == null}
                    type="button"
                  >
                    <RotateCcw size={11} />
                    Reset order
                  </button>
                </div>
                {orderedNormalOperations.length === 0 ? (
                  <p className={styles.emptyOps}>No patches affect this Def.</p>
                ) : (
                  <ul className={styles.opsList} role="list">
                    {orderedNormalOperations.map((op) => (
                      <OperationRow
                        key={patchOperationKeyToString(op.key)}
                        op={op}
                        disabled={disabledKeys.some((k) => samePatchOperationKey(k, op.key))}
                        onToggleDisabled={() => toggleDisabled(op.key)}
                        onMove={(direction) => moveReorderable(op.key, direction)}
                      />
                    ))}
                  </ul>
                )}

                {unknownImpactOperations.length > 0 && (
                  <>
                    <div className={styles.opsHeader}>
                      <span>Unknown impact ({unknownImpactOperations.length})</span>
                    </div>
                    <p className={styles.unknownImpactHint}>
                      These operations' XPath could not be statically resolved. They are included
                      here only because a runtime trace matched them against this Def's
                      pre-patch ancestor chain.
                    </p>
                    <ul className={styles.opsList} role="list">
                      {unknownImpactOperations.map((op) => (
                        <OperationRow
                          key={patchOperationKeyToString(op.key)}
                          op={op}
                          disabled={disabledKeys.some((k) => samePatchOperationKey(k, op.key))}
                          onToggleDisabled={() => toggleDisabled(op.key)}
                          onMove={() => undefined}
                        />
                      ))}
                    </ul>
                  </>
                )}
              </div>

              <div className={styles.xmlColumn}>
                <div className={styles.xmlHeader}>Final XML</div>
                <pre className={styles.xmlView}>{result.xml ?? "(Def not found)"}</pre>
              </div>
            </div>

            <DiagnosticsSection
              applyDiagnostics={result.applyDiagnostics}
              inheritanceDiagnostics={result.inheritanceDiagnostics}
              conflictDiagnostics={result.conflictDiagnostics}
            />
          </>
        )}

        <div className={styles.footer}>
          <button className={styles.closeFooterBtn} onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

function operationReason(op: PatchPreviewOperationSummary): string {
  if (op.target.kind === "def") {
    return `Targets ${op.target.defType}:${op.target.defName} directly`;
  }
  if (op.target.kind === "defType") {
    return `Targets every ${op.target.defType}`;
  }
  if (op.target.kind === "defs") {
    return `Targets ${op.target.defNames.length} Defs directly by defName`;
  }
  return "Unknown static impact - matched via runtime trace";
}

function statusClass(status: OperationTraceStatus | null): string {
  switch (status) {
    case "applied":
      return styles.statusOk;
    case "failed":
      return styles.statusErrorText;
    case "unsupported":
      return styles.statusWarnText;
    case "skipped":
      return styles.statusMuted;
    default:
      return styles.statusMuted;
  }
}

interface OperationRowProps {
  op: PatchPreviewOperationSummary;
  disabled: boolean;
  onToggleDisabled: () => void;
  onMove: (direction: -1 | 1) => void;
}

function OperationRow({ op, disabled, onToggleDisabled, onMove }: OperationRowProps) {
  return (
    <li className={styles.opRow}>
      <input
        type="checkbox"
        checked={!disabled}
        onChange={onToggleDisabled}
        aria-label={`Enable ${op.className}`}
      />
      <div className={styles.opMain}>
        <div className={styles.opTopLine}>
          <span className={styles.opClassName}>{op.className}</span>
          <span className={`${styles.opStatus} ${statusClass(op.status)}`}>
            {op.status ?? "n/a"}
          </span>
        </div>
        <div className={styles.opMeta}>
          {op.locationName} · {op.relativePath}
        </div>
        {op.xpath && <div className={styles.opXpath}>{op.xpath}</div>}
        <div className={styles.opReason}>{operationReason(op)}</div>
        {op.previewSupport.kind === "unsupported" && (
          <div className={styles.opUnsupported}>Preview unsupported: {op.previewSupport.reason}</div>
        )}
        {op.statusMessage && <div className={styles.opUnsupported}>{op.statusMessage}</div>}
      </div>
      {op.canReorder && (
        <div className={styles.opReorderBtns}>
          <button
            className={styles.reorderBtn}
            onClick={() => onMove(-1)}
            aria-label={`Move ${op.className} up`}
            type="button"
          >
            <ChevronUp size={12} />
          </button>
          <button
            className={styles.reorderBtn}
            onClick={() => onMove(1)}
            aria-label={`Move ${op.className} down`}
            type="button"
          >
            <ChevronDown size={12} />
          </button>
        </div>
      )}
    </li>
  );
}

interface DiagnosticsSectionProps {
  applyDiagnostics: ApplyDiagnostic[];
  inheritanceDiagnostics: InheritanceDiagnostic[];
  conflictDiagnostics: PatchPreviewConflictDiagnostic[];
}

function DiagnosticsSection({
  applyDiagnostics,
  inheritanceDiagnostics,
  conflictDiagnostics,
}: DiagnosticsSectionProps) {
  const total =
    applyDiagnostics.length + inheritanceDiagnostics.length + conflictDiagnostics.length;
  if (total === 0) return null;

  return (
    <div className={styles.diagnostics}>
      <div className={styles.diagnosticsHeader}>Diagnostics ({total})</div>
      <ul className={styles.diagnosticsList} role="list">
        {conflictDiagnostics.map((d, i) => (
          <li key={`conflict-${i}`} className={`${styles.diagnosticItem} ${styles.severityWarning}`}>
            <span className={styles.diagnosticBadge}>{d.code}</span>
            {d.message}
          </li>
        ))}
        {applyDiagnostics.map((d, i) => (
          <li
            key={`apply-${i}`}
            className={`${styles.diagnosticItem} ${
              d.severity === "error" ? styles.severityError : styles.severityWarning
            }`}
          >
            <span className={styles.diagnosticBadge}>{d.code}</span>
            {d.message}
          </li>
        ))}
        {inheritanceDiagnostics.map((d, i) => (
          <li
            key={`inheritance-${i}`}
            className={`${styles.diagnosticItem} ${
              d.severity === "error" ? styles.severityError : styles.severityWarning
            }`}
          >
            <span className={styles.diagnosticBadge}>{d.code}</span>
            {d.message}
          </li>
        ))}
      </ul>
    </div>
  );
}
