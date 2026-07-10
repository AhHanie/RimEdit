use crate::patches::{
    PatchFile, PatchImpactRef, PatchIndex, PatchOperationId, PatchOperationKey, PatchOperationKind,
    PatchOperationNode,
};

/// Looks up an operation's `class_name` by its `PatchImpactRef` (file + in-file id). Used to name
/// a specific conflicting operation in a diagnostic message rather than leaving it anonymous.
pub(super) fn operation_class_name<'a>(
    patch_index: &'a PatchIndex,
    reference: &PatchImpactRef,
) -> Option<&'a str> {
    patch_index
        .files
        .iter()
        .find(|f| {
            f.source.location_id == reference.location_id
                && f.relative_path == reference.relative_path
        })
        .and_then(|f| {
            f.operations
                .iter()
                .find(|op| op.id == reference.operation_id)
        })
        .map(|op| op.class_name.as_str())
}

/// Finds the actual parsed [`PatchOperationNode`] a [`PatchOperationKey`] identifies, searching
/// the whole operation tree (any nesting depth) of the file it names -- `operation_id` is unique
/// per file, not scoped to top-level operations, so a key can name a `Sequence`/`Conditional`/
/// `FindMod` child.
pub(super) fn find_operation_node<'a>(
    patch_index: &PatchIndex,
    patch_files: &'a [PatchFile],
    key: &PatchOperationKey,
) -> Option<&'a PatchOperationNode> {
    let file_idx = patch_index.files.iter().position(|f| {
        f.source.location_id == key.location_id && f.relative_path == key.relative_path
    })?;
    let patch_file = patch_files.get(file_idx)?;
    find_operation_node_in(&patch_file.operations, key.operation_id)
}

fn find_operation_node_in(
    nodes: &[PatchOperationNode],
    id: PatchOperationId,
) -> Option<&PatchOperationNode> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        let found = match &node.kind {
            PatchOperationKind::Sequence(children) => find_operation_node_in(children, id),
            PatchOperationKind::FindMod {
                match_op,
                nomatch_op,
                ..
            }
            | PatchOperationKind::Conditional {
                match_op,
                nomatch_op,
                ..
            } => match_op
                .as_deref()
                .and_then(|m| find_operation_node_in(std::slice::from_ref(m), id))
                .or_else(|| {
                    nomatch_op
                        .as_deref()
                        .and_then(|nm| find_operation_node_in(std::slice::from_ref(nm), id))
                }),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}
