import { memo } from "react";
import { useTranslation } from "react-i18next";
import type { SchemaCatalog } from "../../../schema-catalog";
import { useOperationListDispatch } from "../../lib/useOperationListDispatch";
import type { PatchOperationId, PatchOperationNode } from "../../types/patchFile";
import { PatchAddOperationPanel } from "../PatchAddOperationPanel/PatchAddOperationPanel";
import { PatchOperationNodeRow } from "../PatchOperationNodeRow/PatchOperationNodeRow";
import styles from "./PatchOperationTree.module.css";

interface Props {
  operations: PatchOperationNode[];
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  generateId: () => PatchOperationId;
  setOperations: (updater: (operations: PatchOperationNode[]) => PatchOperationNode[]) => void;
  registerDraftFlush?: (flush: () => void) => () => void;
}

/** Top-level operation list for a `<Patch>` file: add, remove, duplicate, and reorder operations,
 * recursing into `PatchOperationNodeRow` for nested sequence/conditional/find-mod operations. */
export function PatchOperationTree({
  operations,
  catalog,
  readOnly,
  projectId,
  generateId,
  setOperations,
  registerDraftFlush,
}: Props) {
  const { t } = useTranslation("patches");
  return (
    <div className={styles.root}>
      {operations.length === 0 && (
        <p className={styles.empty}>{t("operationTree.empty")}</p>
      )}
      <ul className={styles.list}>
        {operations.map((op, i) => (
          <TopLevelOperationRow
            key={op.id}
            node={op}
            index={i}
            total={operations.length}
            catalog={catalog}
            readOnly={readOnly}
            projectId={projectId}
            generateId={generateId}
            setOperations={setOperations}
            registerDraftFlush={registerDraftFlush}
          />
        ))}
      </ul>
      {!readOnly && (
        <PatchAddOperationPanel catalog={catalog} generateId={generateId} onAdd={(op) => setOperations((ops) => [...ops, op])} />
      )}
    </div>
  );
}

interface TopLevelOperationRowProps {
  node: PatchOperationNode;
  index: number;
  total: number;
  catalog: SchemaCatalog | null;
  readOnly: boolean;
  projectId: string | null;
  generateId: () => PatchOperationId;
  setOperations: (updater: (operations: PatchOperationNode[]) => PatchOperationNode[]) => void;
  registerDraftFlush?: (flush: () => void) => () => void;
}

/** Builds this row's `onChange`/`onRemove`/`onDuplicate`/`onMoveUp`/`onMoveDown` callbacks keyed
 * by the operation's stable id rather than its array index, so their identity survives a
 * `setOperations` update to a *different* row untouched (Plan.md's "operation-id-based
 * dispatcher") -- combined with `React.memo` on `PatchOperationNodeRow`, editing one row no longer
 * forces every other row's subtree to re-render just because the top-level `operations` array got
 * a new reference. */
const TopLevelOperationRow = memo(function TopLevelOperationRow({
  node,
  index,
  total,
  catalog,
  readOnly,
  projectId,
  generateId,
  setOperations,
  registerDraftFlush,
}: TopLevelOperationRowProps) {
  const { onChange, onRemove, onDuplicate, onMoveUp, onMoveDown } = useOperationListDispatch(
    node.id,
    setOperations,
    generateId,
  );
  return (
    <PatchOperationNodeRow
      node={node}
      catalog={catalog}
      readOnly={readOnly}
      projectId={projectId}
      depth={0}
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
