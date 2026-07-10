/// Returns `true` if `key` is a valid XML element name.
///
/// Rejects empty strings, strings starting with a digit, hyphen, or period,
/// and strings containing whitespace or XML delimiter characters.
pub(crate) fn is_valid_xml_name(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let first = key.chars().next().unwrap();
    if first.is_ascii_digit() || first == '-' || first == '.' {
        return false;
    }
    key.chars().all(|c| {
        c != ' '
            && c != '\t'
            && c != '\n'
            && c != '\r'
            && c != '<'
            && c != '>'
            && c != '&'
            && c != '"'
            && c != '\''
            && c != '/'
            && c != '?'
            && c != '!'
            && c != '='
            && c != '['
            && c != ']'
    })
}
