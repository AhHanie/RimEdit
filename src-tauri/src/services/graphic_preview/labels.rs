use super::model::{Direction, GraphicPreviewLabel, StackSlot};

pub(super) fn is_mask_stem(stem: &str) -> bool {
    let lower = stem.to_lowercase();
    lower.ends_with("_m")
        || lower.ends_with("_mask")
        || lower.ends_with("_northm")
        || lower.ends_with("_eastm")
        || lower.ends_with("_southm")
        || lower.ends_with("_westm")
}

pub(super) const DIRECTION_SUFFIXES: [(&str, &str, Direction); 4] = [
    ("_north", "north", Direction::North),
    ("_east", "east", Direction::East),
    ("_south", "south", Direction::South),
    ("_west", "west", Direction::West),
];

/// Returns `(role, direction, suffix_len)` when `stem` ends with a directional suffix.
pub(super) fn detect_direction(stem: &str) -> Option<(&'static str, Direction, usize)> {
    let lower = stem.to_lowercase();
    for (suffix, role, direction) in &DIRECTION_SUFFIXES {
        if lower.ends_with(suffix) {
            return Some((role, *direction, suffix.len()));
        }
    }
    None
}

/// Classifies a stack-count texture's file stem into which of the three stack slots it
/// represents, or falls back to a plain `Variant` index when the stem doesn't match any
/// recognized slot naming convention.
pub(super) fn stack_count_label(stem: &str, index: usize) -> GraphicPreviewLabel {
    let lower = stem.to_lowercase();
    if lower.contains("single") || lower.ends_with('1') {
        GraphicPreviewLabel::Stack {
            stack: StackSlot::Single,
            direction: None,
        }
    } else if lower.contains("partial") || lower.ends_with('2') {
        GraphicPreviewLabel::Stack {
            stack: StackSlot::Partial,
            direction: None,
        }
    } else if lower.contains("full") || lower.ends_with('3') {
        GraphicPreviewLabel::Stack {
            stack: StackSlot::Full,
            direction: None,
        }
    } else {
        GraphicPreviewLabel::Variant {
            index: index + 1,
            direction: None,
        }
    }
}

/// Classifies an appearance-scan texture's file stem into a named-suffix label (the part of the
/// stem after the base name, e.g. `"Damaged"` from `Blocks_Damaged`) or a plain `Appearance`
/// index fallback when the stem carries no distinguishing suffix.
pub(super) fn appearance_label(
    stem: &str,
    base_name_lower: &str,
    index: usize,
) -> GraphicPreviewLabel {
    let stem_lower = stem.to_lowercase();
    if stem_lower.starts_with(base_name_lower) {
        let suffix = &stem[base_name_lower.len()..];
        let suffix = suffix.trim_start_matches('_');
        if !suffix.is_empty() {
            return GraphicPreviewLabel::AppearanceNamed {
                suffix: to_title_case(suffix),
            };
        }
    }
    GraphicPreviewLabel::Appearance { index: index + 1 }
}

pub(super) fn to_title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
