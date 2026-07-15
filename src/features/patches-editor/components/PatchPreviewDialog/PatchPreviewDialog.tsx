import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { i18n as I18nInstance, TFunction } from "i18next";
import { X, ChevronUp, ChevronDown, RotateCcw, Loader2 } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import { renderDiagnostic, renderDiagnosticSectionHeading } from "../../../../i18n/diagnostics";
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
  // Three separate single-namespace hooks, not `useTranslation(["diagnostics", "patches", "common"])`
  // with `"patches:key"`/`"common:key"`-prefixed lookups -- see `AboutDependencySection`'s
  // `DependencyRow` doc comment (same TypeScript cross-namespace union-size issue, here for
  // `patches.json`).
  const { i18n } = useTranslation("diagnostics");
  const { t } = useTranslation("patches");
  const { t: tCommon } = useTranslation("common");
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
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("previewDialog.dialogAriaLabel")}
    >
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>
            {t("previewDialog.title", { defType: target.defType, identity: target.identity })}
          </span>
          <button className={styles.closeBtn} onClick={onClose} aria-label={tCommon("actions.close")}>
            <X size={14} />
          </button>
        </div>

        {loading && (
          <div className={styles.statusBanner}>
            <Loader2 size={13} className="spin" />
            {t("previewDialog.loadingPreview")}
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
                ? (() => {
                    const notFoundDiagnostic = result.applyDiagnostics.find(
                      (d) =>
                        d.code === "patch_preview_target_not_found" ||
                        d.code === "patch_preview_target_removed",
                    );
                    return notFoundDiagnostic
                      ? renderDiagnostic(notFoundDiagnostic, i18n)
                      : t("previewDialog.defNotFound");
                  })()
                : result.isPartial
                  ? t("previewDialog.partialPreview")
                  : t("previewDialog.completePreview")}
            </div>

            <div className={styles.body}>
              <div className={styles.opsColumn}>
                <div className={styles.opsHeader}>
                  <span>
                    {t("previewDialog.patchOperations", { count: orderedNormalOperations.length })}
                  </span>
                  <button
                    className={styles.resetBtn}
                    onClick={resetOrder}
                    disabled={orderKeys == null}
                    type="button"
                  >
                    <RotateCcw size={11} />
                    {t("previewDialog.resetOrder")}
                  </button>
                </div>
                {orderedNormalOperations.length === 0 ? (
                  <p className={styles.emptyOps}>{t("previewDialog.noPatchesAffect")}</p>
                ) : (
                  <ul className={styles.opsList} role="list">
                    {orderedNormalOperations.map((op) => (
                      <OperationRow
                        key={patchOperationKeyToString(op.key)}
                        op={op}
                        disabled={disabledKeys.some((k) => samePatchOperationKey(k, op.key))}
                        onToggleDisabled={() => toggleDisabled(op.key)}
                        onMove={(direction) => moveReorderable(op.key, direction)}
                        t={t}
                        i18n={i18n}
                      />
                    ))}
                  </ul>
                )}

                {unknownImpactOperations.length > 0 && (
                  <>
                    <div className={styles.opsHeader}>
                      <span>
                        {t("previewDialog.unknownImpact", { count: unknownImpactOperations.length })}
                      </span>
                    </div>
                    <p className={styles.unknownImpactHint}>
                      {t("previewDialog.unknownImpactHint")}
                    </p>
                    <ul className={styles.opsList} role="list">
                      {unknownImpactOperations.map((op) => (
                        <OperationRow
                          key={patchOperationKeyToString(op.key)}
                          op={op}
                          disabled={disabledKeys.some((k) => samePatchOperationKey(k, op.key))}
                          onToggleDisabled={() => toggleDisabled(op.key)}
                          onMove={() => undefined}
                          t={t}
                          i18n={i18n}
                        />
                      ))}
                    </ul>
                  </>
                )}
              </div>

              <div className={styles.xmlColumn}>
                <div className={styles.xmlHeader}>{t("previewDialog.finalXml")}</div>
                <pre className={styles.xmlView}>{result.xml ?? t("previewDialog.defNotFoundXml")}</pre>
              </div>
            </div>

            <DiagnosticsSection
              applyDiagnostics={result.applyDiagnostics}
              inheritanceDiagnostics={result.inheritanceDiagnostics}
              conflictDiagnostics={result.conflictDiagnostics}
              i18n={i18n}
            />
          </>
        )}

        <div className={styles.footer}>
          <button className={styles.closeFooterBtn} onClick={onClose}>
            {tCommon("actions.close")}
          </button>
        </div>
      </div>
    </div>
  );
}

function operationReason(op: PatchPreviewOperationSummary, t: TFunction<"patches">): string {
  if (op.target.kind === "def") {
    return t("previewDialog.reasonTargetsDefDirectly", {
      defType: op.target.defType,
      defName: op.target.defName,
    });
  }
  if (op.target.kind === "defType") {
    return t("previewDialog.reasonTargetsEveryDefType", { defType: op.target.defType });
  }
  if (op.target.kind === "defs") {
    return t("previewDialog.reasonTargetsDefsByName", { count: op.target.defNames.length });
  }
  return t("previewDialog.reasonUnknownStaticImpact");
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
  t: TFunction<"patches">;
  i18n: I18nInstance;
}

function OperationRow({ op, disabled, onToggleDisabled, onMove, t, i18n }: OperationRowProps) {
  return (
    <li className={styles.opRow}>
      <input
        type="checkbox"
        checked={!disabled}
        onChange={onToggleDisabled}
        aria-label={t("previewDialog.enableOperation", { className: op.className })}
      />
      <div className={styles.opMain}>
        <div className={styles.opTopLine}>
          <span className={styles.opClassName}>{op.className}</span>
          <span className={`${styles.opStatus} ${statusClass(op.status)}`}>
            {op.status ?? t("previewDialog.statusNotAvailable")}
          </span>
        </div>
        <div className={styles.opMeta}>
          {op.locationName} · {op.relativePath}
        </div>
        {op.xpath && <div className={styles.opXpath}>{op.xpath}</div>}
        <div className={styles.opReason}>{operationReason(op, t)}</div>
        {op.previewSupport.kind === "unsupported" && (
          <div className={styles.opUnsupported}>
            {t("previewDialog.previewUnsupported", { reason: op.previewSupport.reason })}
          </div>
        )}
        {op.statusMessage && (
          <div className={styles.opUnsupported}>
            {renderDiagnostic(
              { code: op.statusCode, args: op.statusArgs, message: op.statusMessage },
              i18n,
            )}
          </div>
        )}
      </div>
      {op.canReorder && (
        <div className={styles.opReorderBtns}>
          <button
            className={styles.reorderBtn}
            onClick={() => onMove(-1)}
            aria-label={t("previewDialog.moveUp", { className: op.className })}
            type="button"
          >
            <ChevronUp size={12} />
          </button>
          <button
            className={styles.reorderBtn}
            onClick={() => onMove(1)}
            aria-label={t("previewDialog.moveDown", { className: op.className })}
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
  i18n: I18nInstance;
}

function DiagnosticsSection({
  applyDiagnostics,
  inheritanceDiagnostics,
  conflictDiagnostics,
  i18n,
}: DiagnosticsSectionProps) {
  const total =
    applyDiagnostics.length + inheritanceDiagnostics.length + conflictDiagnostics.length;
  if (total === 0) return null;

  return (
    <div className={styles.diagnostics}>
      <div className={styles.diagnosticsHeader}>{renderDiagnosticSectionHeading(total, i18n)}</div>
      <ul className={styles.diagnosticsList} role="list">
        {conflictDiagnostics.map((d, i) => (
          <li key={`conflict-${i}`} className={`${styles.diagnosticItem} ${styles.severityWarning}`}>
            <span className={styles.diagnosticBadge}>{d.code}</span>
            {renderDiagnostic(d, i18n)}
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
            {renderDiagnostic(d, i18n)}
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
            {renderDiagnostic(d, i18n)}
          </li>
        ))}
      </ul>
    </div>
  );
}
