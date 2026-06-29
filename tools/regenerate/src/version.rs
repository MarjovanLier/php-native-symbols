/// Parse a `major.minor` version label such as `"8.1"` (minor required).
pub(crate) fn parse_mm(v: &str) -> (u8, u8) {
    let (major, minor) = v.split_once('.').expect("version label has a dot");
    (
        major.parse().expect("major is u8"),
        minor.parse().expect("minor is u8"),
    )
}

/// Parse a possibly-partial version string (`"8"`, `"8.4"`, `"8.4.1"`); missing
/// minor defaults to 0. Returns `None` if it cannot be read as `major[.minor]`.
pub(crate) fn parse_version_lenient(v: &str) -> Option<(u8, u8)> {
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |m| m.parse().ok())?;
    Some((major, minor))
}

/// `true` when phpstorm-stubs records no in-range introduction for a symbol:
/// no `@since`, or one that resolves to before the 7.4 floor.
pub(crate) fn since_is_prefloor(since: &Option<String>) -> bool {
    match since {
        None => true,
        Some(s) if s.trim().is_empty() => true,
        Some(s) => parse_version_lenient(s.trim()).is_some_and(|mm| mm < (7, 4)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_helpers_parse_generator_version_shapes() {
        assert_eq!(parse_mm("8.4"), (8, 4));
        assert_eq!(parse_version_lenient("8"), Some((8, 0)));
        assert_eq!(parse_version_lenient("8.4"), Some((8, 4)));
        assert_eq!(parse_version_lenient("8.4.12"), Some((8, 4)));
        assert_eq!(parse_version_lenient("8.x"), None);
        assert_eq!(parse_version_lenient("x.4"), None);

        assert!(since_is_prefloor(&None));
        assert!(since_is_prefloor(&Some(String::new())));
        assert!(since_is_prefloor(&Some("7.3".to_string())));
        assert!(!since_is_prefloor(&Some("7.4".to_string())));
        assert!(!since_is_prefloor(&Some("8.0.1".to_string())));
    }
}
