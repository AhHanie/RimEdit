import { useCallback } from "react";
import { insertAt, moveItem } from "./arrayUtils";
import { cloneWithFreshIds } from "./patchOperationDefaults";
import type { PatchOperationId, PatchOperationNode } from "../types/patchFile";

export interface OperationListDispatch {
  onChange: (updater: (node: PatchOperationNode) => PatchOperationNode) => void;
  onRemove: () => void;
  onDuplicate: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}

/** Builds `onChange`/`onRemove`/`onDuplicate`/`onMoveUp`/`onMoveDown` callbacks for one operation
 * within a list, dispatched by the operation's stable `id` (found via `findIndex` at call time)
 * rather than a closed-over array index. `setList`/`id`/`generateId` are the only dependencies, so
 * these callbacks keep the same identity across a `setList` update that only touched a *different*
 * operation -- the "operation-id-based dispatcher" `React.memo`'d rows rely on to avoid
 * re-rendering their whole subtree on an unrelated sibling's edit (Plan.md's tree-isolation goal).
 * `onMoveUp`/`onMoveDown` are always safe to call (a no-op at the list's boundary); callers gate
 * whether to offer them at all using the current index/length, which is expected to change (and
 * isn't meant to stay referentially stable) across reorders. */
export function useOperationListDispatch(
  id: PatchOperationId,
  setList: (updater: (operations: PatchOperationNode[]) => PatchOperationNode[]) => void,
  generateId: () => PatchOperationId,
): OperationListDispatch {
  const onChange = useCallback(
    (updater: (node: PatchOperationNode) => PatchOperationNode) => {
      setList((ops) => ops.map((o) => (o.id === id ? updater(o) : o)));
    },
    [setList, id],
  );

  const onRemove = useCallback(() => {
    setList((ops) => ops.filter((o) => o.id !== id));
  }, [setList, id]);

  const onDuplicate = useCallback(() => {
    setList((ops) => {
      const idx = ops.findIndex((o) => o.id === id);
      return idx === -1 ? ops : insertAt(ops, idx + 1, cloneWithFreshIds(ops[idx], generateId));
    });
  }, [setList, id, generateId]);

  const onMoveUp = useCallback(() => {
    setList((ops) => {
      const idx = ops.findIndex((o) => o.id === id);
      return idx <= 0 ? ops : moveItem(ops, idx, -1);
    });
  }, [setList, id]);

  const onMoveDown = useCallback(() => {
    setList((ops) => {
      const idx = ops.findIndex((o) => o.id === id);
      return idx === -1 || idx >= ops.length - 1 ? ops : moveItem(ops, idx, 1);
    });
  }, [setList, id]);

  return { onChange, onRemove, onDuplicate, onMoveUp, onMoveDown };
}
