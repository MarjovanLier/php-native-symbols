use std::collections::HashSet;

use php_native_symbols::{
    callable_method_availability, class_availability, classes, classes_available_at,
    constant_availability, constants, constants_available_at, function_availability, functions,
    functions_available_at, is_callable_method, is_callable_method_available,
    is_callable_method_deprecated_at, is_class, is_class_available, is_class_deprecated_at,
    is_constant, is_constant_available, is_constant_deprecated_at, is_core_extension, is_function,
    is_function_available, is_function_deprecated_at, is_method, is_method_available,
    is_method_deprecated_at, method_availability, methods, methods_available_at, Availability,
    CallableMethod, ParsePhpVersionError, PhpVersion, SymbolKind,
};

const PHP_74: PhpVersion = PhpVersion::minor(7, 4);
const PHP_80: PhpVersion = PhpVersion::minor(8, 0);
const PHP_81: PhpVersion = PhpVersion::minor(8, 1);
const PHP_82: PhpVersion = PhpVersion::minor(8, 2);
const PHP_83: PhpVersion = PhpVersion::minor(8, 3);
const PHP_84: PhpVersion = PhpVersion::minor(8, 4);
const PHP_85: PhpVersion = PhpVersion::minor(8, 5);

fn cloned_through_trait<T: Clone>(value: &T) -> T {
    value.clone()
}

fn parse_error(input: &str) -> ParsePhpVersionError {
    input
        .parse::<PhpVersion>()
        .expect_err("input should not parse as a PHP version")
}

#[test]
fn php_version_construction_parsing_and_error_traits_are_public() {
    assert_eq!(PhpVersion::new(8, 1, 3).major, 8);
    assert_eq!(PhpVersion::new(8, 1, 3).minor, 1);
    assert_eq!(PhpVersion::new(8, 1, 3).patch, 3);
    assert_eq!(PhpVersion::minor(8, 1), PhpVersion::new(8, 1, 0));

    assert_eq!("8".parse::<PhpVersion>(), Ok(PhpVersion::new(8, 0, 0)));
    assert_eq!("8.1".parse::<PhpVersion>(), Ok(PhpVersion::new(8, 1, 0)));
    assert_eq!("8.1.3".parse::<PhpVersion>(), Ok(PhpVersion::new(8, 1, 3)));
    assert_eq!(
        "8.1.255".parse::<PhpVersion>(),
        Ok(PhpVersion::new(8, 1, 255))
    );

    let shape = parse_error("8.1.3.4");
    assert_eq!(shape, ParsePhpVersionError::Shape);
    assert_eq!(
        format!("{shape}"),
        "expected 1 to 3 dot-separated version components"
    );
    assert_eq!(
        shape.to_string(),
        "expected 1 to 3 dot-separated version components"
    );
    assert!(std::error::Error::source(&shape).is_none());

    let component = parse_error("8.x");
    assert!(matches!(component, ParsePhpVersionError::Component(_)));
    assert!(format!("{component}").contains("invalid digit"));
    assert!(component.to_string().contains("invalid version component"));
    let source = std::error::Error::source(&component).expect("component source");
    assert!(source.to_string().contains("invalid digit"));

    let patch_component = parse_error("8.1.x");
    assert!(matches!(
        patch_component,
        ParsePhpVersionError::Component(_)
    ));
    assert!(patch_component.to_string().contains("invalid digit"));

    let overflow = parse_error("256");
    assert!(matches!(overflow, ParsePhpVersionError::Component(_)));
    assert!(overflow.to_string().contains("number too large"));

    let patch_overflow = parse_error("8.1.256");
    assert!(matches!(patch_overflow, ParsePhpVersionError::Component(_)));
    assert!(patch_overflow.to_string().contains("number too large"));

    let empty = parse_error("");
    assert!(matches!(empty, ParsePhpVersionError::Component(_)));
    assert!(empty.to_string().contains("cannot parse integer"));

    assert_eq!(shape.clone(), shape);
    assert_eq!(component.clone(), component);
    assert_ne!(shape, component.clone());
    assert_eq!(format!("{shape:?}"), "Shape");
    assert!(format!("{component:?}").starts_with("Component("));
}

#[test]
fn symbol_kind_traits_and_availability_fields_are_observable() {
    let kinds: HashSet<_> = [
        SymbolKind::Function,
        SymbolKind::Constant,
        SymbolKind::Class,
        SymbolKind::Method,
    ]
    .into_iter()
    .collect();
    assert_eq!(kinds.len(), 4);
    assert!(kinds.contains(&SymbolKind::Function));
    assert!(kinds.contains(&SymbolKind::Constant));
    assert!(kinds.contains(&SymbolKind::Class));
    assert!(kinds.contains(&SymbolKind::Method));

    let cloned = cloned_through_trait(&SymbolKind::Function);
    assert_eq!(cloned, SymbolKind::Function);
    assert_ne!(cloned, SymbolKind::Method);
    assert_eq!(format!("{:?}", SymbolKind::Method), "Method");

    let availability = function_availability("strlen").expect("strlen should exist");
    assert_eq!(availability.added, None);
    assert_eq!(availability.deprecated, None);
    assert_eq!(availability.removed, None);
    assert_eq!(availability.replacement, None);
    assert_eq!(availability.extension, "Core");
    assert!(availability.compiler_optimized);
}

#[test]
fn function_public_api_handles_known_unknown_and_edge_names() {
    let strlen = function_availability("strlen").expect("strlen should exist");
    assert_eq!(
        strlen,
        Availability {
            added: None,
            deprecated: None,
            removed: None,
            replacement: None,
            extension: "Core",
            compiler_optimized: true,
        }
    );
    assert_eq!(function_availability("\\strlen"), Some(strlen));
    assert_eq!(function_availability("STRLEN"), Some(strlen));
    assert_eq!(function_availability("definitely_not_a_php_function"), None);

    assert!(is_function("strlen"));
    assert!(is_function("\\STRLEN"));
    assert!(!is_function("definitely_not_a_php_function"));

    let str_contains = function_availability("str_contains").expect("str_contains should exist");
    assert_eq!(str_contains.added, Some(PHP_80));
    assert_eq!(str_contains.extension, "Core");
    assert!(!is_function_available("str_contains", PHP_74));
    assert!(is_function_available("str_contains", PHP_80));
    assert!(!is_function_available(
        "definitely_not_a_php_function",
        PHP_80
    ));

    let mb_str_split = function_availability("mb_str_split").expect("mb_str_split should exist");
    assert_eq!(mb_str_split.added, Some(PHP_74));
    assert_eq!(mb_str_split.extension, "mbstring");

    let create_function =
        function_availability("create_function").expect("create_function should exist");
    assert_eq!(create_function.deprecated, Some(PhpVersion::minor(7, 2)));
    assert_eq!(create_function.removed, Some(PHP_80));
    assert!(is_function_available("create_function", PHP_74));
    assert!(!is_function_available("create_function", PHP_80));
    assert!(is_function_deprecated_at("create_function", PHP_74));

    let utf8_encode = function_availability("utf8_encode").expect("utf8_encode should exist");
    assert_eq!(utf8_encode.deprecated, Some(PHP_82));
    assert_eq!(utf8_encode.replacement, Some("mb_convert_encoding()"));
    assert!(!is_function_deprecated_at("utf8_encode", PHP_81));
    assert!(is_function_deprecated_at("utf8_encode", PHP_82));
    assert!(!is_function_deprecated_at("strlen", PHP_85));
    assert!(!is_function_deprecated_at(
        "definitely_not_a_php_function",
        PHP_85
    ));
}

#[test]
fn function_iterators_are_non_empty_and_filter_by_version() {
    let all_functions: HashSet<_> = functions().map(|(name, _)| name).collect();
    assert!(!all_functions.is_empty());
    assert!(all_functions.contains("strlen"));
    assert!(all_functions.contains("str_contains"));
    assert_eq!(
        *functions()
            .find(|(name, _)| *name == "strlen")
            .expect("strlen should be yielded")
            .1,
        function_availability("strlen").expect("strlen should exist")
    );

    let at_74: HashSet<_> = functions_available_at(PHP_74).collect();
    assert!(at_74.contains("strlen"));
    assert!(at_74.contains("create_function"));
    assert!(at_74.contains("mb_str_split"));
    assert!(!at_74.contains("str_contains"));

    let at_80: HashSet<_> = functions_available_at(PHP_80).collect();
    assert!(at_80.contains("strlen"));
    assert!(at_80.contains("str_contains"));
    assert!(!at_80.contains("create_function"));
}

#[test]
fn constant_public_api_handles_case_sensitivity_and_boundaries() {
    let php_int_max = constant_availability("PHP_INT_MAX").expect("PHP_INT_MAX should exist");
    assert_eq!(
        php_int_max,
        Availability {
            added: None,
            deprecated: None,
            removed: None,
            replacement: None,
            extension: "Core",
            compiler_optimized: false,
        }
    );
    assert_eq!(constant_availability("\\PHP_INT_MAX"), Some(php_int_max));
    assert_eq!(constant_availability("php_int_max"), None);
    assert_eq!(constant_availability("DEFINITELY_NOT_A_PHP_CONSTANT"), None);

    assert!(is_constant("PHP_INT_MAX"));
    assert!(is_constant("\\PHP_INT_MAX"));
    assert!(!is_constant("php_int_max"));
    assert!(!is_constant("DEFINITELY_NOT_A_PHP_CONSTANT"));

    let validate_bool =
        constant_availability("FILTER_VALIDATE_BOOL").expect("FILTER_VALIDATE_BOOL should exist");
    assert_eq!(validate_bool.added, Some(PHP_80));
    assert_eq!(validate_bool.extension, "filter");
    assert!(!is_constant_available("FILTER_VALIDATE_BOOL", PHP_74));
    assert!(is_constant_available("FILTER_VALIDATE_BOOL", PHP_80));
    assert!(!is_constant_available("php_int_max", PHP_80));
    assert!(!is_constant_available(
        "DEFINITELY_NOT_A_PHP_CONSTANT",
        PHP_80
    ));

    let json_throw =
        constant_availability("JSON_THROW_ON_ERROR").expect("JSON_THROW_ON_ERROR should exist");
    assert_eq!(json_throw.added, None);
    assert_eq!(json_throw.extension, "json");

    let e_strict = constant_availability("E_STRICT").expect("E_STRICT should exist");
    assert_eq!(e_strict.deprecated, Some(PHP_84));
    assert!(!is_constant_deprecated_at("E_STRICT", PHP_83));
    assert!(is_constant_deprecated_at("E_STRICT", PHP_84));
    assert!(!is_constant_deprecated_at("PHP_INT_MAX", PHP_85));
    assert!(!is_constant_deprecated_at(
        "DEFINITELY_NOT_A_PHP_CONSTANT",
        PHP_85
    ));

    let host_required = constant_availability("FILTER_FLAG_HOST_REQUIRED")
        .expect("FILTER_FLAG_HOST_REQUIRED should exist");
    assert_eq!(host_required.deprecated, Some(PhpVersion::minor(7, 3)));
    assert_eq!(host_required.removed, Some(PHP_80));
    assert!(is_constant_available("FILTER_FLAG_HOST_REQUIRED", PHP_74));
    assert!(!is_constant_available("FILTER_FLAG_HOST_REQUIRED", PHP_80));
    assert!(is_constant_deprecated_at(
        "FILTER_FLAG_HOST_REQUIRED",
        PHP_80
    ));

    let all_constants: HashSet<_> = constants().map(|(name, _)| name).collect();
    assert!(!all_constants.is_empty());
    assert!(all_constants.contains("PHP_INT_MAX"));
    assert!(all_constants.contains("FILTER_VALIDATE_BOOL"));
    assert_eq!(
        *constants()
            .find(|(name, _)| *name == "PHP_INT_MAX")
            .expect("PHP_INT_MAX should be yielded")
            .1,
        php_int_max
    );

    let constants_74: HashSet<_> = constants_available_at(PHP_74).collect();
    assert!(constants_74.contains("PHP_INT_MAX"));
    assert!(constants_74.contains("FILTER_FLAG_HOST_REQUIRED"));
    assert!(!constants_74.contains("FILTER_VALIDATE_BOOL"));

    let constants_80: HashSet<_> = constants_available_at(PHP_80).collect();
    assert!(constants_80.contains("PHP_INT_MAX"));
    assert!(constants_80.contains("FILTER_VALIDATE_BOOL"));
    assert!(!constants_80.contains("FILTER_FLAG_HOST_REQUIRED"));
}

#[test]
fn class_public_api_handles_case_leading_backslash_and_boundaries() {
    let weak_reference = class_availability("WeakReference").expect("WeakReference should exist");
    assert_eq!(weak_reference.added, Some(PHP_74));
    assert_eq!(weak_reference.extension, "Core");

    let weak_map = class_availability("WeakMap").expect("WeakMap should exist");
    assert_eq!(weak_map.added, Some(PHP_80));

    let fiber = class_availability("Fiber").expect("Fiber should exist");
    assert_eq!(
        fiber,
        Availability {
            added: Some(PHP_81),
            deprecated: None,
            removed: None,
            replacement: None,
            extension: "Core",
            compiler_optimized: false,
        }
    );

    let randomizer = class_availability("Random\\Randomizer").expect("Randomizer should exist");
    assert_eq!(randomizer.added, Some(PHP_82));
    assert_eq!(randomizer.extension, "random");
    assert_eq!(class_availability("\\Random\\Randomizer"), Some(randomizer));
    assert_eq!(class_availability("RANDOM\\RANDOMIZER"), Some(randomizer));
    assert_eq!(class_availability("DefinitelyNotAPhpClass"), None);

    assert!(is_class("WeakReference"));
    assert!(is_class("\\RANDOM\\RANDOMIZER"));
    assert!(!is_class("DefinitelyNotAPhpClass"));

    assert!(!is_class_available("Fiber", PHP_80));
    assert!(is_class_available("Fiber", PHP_81));
    assert!(is_class_available("\\RANDOM\\RANDOMIZER", PHP_82));
    assert!(!is_class_available("DefinitelyNotAPhpClass", PHP_82));

    let dom_configuration =
        class_availability("DOMConfiguration").expect("DOMConfiguration should exist");
    assert_eq!(dom_configuration.added, None);
    assert_eq!(dom_configuration.removed, Some(PHP_80));
    assert_eq!(dom_configuration.extension, "dom");
    assert!(is_class_available("DOMConfiguration", PHP_74));
    assert!(!is_class_available("DOMConfiguration", PHP_80));

    assert!(!is_class_deprecated_at("WeakReference", PHP_85));
    assert!(!is_class_deprecated_at("DefinitelyNotAPhpClass", PHP_85));

    let all_classes: HashSet<_> = classes().map(|(name, _)| name).collect();
    assert!(!all_classes.is_empty());
    assert!(all_classes.contains("weakreference"));
    assert!(all_classes.contains("random\\randomizer"));
    assert_eq!(
        *classes()
            .find(|(name, _)| *name == "fiber")
            .expect("fiber should be yielded")
            .1,
        fiber
    );

    let classes_80: HashSet<_> = classes_available_at(PHP_80).collect();
    assert!(classes_80.contains("weakmap"));
    assert!(!classes_80.contains("fiber"));
    assert!(!classes_80.contains("domconfiguration"));

    let classes_81: HashSet<_> = classes_available_at(PHP_81).collect();
    assert!(classes_81.contains("fiber"));
}

#[test]
fn method_public_api_handles_declared_only_lookup_and_boundaries() {
    let push = method_availability("SplDoublyLinkedList", "push").expect("push should be declared");
    assert_eq!(
        push,
        Availability {
            added: None,
            deprecated: None,
            removed: None,
            replacement: None,
            extension: "SPL",
            compiler_optimized: false,
        }
    );
    assert_eq!(method_availability("SplStack", "push"), None);
    assert_eq!(
        method_availability("Random\\Randomizer", "definitelyNotAMethod"),
        None
    );
    assert_eq!(method_availability("DefinitelyNotAPhpClass", "push"), None);

    assert!(is_method("SplDoublyLinkedList", "push"));
    assert!(!is_method("SplStack", "push"));
    assert!(!is_method("Random\\Randomizer", "definitelyNotAMethod"));
    assert!(!is_method("DefinitelyNotAPhpClass", "push"));

    let next_int =
        method_availability("Random\\Randomizer", "nextInt").expect("nextInt should exist");
    assert_eq!(next_int.added, Some(PHP_82));
    assert_eq!(next_int.extension, "random");
    assert!(!is_method_available(
        "Random\\Randomizer",
        "nextInt",
        PHP_81
    ));
    assert!(is_method_available("Random\\Randomizer", "nextInt", PHP_82));

    let get_float =
        method_availability("\\Random\\Randomizer", "GETFLOAT").expect("getFloat should exist");
    assert_eq!(get_float.added, Some(PHP_83));
    assert_eq!(
        method_availability("random\\randomizer", "getfloat"),
        Some(get_float)
    );
    assert!(!is_method_available(
        "Random\\Randomizer",
        "getFloat",
        PHP_82
    ));
    assert!(is_method_available(
        "\\Random\\Randomizer",
        "GETFLOAT",
        PHP_83
    ));
    assert!(!is_method_available(
        "Random\\Randomizer",
        "definitelyNotAMethod",
        PHP_83
    ));
    assert!(!is_method_available(
        "DefinitelyNotAPhpClass",
        "push",
        PHP_83
    ));

    let get_class =
        method_availability("ReflectionParameter", "getClass").expect("getClass should exist");
    assert_eq!(get_class.deprecated, Some(PHP_80));
    assert_eq!(
        get_class.replacement,
        Some("ReflectionParameter::getType()")
    );
    assert!(!is_method_deprecated_at(
        "ReflectionParameter",
        "getClass",
        PHP_74
    ));
    assert!(is_method_deprecated_at(
        "ReflectionParameter",
        "getClass",
        PHP_80
    ));
    assert!(!is_method_deprecated_at(
        "Random\\Randomizer",
        "nextInt",
        PHP_85
    ));
    assert!(!is_method_deprecated_at(
        "Random\\Randomizer",
        "definitelyNotAMethod",
        PHP_85
    ));
    assert!(!is_method_deprecated_at(
        "DefinitelyNotAPhpClass",
        "push",
        PHP_85
    ));

    let all_methods: HashSet<_> = methods()
        .map(|(class, method, _)| (class, method))
        .collect();
    assert!(!all_methods.is_empty());
    assert!(all_methods.contains(&("spldoublylinkedlist", "push")));
    assert!(all_methods.contains(&("random\\randomizer", "nextint")));
    assert_eq!(
        *methods()
            .find(|(class, method, _)| { *class == "spldoublylinkedlist" && *method == "push" })
            .expect("SplDoublyLinkedList::push should be yielded")
            .2,
        push
    );

    let methods_82: HashSet<_> = methods_available_at(PHP_82).collect();
    assert!(methods_82.contains(&("random\\randomizer", "nextint")));
    assert!(!methods_82.contains(&("random\\randomizer", "getfloat")));

    let methods_83: HashSet<_> = methods_available_at(PHP_83).collect();
    assert!(methods_83.contains(&("random\\randomizer", "getfloat")));
}

#[test]
fn callable_method_public_api_resolves_direct_and_inherited_methods() {
    let set_iterator_mode = method_availability("SplStack", "setIteratorMode")
        .expect("SplStack::setIteratorMode should be declared");
    let direct = callable_method_availability("SplStack", "setIteratorMode")
        .expect("SplStack::setIteratorMode should be callable");
    assert_eq!(
        direct,
        CallableMethod {
            class: "splstack",
            method: "setiteratormode",
            declaring_class: "splstack",
            availability: set_iterator_mode,
        }
    );
    assert_eq!(cloned_through_trait(&direct), direct);
    let copied = direct;
    assert_eq!(copied, direct);
    assert!(format!("{direct:?}").contains("CallableMethod"));

    let push = callable_method_availability("SplStack", "push")
        .expect("SplStack::push should be inherited from SplDoublyLinkedList");
    assert_eq!(push.class, "splstack");
    assert_eq!(push.method, "push");
    assert_eq!(push.declaring_class, "spldoublylinkedlist");
    assert_eq!(push.availability.added, None);
    assert_eq!(push.availability.removed, None);
    assert_eq!(push.availability.extension, "SPL");
    assert_eq!(method_availability("SplStack", "push"), None);
    assert!(is_callable_method("SplStack", "push"));

    let mixed_case = callable_method_availability("\\sPlStAcK", "PUSH")
        .expect("class and method lookup should be case-insensitive");
    assert_eq!(mixed_case, push);

    // DirectoryIterator's direct ancestors are SeekableIterator and SplFileInfo.
    // The first is an interface with no method row in the declared-method table,
    // so this confirms traversal continues and resolves the parent declaration.
    let to_string = callable_method_availability("DirectoryIterator", "__toString")
        .expect("DirectoryIterator::__toString should resolve from SplFileInfo");
    assert_eq!(to_string.class, "directoryiterator");
    assert_eq!(to_string.method, "__tostring");
    assert_eq!(to_string.declaring_class, "splfileinfo");
}

#[test]
fn callable_method_public_api_handles_missing_names_and_diamond_deduplication() {
    assert_eq!(
        callable_method_availability("SplStack", "definitelyNotAMethod"),
        None
    );
    assert_eq!(
        callable_method_availability("DefinitelyNotAPhpClass", "push"),
        None
    );
    assert_eq!(
        callable_method_availability("stdClass", "definitelyNotAMethod"),
        None
    );
    assert!(!is_callable_method("SplStack", "definitelyNotAMethod"));
    assert!(!is_callable_method("DefinitelyNotAPhpClass", "push"));
    assert!(!is_callable_method("stdClass", "definitelyNotAMethod"));

    // IntlPartsIterator reaches Iterator directly and through IntlIterator.
    // A missing method forces the traversal through the duplicate ancestor.
    assert_eq!(
        callable_method_availability("IntlPartsIterator", "definitelyNotAMethod"),
        None
    );
}

#[test]
fn callable_method_public_api_uses_effective_availability_bounds() {
    let get_message = callable_method_availability("ValueError", "getMessage")
        .expect("ValueError::getMessage should resolve from Error");
    assert_eq!(get_message.class, "valueerror");
    assert_eq!(get_message.method, "getmessage");
    assert_eq!(get_message.declaring_class, "error");
    assert_eq!(get_message.availability.added, Some(PHP_80));
    assert_eq!(get_message.availability.removed, None);
    assert!(!is_callable_method_available(
        "ValueError",
        "getMessage",
        PHP_74
    ));
    assert!(is_callable_method_available(
        "ValueError",
        "getMessage",
        PHP_80
    ));

    let get_feature = callable_method_availability("DOMAttr", "getFeature")
        .expect("DOMAttr::getFeature should resolve from DOMNode");
    assert_eq!(get_feature.class, "domattr");
    assert_eq!(get_feature.method, "getfeature");
    assert_eq!(get_feature.declaring_class, "domnode");
    assert_eq!(get_feature.availability.added, None);
    assert_eq!(get_feature.availability.removed, Some(PHP_80));
    assert!(is_callable_method_available(
        "DOMAttr",
        "getFeature",
        PHP_74
    ));
    assert!(!is_callable_method_available(
        "DOMAttr",
        "getFeature",
        PHP_80
    ));
    assert!(!is_callable_method_available(
        "DefinitelyNotAPhpClass",
        "getFeature",
        PHP_74
    ));
}

#[test]
fn callable_method_public_api_uses_method_deprecation_metadata() {
    let get_class = callable_method_availability("ReflectionParameter", "getClass")
        .expect("ReflectionParameter::getClass should be callable");
    assert_eq!(get_class.class, "reflectionparameter");
    assert_eq!(get_class.method, "getclass");
    assert_eq!(get_class.declaring_class, "reflectionparameter");
    assert_eq!(get_class.availability.deprecated, Some(PHP_80));
    assert_eq!(
        get_class.availability.replacement,
        Some("ReflectionParameter::getType()")
    );
    assert!(!is_callable_method_deprecated_at(
        "ReflectionParameter",
        "getClass",
        PHP_74
    ));
    assert!(is_callable_method_deprecated_at(
        "ReflectionParameter",
        "getClass",
        PHP_80
    ));
    assert!(!is_callable_method_deprecated_at(
        "ValueError",
        "getMessage",
        PHP_85
    ));
    assert!(!is_callable_method_deprecated_at(
        "DefinitelyNotAPhpClass",
        "getClass",
        PHP_85
    ));
}

#[test]
fn core_extension_lookup_is_case_sensitive() {
    assert!(is_core_extension("Core"));
    assert!(is_core_extension("standard"));
    assert!(is_core_extension("SPL"));
    assert!(is_core_extension("random"));
    assert!(!is_core_extension("mbstring"));
    assert!(!is_core_extension("core"));
    assert!(!is_core_extension(""));
}
