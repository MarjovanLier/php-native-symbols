use std::hint::black_box;
use std::time::{Duration, Instant};

use php_native_symbols::{
    callable_method_availability, class_availability, compatibility_report_at,
    constant_availability, function_availability, method_availability, resolve_class,
    resolve_function, resolve_method, PhpVersion, SymbolRef,
};

const ITERS: usize = 100_000;
const PHP_82: PhpVersion = PhpVersion::minor(8, 2);

fn bench<T>(label: &str, mut f: impl FnMut(usize) -> T) {
    let start = Instant::now();
    for i in 0..ITERS {
        black_box(f(black_box(i)));
    }
    report(label, start.elapsed());
}

fn report(label: &str, elapsed: Duration) {
    let nanos = elapsed.as_nanos() / ITERS as u128;
    println!("{label}: {nanos} ns/op over {ITERS} iterations");
}

fn main() {
    let functions = ["strlen", "\\STR_CONTAINS", "utf8_encode", "create_function"];
    let constants = [
        "PHP_INT_MAX",
        "\\FILTER_VALIDATE_BOOL",
        "JSON_THROW_ON_ERROR",
    ];
    let classes = ["Random\\Randomizer", "\\weakreference", "SplStack"];
    let methods = [
        ("Random\\Randomizer", "getFloat"),
        ("ReflectionParameter", "getClass"),
        ("SplDoublyLinkedList", "push"),
    ];
    let compatibility = [
        SymbolRef::Function("strlen"),
        SymbolRef::Function("str_contains"),
        SymbolRef::Function("utf8_encode"),
        SymbolRef::Constant("PHP_INT_MAX"),
        SymbolRef::Method {
            class: "Random\\Randomizer",
            method: "getFloat",
        },
    ];

    bench("direct function lookup", |i| {
        function_availability(functions[i % functions.len()])
    });
    bench("direct constant lookup", |i| {
        constant_availability(constants[i % constants.len()])
    });
    bench("direct class lookup", |i| {
        class_availability(classes[i % classes.len()])
    });
    bench("direct method lookup", |i| {
        let (class, method) = methods[i % methods.len()];
        method_availability(class, method)
    });
    bench("canonical resolution", |i| match i % 3 {
        0 => resolve_function(functions[i % functions.len()]).map(|(name, _)| name),
        1 => resolve_class(classes[i % classes.len()]).map(|(name, _)| name),
        _ => {
            let (class, method) = methods[i % methods.len()];
            resolve_method(class, method).map(|(class, method, _)| {
                black_box(class);
                method
            })
        }
    });
    bench("compatibility report", |_| {
        compatibility_report_at(compatibility, PHP_82)
    });
    bench("callable method lookup", |_| {
        callable_method_availability("SplStack", "push")
    });
}
