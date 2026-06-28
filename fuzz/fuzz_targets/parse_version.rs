#![no_main]
//! Fuzz `PhpVersion` parsing. Arbitrary input must never panic, and any string
//! that parses must round-trip through its rendered `major.minor.patch` form.

use libfuzzer_sys::fuzz_target;
use php_native_symbols::PhpVersion;

fuzz_target!(|data: &str| {
    if let Ok(version) = data.parse::<PhpVersion>() {
        let rendered = format!("{}.{}.{}", version.major, version.minor, version.patch);
        let reparsed: PhpVersion = rendered.parse().expect("a rendered version must reparse");
        assert_eq!(version, reparsed, "parse is not stable for {data:?}");
    }
});
