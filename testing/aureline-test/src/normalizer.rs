//! Adapts a compiler-stage result into a logical S-expression in three generic
//! stages: Serde traversal, arena-reference resolution, then S-expression
//! rendering. Syntax-node dispatch does not belong here; logical constructor
//! names live with the serializable stage types so adding a node cannot create
//! a second AST in this test crate.

use std::{collections::HashMap, fmt};

use serde::{Serialize, ser};

use crate::sexpr::SExpr;

pub(crate) fn normalize(value: &impl Serialize) -> Result<SExpr, String> {
    let value = value
        .serialize(ValueSerializer)
        .map_err(|error| error.to_string())?;
    Resolver::new(&value)?.resolve_root(&value)
}

#[derive(Debug)]
enum Value {
    Unit,
    Atom(String),
    Option(Option<Box<Self>>),
    Sequence(Vec<Self>),
    Map(Vec<(Self, Self)>),
    Record {
        name: String,
        fields: Vec<(String, Self)>,
    },
    Variant {
        name: String,
        fields: Vec<(Option<String>, Self)>,
    },
}

struct Resolver<'value> {
    arenas: HashMap<&'value str, &'value [Value]>,
    resolving: Vec<(&'value str, usize)>,
}

impl<'value> Resolver<'value> {
    fn new(value: &'value Value) -> Result<Self, String> {
        let mut resolver = Self {
            arenas: HashMap::new(),
            resolving: Vec::new(),
        };
        resolver.collect_arenas(value)?;
        Ok(resolver)
    }

    fn collect_arenas(&mut self, value: &'value Value) -> Result<(), String> {
        match value {
            Value::Record { name, fields } if name == "$Arena" => {
                let kind = record_atom(fields, "kind")?;
                let values = record_sequence(fields, "values")?;
                if self.arenas.insert(kind, values).is_some() {
                    return Err(format!("duplicate arena kind `{kind}`"));
                }
                for value in values {
                    self.collect_arenas(value)?;
                }
            }
            Value::Option(Some(value)) => self.collect_arenas(value)?,
            Value::Option(None) | Value::Unit | Value::Atom(_) => {}
            Value::Sequence(values) => {
                for value in values {
                    self.collect_arenas(value)?;
                }
            }
            Value::Map(entries) => {
                for (key, value) in entries {
                    self.collect_arenas(key)?;
                    self.collect_arenas(value)?;
                }
            }
            Value::Record { fields, .. } => {
                for (_, value) in fields {
                    self.collect_arenas(value)?;
                }
            }
            Value::Variant { fields, .. } => {
                for (_, value) in fields {
                    self.collect_arenas(value)?;
                }
            }
        }
        Ok(())
    }

    fn resolve_root(&mut self, value: &'value Value) -> Result<SExpr, String> {
        let expressions = self.fragments(value)?;
        match expressions.as_slice() {
            [expression] => Ok(expression.clone()),
            [] => Err("normalization produced no logical value".to_owned()),
            _ => Err("normalization produced more than one logical root".to_owned()),
        }
    }

    fn fragments(&mut self, value: &'value Value) -> Result<Vec<SExpr>, String> {
        match value {
            Value::Unit | Value::Option(None) => Ok(Vec::new()),
            Value::Atom(atom) => Ok(vec![SExpr::Atom(atom.clone())]),
            Value::Option(Some(value)) => self.fragments(value),
            Value::Sequence(values) => self.sequence_fragments(values),
            Value::Map(entries) => {
                let mut items = vec![SExpr::Atom("Map".to_owned())];
                for (key, value) in entries {
                    let mut pair = self.fragments(key)?;
                    pair.extend(self.fragments(value)?);
                    items.push(SExpr::List(pair));
                }
                Ok(vec![SExpr::List(items)])
            }
            Value::Record { name, fields } if name == "$Arena" => Ok(Vec::new()),
            Value::Record { name, fields } if name == "$Ref" => self.resolve_reference(fields),
            Value::Record { fields, .. } if field(fields, "$root").is_some() => {
                self.fragments(field(fields, "$root").expect("root field exists"))
            }
            Value::Record { name, fields } => {
                let mut items = vec![SExpr::Atom(name.clone())];
                for (_, field) in fields {
                    items.extend(self.fragments(field)?);
                }
                Ok(vec![SExpr::List(items)])
            }
            Value::Variant { name, fields } if fields.is_empty() => {
                Ok(vec![SExpr::Atom(name.clone())])
            }
            Value::Variant { name, fields } => {
                let mut items = vec![SExpr::Atom(name.clone())];
                for (_, field) in fields {
                    items.extend(self.variant_field_fragments(field)?);
                }
                Ok(vec![SExpr::List(items)])
            }
        }
    }

    fn sequence_fragments(&mut self, values: &'value [Value]) -> Result<Vec<SExpr>, String> {
        let mut expressions = Vec::new();
        for value in values {
            expressions.extend(self.fragments(value)?);
        }
        Ok(expressions)
    }

    fn variant_field_fragments(&mut self, value: &'value Value) -> Result<Vec<SExpr>, String> {
        match value {
            Value::Record { name, fields } if !name.starts_with('$') => {
                let mut expressions = Vec::new();
                for (_, field) in fields {
                    expressions.extend(self.fragments(field)?);
                }
                Ok(expressions)
            }
            _ => self.fragments(value),
        }
    }

    fn resolve_reference(
        &mut self,
        fields: &'value [(String, Value)],
    ) -> Result<Vec<SExpr>, String> {
        let kind = record_atom(fields, "kind")?;
        let index = record_atom(fields, "index")?
            .parse::<usize>()
            .map_err(|error| format!("invalid {kind} arena index: {error}"))?;
        let key = (kind, index);

        if self.resolving.contains(&key) {
            return Ok(Vec::new());
        }

        let value = self
            .arenas
            .get(kind)
            .and_then(|values| values.get(index))
            .ok_or_else(|| format!("missing {kind} arena value at index {index}"))?;
        self.resolving.push(key);
        let result = self.fragments(value);
        self.resolving.pop();
        result
    }
}

fn field<'value>(fields: &'value [(String, Value)], name: &str) -> Option<&'value Value> {
    fields
        .iter()
        .find_map(|(field_name, value)| (field_name == name).then_some(value))
}

fn record_atom<'value>(
    fields: &'value [(String, Value)],
    name: &str,
) -> Result<&'value str, String> {
    match field(fields, name) {
        Some(Value::Atom(atom)) => Ok(atom),
        _ => Err(format!("missing `{name}` atom")),
    }
}

fn record_sequence<'value>(
    fields: &'value [(String, Value)],
    name: &str,
) -> Result<&'value [Value], String> {
    match field(fields, name) {
        Some(Value::Sequence(values)) => Ok(values),
        _ => Err(format!("missing `{name}` sequence")),
    }
}

#[derive(Clone, Copy)]
struct ValueSerializer;

#[derive(Debug)]
struct SerializationError(String);

impl fmt::Display for SerializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SerializationError {}

impl ser::Error for SerializationError {
    fn custom<T>(message: T) -> Self
    where
        T: fmt::Display,
    {
        Self(message.to_string())
    }
}

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = SerializationError;
    type SerializeSeq = SequenceSerializer;
    type SerializeTuple = SequenceSerializer;
    type SerializeTupleStruct = TupleStructSerializer;
    type SerializeTupleVariant = VariantSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = RecordSerializer;
    type SerializeStructVariant = VariantSerializer;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_string()))
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_string()))
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_string()))
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_string()))
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_string()))
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_string()))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Atom(value.to_owned()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Sequence(
            value
                .iter()
                .map(|byte| Value::Atom(byte.to_string()))
                .collect(),
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Option(None))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Ok(Value::Option(Some(Box::new(value.serialize(self)?))))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Unit)
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Record {
            name: name.to_owned(),
            fields: Vec::new(),
        })
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Variant {
            name: variant.to_owned(),
            fields: Vec::new(),
        })
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Ok(Value::Record {
            name: name.to_owned(),
            fields: vec![(String::new(), value.serialize(self)?)],
        })
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Ok(Value::Variant {
            name: variant.to_owned(),
            fields: vec![(None, value.serialize(self)?)],
        })
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SequenceSerializer::new(length))
    }

    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(SequenceSerializer::new(Some(length)))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(TupleStructSerializer {
            name: name.to_owned(),
            values: Vec::with_capacity(length),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(VariantSerializer {
            name: variant.to_owned(),
            fields: Vec::with_capacity(length),
        })
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer {
            entries: Vec::with_capacity(length.unwrap_or(0)),
            pending_key: None,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(RecordSerializer {
            name: name.to_owned(),
            fields: Vec::with_capacity(length),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(VariantSerializer {
            name: variant.to_owned(),
            fields: Vec::with_capacity(length),
        })
    }
}

struct SequenceSerializer {
    values: Vec<Value>,
}

impl SequenceSerializer {
    fn new(length: Option<usize>) -> Self {
        Self {
            values: Vec::with_capacity(length.unwrap_or(0)),
        }
    }
}

impl ser::SerializeSeq for SequenceSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.values.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Sequence(self.values))
    }
}

impl ser::SerializeTuple for SequenceSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

struct TupleStructSerializer {
    name: String,
    values: Vec<Value>,
}

impl ser::SerializeTupleStruct for TupleStructSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.values.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Record {
            name: self.name,
            fields: self
                .values
                .into_iter()
                .map(|value| (String::new(), value))
                .collect(),
        })
    }
}

struct VariantSerializer {
    name: String,
    fields: Vec<(Option<String>, Value)>,
}

impl ser::SerializeTupleVariant for VariantSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.fields.push((None, value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Variant {
            name: self.name,
            fields: self.fields,
        })
    }
}

impl ser::SerializeStructVariant for VariantSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.fields
            .push((Some(key.to_owned()), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Variant {
            name: self.name,
            fields: self.fields,
        })
    }
}

struct MapSerializer {
    entries: Vec<(Value, Value)>,
    pending_key: Option<Value>,
}

impl ser::SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if self.pending_key.is_some() {
            return Err(ser::Error::custom("map key serialized without a value"));
        }
        self.pending_key = Some(key.serialize(ValueSerializer)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let key = self
            .pending_key
            .take()
            .ok_or_else(|| ser::Error::custom("map value serialized without a key"))?;
        self.entries.push((key, value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.pending_key.is_some() {
            return Err(ser::Error::custom("map ended with a key but no value"));
        }
        Ok(Value::Map(self.entries))
    }
}

struct RecordSerializer {
    name: String,
    fields: Vec<(String, Value)>,
}

impl ser::SerializeStruct for RecordSerializer {
    type Ok = Value;
    type Error = SerializationError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.fields
            .push((key.to_owned(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Record {
            name: self.name,
            fields: self.fields,
        })
    }
}
