use std::error::Error;
use std::path::Path;
use std::process::Command;

/// phpstorm-stubs commit the committed tables are generated from. The checkout's
/// HEAD is verified against this before generation (unless overridden).
pub(crate) const PHPSTORM_STUBS_SHA: &str = "7f1c9cada07266d488698b6c9128503d6c94e58b";

/// PHP-CS-Fixer release the `@compiler_optimized` set below was taken from.
pub(crate) const PHP_CS_FIXER_TAG: &str = "v3.95.11";

/// PHPCompatibility commit the cross-check is verified against. The checkout's
/// HEAD is verified against this before generation (unless overridden).
pub(crate) const PHPCOMPATIBILITY_SHA: &str = "d9a91bdf66d39fbd5c22272a592c8b63a1d0954f";

/// Absent baseline: symbols present here predate the 7.4 coverage floor.
pub(crate) const BASELINE: &str = "7.3";

/// The reported coverage range, earliest first. `added` is the earliest of
/// these in which a symbol appears (or `None` if it predates the floor).
pub(crate) const RANGE: &[&str] = &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4", "8.5"];

/// Read `git rev-parse HEAD` for a checkout.
pub(crate) fn head_sha(dir: &Path) -> Result<String, Box<dyn Error>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("running git in {}: {e}", dir.display()))?;
    if !out.status.success() {
        return Err(format!("git rev-parse failed in {}", dir.display()).into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}
