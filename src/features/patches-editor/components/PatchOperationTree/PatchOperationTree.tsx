import type { SchemaCatalog } from "../../../schema-catalog";
import { insertAt, moveItem, removeAt, replaceAt } from "../../lib/arrayUtils";
import { cloneWithFreshIds } from "../../lib/patchOperationDefaults";
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
}

/** Top-level operation list for a `<Patch>` file: add, remove, duplicate, and reorder operations,
 * recursing into `PatchOperationNodeRow` for nested sequence/conditional/find-mod operations. */
export function PatchOperationTree({ operations, catalog, readOnly, projectId, generateId, setOperations }: Props) {
  return (
    <div className={styles.root}>
      {operations.length === 0 && <p className={styles.empty}>This patch file has no operations yet.</p>}
      <ul className={styles.list}>
        {operations.map((op, i) => (
          <PatchOperationNodeRow
            key={op.id}
            node={op}
            catalog={catalog}
            readOnly={readOnly}
            projectId={projectId}
            depth={0}
            generateId={generateId}
            onChange={(updater) => setOperations((ops) => replaceAt(ops, i, updater(ops[i])))}
            onRemove={() => setOperations((ops) => removeAt(ops, i))}
            onDuplicate={() => setOperations((ops) => insertAt(ops, i + 1, cloneWithFreshIds(ops[i], generateId)))}
            onMoveUp={i > 0 ? () => setOperations((ops) => moveItem(ops, i, -1)) : undefined}
            onMoveDown={i < operations.length - 1 ? () => setOperations((ops) => moveItem(ops, i, 1)) : undefined}
          />
        ))}
      </ul>
      {!readOnly && (
        <PatchAddOperationPanel catalog={catalog} generateId={generateId} onAdd={(op) => setOperations((ops) => [...ops, op])} />
      )}
    </div>
  );
}
