use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation};

pub(super) fn build_search_locations<'a>(
    settings: &'a ProjectSettings,
    project_id: &str,
) -> Vec<&'a RegisteredLocation> {
    let mut result: Vec<&RegisteredLocation> = Vec::new();
    if let Some(loc) = settings
        .locations
        .iter()
        .find(|l| l.id == project_id && l.kind == LocationKind::Project)
    {
        result.push(loc);
    }
    let sources: Vec<_> = settings
        .locations
        .iter()
        .filter(|l| l.kind == LocationKind::Source)
        .collect();
    for loc in sources.into_iter().rev() {
        result.push(loc);
    }
    result
}
