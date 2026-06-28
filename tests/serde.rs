#![cfg(feature = "serde")]

use std::fmt;

use php_native_symbols::{
    Availability, CompatibilityIssue, PhpVersion, ResolvedSymbol, SymbolKind, SymbolRef,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
enum Value {
    Bool(bool),
    U8(u8),
    Str(String),
    None,
    Some(Box<Value>),
    Struct {
        name: &'static str,
        fields: Vec<(String, Value)>,
    },
    UnitVariant {
        enum_name: &'static str,
        variant: &'static str,
    },
    NewtypeVariant {
        enum_name: &'static str,
        variant: &'static str,
        value: Box<Value>,
    },
    StructVariant {
        enum_name: &'static str,
        variant: &'static str,
        fields: Vec<(String, Value)>,
    },
    Seq(Vec<Value>),
}

#[derive(Debug)]
struct TestError(String);

impl serde::ser::Error for TestError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TestError {}

struct ValueSerializer;

impl serde::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = TestError;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = serde::ser::Impossible<Value, TestError>;
    type SerializeTupleStruct = serde::ser::Impossible<Value, TestError>;
    type SerializeTupleVariant = serde::ser::Impossible<Value, TestError>;
    type SerializeMap = serde::ser::Impossible<Value, TestError>;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = StructVariantSerializer;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(value))
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("i8 unsupported"))
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("i16 unsupported"))
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("i32 unsupported"))
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("i64 unsupported"))
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::U8(value))
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("u16 unsupported"))
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("u32 unsupported"))
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("u64 unsupported"))
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("f32 unsupported"))
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("f64 unsupported"))
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("char unsupported"))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(value.to_string()))
    }

    fn collect_str<T: ?Sized + fmt::Display>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom(
            "bytes unsupported",
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::None)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Some(Box::new(value.serialize(ValueSerializer)?)))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("unit unsupported"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom(
            "unit struct unsupported",
        ))
    }

    fn serialize_unit_variant(
        self,
        enum_name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::UnitVariant { enum_name, variant })
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        enum_name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::NewtypeVariant {
            enum_name,
            variant,
            value: Box::new(value.serialize(ValueSerializer)?),
        })
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SeqSerializer { values: Vec::new() })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom(
            "tuple unsupported",
        ))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom(
            "tuple struct unsupported",
        ))
    }

    fn serialize_tuple_variant(
        self,
        _enum_name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom(
            "tuple variant unsupported",
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(<TestError as serde::ser::Error>::custom("map unsupported"))
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(StructSerializer {
            name,
            fields: Vec::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        enum_name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(StructVariantSerializer {
            enum_name,
            variant,
            fields: Vec::new(),
        })
    }
}

struct StructSerializer {
    name: &'static str,
    fields: Vec<(String, Value)>,
}

impl serde::ser::SerializeStruct for StructSerializer {
    type Ok = Value;
    type Error = TestError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.fields
            .push((key.to_string(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Struct {
            name: self.name,
            fields: self.fields,
        })
    }
}

struct StructVariantSerializer {
    enum_name: &'static str,
    variant: &'static str,
    fields: Vec<(String, Value)>,
}

impl serde::ser::SerializeStructVariant for StructVariantSerializer {
    type Ok = Value;
    type Error = TestError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.fields
            .push((key.to_string(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::StructVariant {
            enum_name: self.enum_name,
            variant: self.variant,
            fields: self.fields,
        })
    }
}

struct SeqSerializer {
    values: Vec<Value>,
}

impl serde::ser::SerializeSeq for SeqSerializer {
    type Ok = Value;
    type Error = TestError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.values.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Seq(self.values))
    }
}

fn serialize_to_value<T: Serialize>(value: &T) -> Value {
    value
        .serialize(ValueSerializer)
        .expect("test serializer supports this value")
}

fn field<'a>(fields: &'a [(String, Value)], name: &str) -> &'a Value {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .expect("field should be present")
}

#[test]
fn php_version_round_trips_through_serde() {
    let version = PhpVersion::new(8, 4, 12);
    let serialized = serialize_to_value(&version);
    let Value::Struct { name, fields } = &serialized else {
        panic!("PhpVersion should serialize as a struct");
    };
    assert_eq!(*name, "PhpVersion");
    assert_eq!(field(fields, "major"), &Value::U8(8));
    assert_eq!(field(fields, "minor"), &Value::U8(4));
    assert_eq!(field(fields, "patch"), &Value::U8(12));

    let pairs = fields.iter().map(|(key, value)| {
        let Value::U8(value) = value else {
            panic!("PhpVersion field should be u8");
        };
        (key.as_str(), *value)
    });
    let deserializer = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(pairs);
    let round_tripped = PhpVersion::deserialize(deserializer).expect("PhpVersion deserializes");
    assert_eq!(round_tripped, version);
}

#[test]
fn symbol_kind_round_trips_through_serde() {
    let kind = SymbolKind::Method;
    let serialized = serialize_to_value(&kind);
    let Value::UnitVariant { enum_name, variant } = serialized else {
        panic!("SymbolKind should serialize as a unit variant");
    };
    assert_eq!(enum_name, "SymbolKind");
    assert_eq!(variant, "Method");

    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(variant);
    let round_tripped =
        SymbolKind::deserialize(deserializer).expect("SymbolKind deserializes from variant");
    assert_eq!(round_tripped, kind);
}

#[test]
fn availability_serializes_public_fields() {
    let availability = Availability {
        added: Some(PhpVersion::minor(8, 0)),
        deprecated: None,
        removed: Some(PhpVersion::minor(8, 5)),
        replacement: Some("successor()"),
        extension: "Core",
        compiler_optimized: true,
    };

    let serialized = serialize_to_value(&availability);
    let Value::Struct { name, fields } = serialized else {
        panic!("Availability should serialize as a struct");
    };
    assert_eq!(name, "Availability");
    assert!(matches!(field(&fields, "added"), Value::Some(_)));
    assert_eq!(field(&fields, "deprecated"), &Value::None);
    assert!(matches!(field(&fields, "removed"), Value::Some(_)));
    assert_eq!(
        field(&fields, "replacement"),
        &Value::Some(Box::new(Value::Str("successor()".to_string())))
    );
    assert_eq!(field(&fields, "extension"), &Value::Str("Core".to_string()));
    assert_eq!(field(&fields, "compiler_optimized"), &Value::Bool(true));
}

#[test]
fn compatibility_issue_serializes_requested_and_resolved_symbols() {
    let issue = CompatibilityIssue::NotYetAvailable {
        requested: SymbolRef::Function("str_contains"),
        resolved: ResolvedSymbol::Function("str_contains"),
        since: PhpVersion::minor(8, 0),
    };

    let serialized = serialize_to_value(&issue);
    let Value::StructVariant {
        enum_name,
        variant,
        fields,
    } = serialized
    else {
        panic!("CompatibilityIssue should serialize as a struct variant");
    };
    assert_eq!(enum_name, "CompatibilityIssue");
    assert_eq!(variant, "NotYetAvailable");
    assert_eq!(
        field(&fields, "requested"),
        &Value::NewtypeVariant {
            enum_name: "SymbolRef",
            variant: "Function",
            value: Box::new(Value::Str("str_contains".to_string()))
        }
    );
    assert_eq!(
        field(&fields, "resolved"),
        &Value::NewtypeVariant {
            enum_name: "ResolvedSymbol",
            variant: "Function",
            value: Box::new(Value::Str("str_contains".to_string()))
        }
    );
    assert!(matches!(field(&fields, "since"), Value::Struct { .. }));
}
