import { memo, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { ChevronDown, ChevronRight, Code2, Copy, Trash2, ArrowUp, ArrowDown } from "lucide-react";
import type { SchemaCatalog } from "../../../schema-catalog";
import { useOperationListDispatch } from "../../lib/useOperationListDispatch";
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
  registerDraftFlush?: (flush: () => void) => () => void;
  onChange: (updater: (node: PatchOperationNode) => PatchOperationNode) => void;
  onRemove: () => void;
  onDuplicate?: () => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
}

/** Memoized so an edit to one row (which only replaces that row's own node in the tree state)
 * does not force every sibling/ancestor row to re-render -- see `useOperationListDispatch` for how
 * callers keep this row's callback props referentially stable across unrelated edits. */
export const PatchOperationNodeRow = memo(function PatchOperationNodeRow({
  node,
  catalog,
  readOnly,
  projectId,
  depth,
  generateId,
  registerDraftFlush,
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

  const updateSequenceChildren = useCallback(
    (updater: (ops: PatchOperationNode[]) => PatchOperationNode[]) => {
      onChange((n) => (n.kind.type === "sequence" ? { ...n, kind: { type: "sequence", data: updater(n.kind.data) } } : n));
    },
    [onChange],
  );

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
              <PatchOperationForm
                node={node}
                catalog={catalog}
                readOnly={readOnly}
                projectId={projectId}
                registerDraftFlush={registerDraftFlush}
                onChange={onChange}
              />

              {kind.type === "sequence" && (
                <div className={styles.nested}>
                  <ul className={styles.childList}>
                    {kind.data.map((child, i) => (
                      <SequenceChildRow
                        key={child.id}
                        node={child}
                        index={i}
                        total={kind.data.length}
                        catalog={catalog}
                        readOnly={readOnly}
                        projectId={projectId}
                        depth={depth + 1}
                        generateId={generateId}
                        registerDraftFlush={registerDraftFlush}
                        setList={updateSequenceChildren}
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
                    slot="matchOp"
                    op={kind.data.matchOp}
                    catalog={catalog}
                    readOnly={readOnly}
                    projectId={projectId}
                    depth={depth}
                    generateId={generateId}
                    registerDraftFlush={registerDraftFlush}
                    onChange={onChange}
                    t={t}
                  />
                  <MatchSlot
                    slot="nomatchOp"
                    op={kind.data.nomatchOp}
                    catalog={catalog}
                    readOnly={readOnly}
                    projectId={projectId}
                    depth={depth}
                    generateId={generateId}
                    registerDraftFlush={registerDraftFlush}
                    onChange={onChange}
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
});

interface SequenceChildRowProps {
  node: PatchOperationNode;
  index: number;
  total: number;
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  depth: number;
  generateId: () => PatchOperationId;
  registerDraftFlush?: (flush: () => void) => () => void;
  setList: (updater: (operations: PatchOperationNode[]) => PatchOperationNode[]) => void;
}

/** Same operation-id-based dispatch as `PatchOperationTree`'s top-level rows, scoped to one
 * `sequence` operation's nested child list instead of the file's top-level operation array. */
const SequenceChildRow = memo(function SequenceChildRow({
  node,
  index,
  total,
  catalog,
  readOnly,
  projectId,
  depth,
  generateId,
  registerDraftFlush,
  setList,
}: SequenceChildRowProps) {
  const { onChange, onRemove, onDuplicate, onMoveUp, onMoveDown } = useOperationListDispatch(
    node.id,
    setList,
    generateId,
  );
  return (
    <PatchOperationNodeRow
      node={node}
      catalog={catalog}
      readOnly={readOnly}
      projectId={projectId}
      depth={depth}
      generateId={generateId}
      registerDraftFlush={registerDraftFlush}
      onChange={onChange}
      onRemove={onRemove}
      onDuplicate={onDuplicate}
      onMoveUp={index > 0 ? onMoveUp : undefined}
      onMoveDown={index < total - 1 ? onMoveDown : undefined}
    />
  );
});

function MatchSlot({
  slot,
  op,
  catalog,
  readOnly,
  projectId,
  depth,
  generateId,
  registerDraftFlush,
  onChange: onRowChange,
  t,
}: {
  /** Which of `findMod`/`conditional`'s data fields this slot manages. */
  slot: "matchOp" | "nomatchOp";
  op: PatchOperationNode | null;
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  depth: number;
  generateId: () => PatchOperationId;
  registerDraftFlush?: (flush: () => void) => () => void;
  /** The containing row's own (stable, `useOperationListDispatch`-provided) onChange -- building
   * `onSet` from this rather than from a per-render-recreated inline callback keeps the nested
   * row's own onChange/onRemove props stable across edits to unrelated sibling fields (e.g.
   * `MayRequire`), so the memoized nested `PatchOperationNodeRow` doesn't re-render on every one
   * of those keystrokes. */
  onChange: (updater: (node: PatchOperationNode) => PatchOperationNode) => void;
  t: TFunction<"patches">;
}) {
  const label = slot === "matchOp" ? "match" : "nomatch";
  const onSet = useCallback(
    (newOp: PatchOperationNode | null) => {
      onRowChange((n): PatchOperationNode => {
        if (n.kind.type === "findMod") {
          return { ...n, kind: { type: "findMod", data: { ...n.kind.data, [slot]: newOp } } };
        }
        if (n.kind.type === "conditional") {
          return { ...n, kind: { type: "conditional", data: { ...n.kind.data, [slot]: newOp } } };
        }
        return n;
      });
    },
    [onRowChange, slot],
  );
  const onChildChange = useCallback(
    (updater: (n: PatchOperationNode) => PatchOperationNode) => {
      if (op) onSet(updater(op));
    },
    [onSet, op],
  );
  const onChildRemove = useCallback(() => onSet(null), [onSet]);

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
            registerDraftFlush={registerDraftFlush}
            onChange={onChildChange}
            onRemove={onChildRemove}
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
