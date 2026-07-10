pub(super) fn is_mask_stem(stem: &str) -> bool {
    let lower = stem.to_lowercase();
    lower.ends_with("_m")
        || lower.ends_with("_mask")
        || lower.ends_with("_northm")
        || lower.ends_with("_eastm")
        || lower.ends_with("_southm")
        || lower.ends_with("_westm")
}

pub(super) const DIRECTION_SUFFIXES: [(&str, &str, &str); 4] = [
    ("_north", "north", "North"),
    ("_east", "east", "East"),
    ("_south", "south", "South"),
    ("_west", "west", "West"),
];

/// Returns `(role, label, suffix_len)` when `stem` ends with a directional suffix.
pub(super) fn detect_direction(stem: &str) -> Option<(&'static str, &'static str, usize)> {
    let lower = stem.to_lowercase();
    for (suffix, role, label) in &DIRECTION_SUFFIXES {
        if lower.ends_with(suffix) {
            return Some((role, label, suffix.len()));
        }
    }
    None
}

pub(super) fn stack_count_label(stem: &str, index: usize) -> String {
    let lower = stem.to_lowercase();
    if lower.contains("single") || lower.ends_with('1') {
        "Stack 1".to_string()
    } else if lower.contains("partial") || lower.ends_with('2') {
        "Stack partial".to_string()
    } else if lower.contains("full") || lower.ends_with('3') {
        "Stack full".to_string()
    } else {
        format!("Variant {}", index + 1)
    }
}

pub(super) fn appearance_label(stem: &str, base_name_lower: &str, index: usize) -> String {
    let stem_lower = stem.to_lowercase();
    if stem_lower.starts_with(base_name_lower) {
        let suffix = &stem[base_name_lower.len()..];
        let suffix = suffix.trim_start_matches('_');
        if !suffix.is_empty() {
            return to_title_case(suffix);
        }
    }
    format!("Appearance {}", index + 1)
}

pub(super) fn to_title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
