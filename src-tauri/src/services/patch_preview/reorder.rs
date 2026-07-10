use std::collections::HashSet;

use crate::patches::{PatchFile, PatchIndex, PatchOperationKey, TopLevelOperation};

pub(super) fn flatten_top_level_operations<'a>(
    index: &PatchIndex,
    files: &'a [PatchFile],
) -> Vec<TopLevelOperation<'a>> {
    let mut ops = Vec::new();
    for (file_idx, index_file) in index.files.iter().enumerate() {
        let Some(patch_file) = files.get(file_idx) else {
            continue;
        };
        for node in &patch_file.operations {
            ops.push(TopLevelOperation {
                location_id: index_file.source.location_id.clone(),
                relative_path: index_file.relative_path.clone(),
                node,
            });
        }
    }
    ops
}

/// Applies the caller's requested top-level reorder. Only the *slots* occupied by
/// reorder-eligible operations are touched: `requested_order` (filtered to truly-eligible, deduped
/// keys) fills those slots first in the order given, then any eligible operation the caller didn't
/// mention fills the remaining eligible slots in original relative order. Every non-eligible
/// operation stays exactly where it was in `default_order` -- reordering never moves an operation
/// across, before, or after an operation that doesn't affect the selected Def.
pub(super) fn apply_reorder<'a>(
    default_order: Vec<TopLevelOperation<'a>>,
    eligible: &HashSet<PatchOperationKey>,
    requested_order: &[PatchOperationKey],
) -> Vec<TopLevelOperation<'a>> {
    if requested_order.is_empty() || eligible.is_empty() {
        return default_order;
    }

    let key_of = |op: &TopLevelOperation<'a>| PatchOperationKey {
        location_id: op.location_id.clone(),
        relative_path: op.relative_path.clone(),
        operation_id: op.node.id,
    };

    let eligible_positions: HashSet<usize> = default_order
        .iter()
        .enumerate()
        .filter(|(_, op)| eligible.contains(&key_of(op)))
        .map(|(i, _)| i)
        .collect();
    if eligible_positions.is_empty() {
        return default_order;
    }

    let mut slots: Vec<Option<TopLevelOperation<'a>>> =
        default_order.into_iter().map(Some).collect();

    let mut new_sequence: Vec<TopLevelOperation<'a>> = Vec::with_capacity(eligible_positions.len());
    let mut placed: HashSet<PatchOperationKey> = HashSet::new();
    for key in requested_order {
        if placed.contains(key) || !eligible.contains(key) {
            continue;
        }
        let pos = eligible_positions
            .iter()
            .copied()
            .find(|&p| slots[p].as_ref().map(&key_of) == Some(key.clone()));
        if let Some(pos) = pos {
            if let Some(op) = slots[pos].take() {
                placed.insert(key.clone());
                new_sequence.push(op);
            }
        }
    }
    // Any eligible slot not covered by `requested_order` keeps its original relative order.
    let mut remaining_positions: Vec<usize> = eligible_positions.iter().copied().collect();
    remaining_positions.sort_unstable();
    for pos in remaining_positions {
        if let Some(op) = slots[pos].take() {
            new_sequence.push(op);
        }
    }

    let mut new_sequence_iter = new_sequence.into_iter();
    let mut result = Vec::with_capacity(slots.len());
    for (i, slot) in slots.into_iter().enumerate() {
        if eligible_positions.contains(&i) {
            result.push(
                new_sequence_iter
                    .next()
                    .expect("new_sequence has exactly eligible_positions.len() entries"),
            );
        } else {
            result.push(slot.expect("non-eligible slots were never taken"));
        }
    }
    result
}
