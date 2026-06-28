//! Allocation-free lookup helpers for sorted generated tables.

use std::cmp::Ordering;

/// Strip one leading namespace separator from a PHP symbol name.
pub(crate) fn strip_one_leading_backslash(name: &str) -> &str {
    name.strip_prefix('\\').unwrap_or(name)
}

/// Compare a canonical table key to a caller query using PHP's ASCII
/// case-insensitive function, class and method lookup semantics.
pub(crate) fn cmp_ascii_case_insensitive_key(candidate: &str, query: &str) -> Ordering {
    candidate
        .bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(query.bytes().map(|byte| byte.to_ascii_lowercase()))
}

/// Binary-search a sorted table by an ASCII case-insensitive string key without
/// allocating a normalised query string.
pub(crate) fn binary_search_ascii_case_insensitive<T>(
    table: &[T],
    query: &str,
    key: impl Fn(&T) -> &str,
) -> Option<usize> {
    table
        .binary_search_by(|entry| cmp_ascii_case_insensitive_key(key(entry), query))
        .ok()
}

/// Binary-search a sorted table by an ASCII case-insensitive pair key without
/// allocating normalised class or method strings.
pub(crate) fn binary_search_ascii_case_insensitive_pair<T>(
    table: &[T],
    left_query: &str,
    right_query: &str,
    key: impl Fn(&T) -> (&str, &str),
) -> Option<usize> {
    table
        .binary_search_by(|entry| {
            let (left, right) = key(entry);
            cmp_ascii_case_insensitive_key(left, left_query)
                .then_with(|| cmp_ascii_case_insensitive_key(right, right_query))
        })
        .ok()
}
