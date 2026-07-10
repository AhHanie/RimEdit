import type { PatchOperationKey, PatchPreviewOperationSummary } from "../types/patchPreview";
import { patchOperationKeyToString } from "../types/patchPreview";

/** Applies a local (preview-only) reorder override to a default-ordered list of operations,
 * mirroring the backend's `services::patch_preview::apply_reorder` slot-filling semantics exactly:
 * only positions occupied by a `canReorder` operation are touched. `desiredOrder` fills those
 * slots first (skipping keys that aren't actually reorder-eligible in `defaultOrder`), then any
 * eligible operation not mentioned keeps its original relative order. Every non-eligible operation
 * stays exactly where it was. */
export function applyLocalReorder(
  defaultOrder: PatchPreviewOperationSummary[],
  desiredOrder: PatchOperationKey[] | null,
): PatchPreviewOperationSummary[] {
  if (!desiredOrder || desiredOrder.length === 0) return defaultOrder;

  const byKey = new Map(
    defaultOrder.map((op) => [patchOperationKeyToString(op.key), op] as const),
  );
  const eligiblePositions = defaultOrder
    .map((op, i) => (op.canReorder ? i : -1))
    .filter((i) => i !== -1);
  if (eligiblePositions.length === 0) return defaultOrder;

  const placed = new Set<string>();
  const newSequence: PatchPreviewOperationSummary[] = [];
  for (const key of desiredOrder) {
    const k = patchOperationKeyToString(key);
    if (placed.has(k)) continue;
    const op = byKey.get(k);
    if (!op || !op.canReorder) continue;
    placed.add(k);
    newSequence.push(op);
  }
  for (const i of eligiblePositions) {
    const op = defaultOrder[i];
    const k = patchOperationKeyToString(op.key);
    if (!placed.has(k)) {
      placed.add(k);
      newSequence.push(op);
    }
  }

  const result: PatchPreviewOperationSummary[] = [];
  let seqIdx = 0;
  for (let i = 0; i < defaultOrder.length; i++) {
    if (eligiblePositions.includes(i)) {
      result.push(newSequence[seqIdx]);
      seqIdx++;
    } else {
      result.push(defaultOrder[i]);
    }
  }
  return result;
}
