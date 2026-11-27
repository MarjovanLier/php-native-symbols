//! Architecture guard: every lifecycle version-boundary rule lives in
//! `src/lifecycle.rs`, so no other library module compares or destructures an
//! `added`, `deprecated` or `removed` field.

use std::fs;
use std::path::Path;

/// Whitespace-free shapes that compare or destructure a lifecycle field. The
/// scan removes all whitespace first, so a rustfmt line break between the field
/// and the comparison does not hide it. A trailing `{` catches the
/// `if let Some(v) = availability.added {` and `match availability.added {`
/// forms that compare the bound version on a later line.
const FORBIDDEN_SUFFIXES: [&str; 8] = [
    "==",
    "!=",
    "<",
    ">",
    ".is_some_and(",
    ".map_or(",
    ".filter(",
    "{",
];

#[test]
fn lifecycle_rules_live_only_in_the_lifecycle_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    // `read_dir` is not recursive, so `src/generated/` is never scanned.
    for entry in fs::read_dir(&src).expect("read src") {
        let path = entry.expect("src entry").path();
        let is_rust = path.extension().is_some_and(|ext| ext == "rs");
        if !is_rust || path.ends_with("lifecycle.rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read source file");
        // Unit-test modules sit at the end of each file and check data
        // invariants, not lifecycle rules.
        let library = text.split("#[cfg(test)]").next().unwrap_or_default();
        let code: String = library
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .flat_map(str::chars)
            .filter(|c| !c.is_whitespace())
            .collect();
        for field in [".added", ".deprecated", ".removed"] {
            for suffix in FORBIDDEN_SUFFIXES {
                if code.contains(&format!("{field}{suffix}")) {
                    offenders.push(format!("{}: `{field}{suffix}`", path.display()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "lifecycle comparisons outside src/lifecycle.rs: {offenders:#?}"
    );
}
