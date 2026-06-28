#![no_main]
//! Fuzz the name-normalisation and binary-search lookups with arbitrary input.
//! They must never panic, and each `is_*` predicate must agree with its
//! `*_availability` counterpart: a predicate can only be true for a known symbol.

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use php_native_symbols::{
    class_availability, constant_availability, function_availability, is_class, is_class_available,
    is_class_deprecated_at, is_constant, is_constant_available, is_constant_deprecated_at,
    is_function, is_function_available, is_function_deprecated_at, is_method, is_method_available,
    is_method_deprecated_at, method_availability, PhpVersion,
};

/// Arbitrary lookup inputs: a single-symbol name, a `(class, method)` pair, and
/// a version triple, all unconstrained so the parser and lookups see junk.
#[derive(Arbitrary, Debug)]
struct Input<'a> {
    name: &'a str,
    class: &'a str,
    method: &'a str,
    version: (u8, u8, u8),
}

fuzz_target!(|input: Input| {
    let Input {
        name,
        class,
        method,
        version,
    } = input;
    let version = PhpVersion::new(version.0, version.1, version.2);

    // Each membership predicate must mirror its availability lookup exactly.
    assert_eq!(is_function(name), function_availability(name).is_some());
    assert_eq!(is_constant(name), constant_availability(name).is_some());
    assert_eq!(is_class(name), class_availability(name).is_some());
    assert_eq!(
        is_method(class, method),
        method_availability(class, method).is_some()
    );

    // A version-gated predicate can only be true for a symbol that is known.
    if is_function_available(name, version) || is_function_deprecated_at(name, version) {
        assert!(is_function(name));
    }
    if is_constant_available(name, version) || is_constant_deprecated_at(name, version) {
        assert!(is_constant(name));
    }
    if is_class_available(name, version) || is_class_deprecated_at(name, version) {
        assert!(is_class(name));
    }
    if is_method_available(class, method, version)
        || is_method_deprecated_at(class, method, version)
    {
        assert!(is_method(class, method));
    }
});
