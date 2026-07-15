import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { ChevronDown, ChevronRight, Code2, Copy, Trash2, ArrowUp, ArrowDown } from "lucide-react";
import type { SchemaCatalog } from "../../../schema-catalog";
import { insertAt, removeAt, replaceAt, moveItem } from "../../lib/arrayUtils";
import { cloneWithFreshIds } from "../../lib/patchOperationDefaults";
import { operationSubtitle, operationTitle } from "../../lib/operationSummary";
import type { PatchOperationId, PatchOperationNode } from "../../types/patchFile";
import { PatchAddOperationPanel } from "../PatchAddOperationPanel/PatchAddOperationPanel";
import { PatchOperationForm } from "../PatchOperationForm/PatchOperationForm";
import styles from "./PatchOperationNodeRow.module.css";

export interface PatchOperationNodeRowProps {
  node: PatchOperationNode;
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  depth: number;
  generateId: () => PatchOperationId;
  onChange: (updater: (node: PatchOperationNode) => PatchOperationNode) => void;
  onRemove: () => void;
  onDuplicate?: () => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
}

export function PatchOperationNodeRow({
  node,
  catalog,
  readOnly,
  projectId,
  depth,
  generateId,
  onChange,
  onRemove,
  onDuplicate,
  onMoveUp,
  onMoveDown,
}: PatchOperationNodeRowProps) {
  const { t } = useTranslation("patches");
  const [expanded, setExpanded] = useState(true);
  const kind = node.kind;
  const isUnknown = kind.type === "unknown";

  function updateSequenceChildren(updater: (ops: PatchOperationNode[]) => PatchOperationNode[]) {
    onChange((n) => (n.kind.type === "sequence" ? { ...n, kind: { type: "sequence", data: updater(n.kind.data) } } : n));
  }

  function updateSlot(slot: "matchOp" | "nomatchOp", op: PatchOperationNode | null) {
    onChange((n): PatchOperationNode => {
      if (n.kind.type === "findMod") {
        return { ...n, kind: { type: "findMod", data: { ...n.kind.data, [slot]: op } } };
      }
      if (n.kind.type === "conditional") {
        return { ...n, kind: { type: "conditional", data: { ...n.kind.data, [slot]: op } } };
      }
      return n;
    });
  }

  return (
    <li className={styles.row} style={{ marginInlineStart: depth > 0 ? 16 : 0 }}>
      <div className={styles.summary}>
        <button
          type="button"
          className={styles.expandBtn}
          onClick={() => setExpanded((v) => !v)}
          aria-label={
            expanded ? t("operationRow.collapseOperation") : t("operationRow.expandOperation")
          }
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>
        {isUnknown && (
          <Code2 size={12} className={styles.rawBadge} aria-label={t("operationRow.rawXmlOnly")} />
        )}
        <span className={styles.title}>{operationTitle(node, catalog)}</span>
        {operationSubtitle(node, t) && (
          <span className={styles.subtitle}>{operationSubtitle(node, t)}</span>
        )}
        {node.success !== "normal" && <span className={styles.badge}>{node.success}</span>}
        {!readOnly && (
          <div className={styles.actions}>
            {onMoveUp && (
              <button
                type="button"
                className={styles.iconBtn}
                onClick={onMoveUp}
                aria-label={t("operationRow.moveUp")}
              >
                <ArrowUp size={12} />
              </button>
            )}
            {onMoveDown && (
              <button
                type="button"
                className={styles.iconBtn}
                onClick={onMoveDown}
                aria-label={t("operationRow.moveDown")}
              >
                <ArrowDown size={12} />
              </button>
            )}
            {onDuplicate && (
              <button
                type="button"
                className={styles.iconBtn}
                onClick={onDuplicate}
                aria-label={t("operationRow.duplicateOperation")}
              >
                <Copy size={12} />
              </button>
            )}
            <button
              type="button"
              className={styles.iconBtn}
              onClick={onRemove}
              aria-label={t("operationRow.removeOperation")}
            >
              <Trash2 size={12} />
            </button>
          </div>
        )}
      </div>

      {expanded && (
        <div className={styles.body}>
          {isUnknown ? (
            <label className={styles.rawField}>
              <span className={styles.rawLabel}>{t("operationRow.rawXmlLabel")}</span>
              <textarea
                rows={6}
                spellCheck={false}
                className={styles.rawXml}
                // XML is machine-readable syntax, not natural-language prose -- keep it forced
                // LTR even once a future RTL locale flips `dir` on `<html>` (see
                // docs/i18n/issues/08-editor-and-patch-ui-migration.md's "keep code editor/XML/
                // XPath controls dir=ltr by semantic policy" carve-out).
                dir="ltr"
                value={kind.data.rawXml}
                disabled={readOnly}
                onChange={(e) =>
                  onChange((n) =>
                    n.kind.type === "unknown"
                      ? { ...n, kind: { type: "unknown", data: { rawXml: e.target.value } } }
                      : n,
                  )
                }
              />
            </label>
          ) : (
            <>
              <PatchOperationForm node={node} catalog={catalog} readOnly={readOnly} projectId={projectId} onChange={onChange} />

              {kind.type === "sequence" && (
                <div className={styles.nested}>
                  <ul className={styles.childList}>
                    {kind.data.map((child, i) => (
                      <PatchOperationNodeRow
                        key={child.id}
                        node={child}
                        catalog={catalog}
                        readOnly={readOnly}
                        projectId={projectId}
                        depth={depth + 1}
                        generateId={generateId}
                        onChange={(updater) => updateSequenceChildren((ops) => replaceAt(ops, i, updater(ops[i])))}
                        onRemove={() => updateSequenceChildren((ops) => removeAt(ops, i))}
                        onDuplicate={() =>
                          updateSequenceChildren((ops) =>
                            insertAt(ops, i + 1, cloneWithFreshIds(ops[i], generateId)),
                          )
                        }
                        onMoveUp={i > 0 ? () => updateSequenceChildren((ops) => moveItem(ops, i, -1)) : undefined}
                        onMoveDown={
                          i < kind.data.length - 1
                            ? () => updateSequenceChildren((ops) => moveItem(ops, i, 1))
                            : undefined
                        }
                      />
                    ))}
                  </ul>
                  {!readOnly && (
                    <PatchAddOperationPanel
                      catalog={catalog}
                      generateId={generateId}
                      slot="sequenceChild"
                      triggerLabel={t("operationRow.addSequenceOperation")}
                      onAdd={(op) => updateSequenceChildren((ops) => [...ops, op])}
                    />
                  )}
                </div>
              )}

              {(kind.type === "findMod" || kind.type === "conditional") && (
                <div className={styles.nested}>
                  <MatchSlot
                    label="match"
                    op={kind.data.matchOp}
                    catalog={catalog}
                    readOnly={readOnly}
                    projectId={projectId}
                    depth={depth}
                    generateId={generateId}
                    onSet={(op) => updateSlot("matchOp", op)}
                    t={t}
                  />
                  <MatchSlot
                    label="nomatch"
                    op={kind.data.nomatchOp}
                    catalog={catalog}
                    readOnly={readOnly}
                    projectId={projectId}
                    depth={depth}
                    generateId={generateId}
                    onSet={(op) => updateSlot("nomatchOp", op)}
                    t={t}
                  />
                </div>
              )}
            </>
          )}
        </div>
      )}
    </li>
  );
}

function MatchSlot({
  label,
  op,
  catalog,
  readOnly,
  projectId,
  depth,
  generateId,
  onSet,
  t,
}: {
  label: "match" | "nomatch";
  op: PatchOperationNode | null;
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  depth: number;
  generateId: () => PatchOperationId;
  onSet: (op: PatchOperationNode | null) => void;
  t: TFunction<"patches">;
}) {
  return (
    <div className={styles.slot}>
      <span className={styles.slotLabel}>{label}</span>
      {op ? (
        <ul className={styles.childList}>
          <PatchOperationNodeRow
            node={op}
            catalog={catalog}
            readOnly={readOnly}
            projectId={projectId}
            depth={depth + 1}
            generateId={generateId}
            onChange={(updater) => onSet(updater(op))}
            onRemove={() => onSet(null)}
          />
        </ul>
      ) : (
        !readOnly && (
          <PatchAddOperationPanel
            catalog={catalog}
            generateId={generateId}
            slot={label}
            triggerLabel={t("operationRow.setSlotOperation", { slot: label })}
            onAdd={(newOp) => onSet(newOp)}
          />
        )
      )}
    </div>
  );
}
