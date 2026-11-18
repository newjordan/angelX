//! Any serde value as a tree, and back.
//!
//! The run serializes into a `Value` tree with its field and variant names
//! kept, so two ticks can be compared field by field, and a tree reads back
//! into the run through the run's own `Deserialize`. Nothing here knows the
//! run's shape: a field added to `Run` (or anything in it) is in the tree
//! the moment it is serialized.

use serde::de::{self, IntoDeserializer, Visitor};
use serde::ser::{self, Serialize};
use std::borrow::Cow;
use std::fmt;

pub type Name = Cow<'static, str>;

#[derive(Clone, Debug)]
pub enum Value {
    Unit,
    Bool(bool),
    I(i64),
    U(u64),
    F32(f32),
    F64(f64),
    Char(char),
    Str(Box<str>),
    Bytes(std::sync::Arc<[u8]>),
    None,
    Some(Box<Value>),
    Seq(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Struct(Vec<(Name, Value)>),
    Variant(Name, Box<Value>),
    Shared(std::sync::Arc<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        use Value::*;
        if let (Shared(a), Shared(b)) = (self, other)
            && std::sync::Arc::ptr_eq(a, b)
        {
            return true;
        }
        match (self.get(), other.get()) {
            (Unit, Unit) | (None, None) => true,
            (Bool(a), Bool(b)) => a == b,
            (I(a), I(b)) => a == b,
            (U(a), U(b)) => a == b,
            (F32(a), F32(b)) => a.to_bits() == b.to_bits(),
            (F64(a), F64(b)) => a.to_bits() == b.to_bits(),
            (Char(a), Char(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Bytes(a), Bytes(b)) => a == b,
            (Some(a), Some(b)) => a == b,
            (Seq(a), Seq(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            (Struct(a), Struct(b)) => a == b,
            (Variant(n, a), Variant(m, b)) => n == m && a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn shared(value: Value) -> Value {
        match value {
            shared @ Value::Shared(_) => shared,
            value => Value::Shared(std::sync::Arc::new(value)),
        }
    }

    pub fn get(&self) -> &Value {
        let mut value = self;
        while let Value::Shared(inner) = value {
            value = inner;
        }
        value
    }

    pub fn get_mut(&mut self) -> &mut Value {
        match self {
            Value::Shared(inner) => std::sync::Arc::make_mut(inner).get_mut(),
            value => value,
        }
    }

    pub fn field(&self, name: &str) -> Option<&Value> {
        match self.get() {
            Value::Struct(fields) => fields.iter().find(|(n, _)| n == name).map(|(_, v)| v.get()),
            _ => Option::None,
        }
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.slot_mut(name).map(Value::get_mut)
    }

    pub fn slot_mut(&mut self, name: &str) -> Option<&mut Value> {
        match self.get_mut() {
            Value::Struct(fields) => fields
                .iter_mut()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v),
            _ => Option::None,
        }
    }

    pub fn as_id(&self) -> Option<u64> {
        match *self.get() {
            Value::U(n) => Some(n),
            Value::I(n) if n >= 0 => Some(n as u64),
            _ => Option::None,
        }
    }
}

#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, Error> {
    value.serialize(Builder)
}

pub fn from_value<T: de::DeserializeOwned>(value: &Value) -> Result<T, Error> {
    T::deserialize(Reader(value))
}

pub fn from_value_over<T: de::DeserializeOwned>(value: &Value, over: &[(&str, &Value)]) -> Result<T, Error> {
    match value.get() {
        Value::Struct(fields) => T::deserialize(Over { fields, over }),
        _ => from_value(value),
    }
}

struct Over<'a> {
    fields: &'a [(Name, Value)],
    over: &'a [(&'a str, &'a Value)],
}

impl<'de> de::Deserializer<'de> for Over<'_> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(StructReader { fields: self.fields.iter(), value: Option::None, over: self.over })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

struct Builder;

pub struct SeqBuilder(Vec<Value>);
pub struct VariantSeqBuilder(&'static str, Vec<Value>);
pub struct MapBuilder(Vec<(Value, Value)>, Option<Value>);
pub struct StructBuilder(Vec<(Name, Value)>);
pub struct VariantStructBuilder(&'static str, Vec<(Name, Value)>);

impl ser::Serializer for Builder {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqBuilder;
    type SerializeTuple = SeqBuilder;
    type SerializeTupleStruct = SeqBuilder;
    type SerializeTupleVariant = VariantSeqBuilder;
    type SerializeMap = MapBuilder;
    type SerializeStruct = StructBuilder;
    type SerializeStructVariant = VariantStructBuilder;

    fn serialize_bool(self, v: bool) -> Result<Value, Error> {
        Ok(Value::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(Value::I(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(Value::I(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(Value::I(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(Value::I(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(Value::U(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(Value::U(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(Value::U(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(Value::U(v))
    }
    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(Value::F32(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(Value::F64(v))
    }
    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(Value::Char(v))
    }
    fn serialize_str(self, v: &str) -> Result<Value, Error> {
        Ok(Value::Str(v.into()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Value, Error> {
        Ok(Value::Bytes(v.into()))
    }
    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Value::None)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value, Error> {
        Ok(Value::Some(Box::new(value.serialize(Builder)?)))
    }
    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Value::Unit)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Value::Unit)
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Value::Variant(Cow::Borrowed(variant), Box::new(Value::Unit)))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        value.serialize(Builder)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        Ok(Value::Variant(
            Cow::Borrowed(variant),
            Box::new(value.serialize(Builder)?),
        ))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<SeqBuilder, Error> {
        Ok(SeqBuilder(Vec::with_capacity(len.unwrap_or(0))))
    }
    fn serialize_tuple(self, len: usize) -> Result<SeqBuilder, Error> {
        Ok(SeqBuilder(Vec::with_capacity(len)))
    }
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SeqBuilder, Error> {
        Ok(SeqBuilder(Vec::with_capacity(len)))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<VariantSeqBuilder, Error> {
        Ok(VariantSeqBuilder(variant, Vec::with_capacity(len)))
    }
    fn serialize_map(self, len: Option<usize>) -> Result<MapBuilder, Error> {
        Ok(MapBuilder(Vec::with_capacity(len.unwrap_or(0)), Option::None))
    }
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<StructBuilder, Error> {
        Ok(StructBuilder(Vec::with_capacity(len)))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<VariantStructBuilder, Error> {
        Ok(VariantStructBuilder(variant, Vec::with_capacity(len)))
    }
}

impl ser::SerializeSeq for SeqBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.0.push(value.serialize(Builder)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Seq(self.0))
    }
}

impl ser::SerializeTuple for SeqBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SeqBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for VariantSeqBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.1.push(value.serialize(Builder)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Variant(Cow::Borrowed(self.0), Box::new(Value::Seq(self.1))))
    }
}

impl ser::SerializeMap for MapBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.1 = Some(key.serialize(Builder)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .1
            .take()
            .ok_or_else(|| Error("a map value came before its key".into()))?;
        self.0.push((key, value.serialize(Builder)?));
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(self.0))
    }
}

impl ser::SerializeStruct for StructBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.0.push((Cow::Borrowed(key), value.serialize(Builder)?));
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Struct(self.0))
    }
}

impl ser::SerializeStructVariant for VariantStructBuilder {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.1.push((Cow::Borrowed(key), value.serialize(Builder)?));
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Variant(Cow::Borrowed(self.0), Box::new(Value::Struct(self.1))))
    }
}

#[derive(Clone, Copy)]
struct Reader<'a>(&'a Value);

impl<'de> de::Deserializer<'de> for Reader<'_> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::Unit => visitor.visit_unit(),
            Value::Bool(v) => visitor.visit_bool(*v),
            Value::I(v) => visitor.visit_i64(*v),
            Value::U(v) => visitor.visit_u64(*v),
            Value::F32(v) => visitor.visit_f32(*v),
            Value::F64(v) => visitor.visit_f64(*v),
            Value::Char(v) => visitor.visit_char(*v),
            Value::Str(v) => visitor.visit_str(v),
            Value::Bytes(v) => visitor.visit_bytes(v),
            Value::None => visitor.visit_none(),
            Value::Some(v) => visitor.visit_some(Reader(v)),
            Value::Seq(items) => visitor.visit_seq(SeqReader(items.iter())),
            Value::Map(entries) => visitor.visit_map(MapReader {
                entries: entries.iter(),
                value: Option::None,
            }),
            Value::Struct(fields) => visitor.visit_map(StructReader {
                fields: fields.iter(),
                value: Option::None,
                over: &[],
            }),
            Value::Variant(name, payload) => match payload.get() {
                Value::Unit => visitor.visit_str(name),
                _ => visitor.visit_map(VariantMap {
                    name: Some(name),
                    payload,
                }),
            },
            Value::Shared(inner) => Reader(inner).deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::None | Value::Unit => visitor.visit_none(),
            Value::Some(v) => visitor.visit_some(Reader(v)),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::Variant(name, payload) => visitor.visit_enum(EnumReader { name, payload }),
            Value::Str(name) => visitor.visit_enum(name.as_ref().into_deserializer()),
            other => Err(Error(format!("expected an enum, found {other:?}"))),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::Struct(fields) => visitor.visit_map(StructReader {
                fields: fields.iter(),
                value: Option::None,
                over: &[],
            }),
            other => Err(Error(format!("expected a struct, found {other:?}"))),
        }
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::Map(entries) => visitor.visit_map(MapReader {
                entries: entries.iter(),
                value: Option::None,
            }),
            other => Err(Error(format!("expected a map, found {other:?}"))),
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::Seq(items) => visitor.visit_seq(SeqReader(items.iter())),
            other => Err(Error(format!("expected a list, found {other:?}"))),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf identifier
    }
}

struct SeqReader<'a>(std::slice::Iter<'a, Value>);

impl<'de> de::SeqAccess<'de> for SeqReader<'_> {
    type Error = Error;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        self.0.next().map(|v| seed.deserialize(Reader(v))).transpose()
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.0.len())
    }
}

struct MapReader<'a> {
    entries: std::slice::Iter<'a, (Value, Value)>,
    value: Option<&'a Value>,
}

impl<'de> de::MapAccess<'de> for MapReader<'_> {
    type Error = Error;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.entries.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(MapKey(key)).map(Some)
            }
            Option::None => Ok(Option::None),
        }
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let value = self
            .value
            .take()
            .ok_or_else(|| Error("a map value was read before its key".into()))?;
        seed.deserialize(Reader(value))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

struct MapKey<'a>(&'a Value);

impl<'de> de::Deserializer<'de> for MapKey<'_> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        Reader(self.0).deserialize_any(visitor)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        Reader(self.0).deserialize_enum(name, variants, visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        Reader(self.0).deserialize_newtype_struct(name, visitor)
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        Reader(self.0).deserialize_option(visitor)
    }
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0.get() {
            Value::U(n) => visitor.visit_string(n.to_string()),
            Value::I(n) => visitor.visit_string(n.to_string()),
            other => Reader(other).deserialize_any(visitor),
        }
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf seq tuple tuple_struct map struct unit unit_struct ignored_any
    }
}

struct StructReader<'a> {
    fields: std::slice::Iter<'a, (Name, Value)>,
    value: Option<&'a Value>,
    over: &'a [(&'a str, &'a Value)],
}

impl<'de> de::MapAccess<'de> for StructReader<'_> {
    type Error = Error;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.fields.next() {
            Some((name, value)) => {
                let over = self.over.iter().find(|(n, _)| n == name).map(|&(_, v)| v);
                self.value = Some(over.unwrap_or(value));
                seed.deserialize(name.as_ref().into_deserializer()).map(Some)
            }
            Option::None => Ok(Option::None),
        }
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let value = self
            .value
            .take()
            .ok_or_else(|| Error("a field was read before its name".into()))?;
        seed.deserialize(Reader(value))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.fields.len())
    }
}

struct VariantMap<'a> {
    name: Option<&'a Name>,
    payload: &'a Value,
}

impl<'de> de::MapAccess<'de> for VariantMap<'_> {
    type Error = Error;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.name.take() {
            Some(name) => seed.deserialize(name.as_ref().into_deserializer()).map(Some),
            Option::None => Ok(Option::None),
        }
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        seed.deserialize(Reader(self.payload))
    }
}

struct EnumReader<'a> {
    name: &'a Name,
    payload: &'a Value,
}

impl<'de, 'a> de::EnumAccess<'de> for EnumReader<'a> {
    type Error = Error;
    type Variant = Reader<'a>;
    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Reader<'a>), Error> {
        let variant = seed.deserialize(self.name.as_ref().into_deserializer())?;
        Ok((variant, Reader(self.payload)))
    }
}

impl<'de> de::VariantAccess<'de> for Reader<'_> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }
    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self)
    }
    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(self, visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_struct(self, "", fields, visitor)
    }
}

pub fn fingerprint<T: Serialize + ?Sized>(value: &T, out: &mut Vec<u8>) -> Result<(), Error> {
    value.serialize(Print(out, None))
}

pub fn fingerprint_except<T: Serialize + ?Sized>(value: &T, out: &mut Vec<u8>, mask: &'static Mask) -> Result<(), Error> {
    value.serialize(Print(out, Some(mask)))
}

#[derive(Debug)]
pub struct Mask {
    pub skip: &'static [&'static str],
    pub inside: &'static [(&'static str, Mask)],
}

pub struct Print<'a>(&'a mut Vec<u8>, Option<&'static Mask>);

impl Print<'_> {
    #[inline]
    fn tag(&mut self, tag: u8) {
        self.0.push(tag);
    }
    #[inline]
    fn len(&mut self, n: usize) {
        self.0.extend_from_slice(&(n as u64).to_le_bytes());
    }
    #[inline]
    fn text(&mut self, text: &str) {
        self.len(text.len());
        self.0.extend_from_slice(text.as_bytes());
    }
    #[inline]
    fn name(&mut self, name: &'static str) {
        self.0.extend_from_slice(&(name.as_ptr() as usize as u64).to_le_bytes());
        self.0.extend_from_slice(&(name.len() as u32).to_le_bytes());
    }
}

impl<'a> ser::Serializer for Print<'a> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = PrintSeq<'a>;
    type SerializeTuple = PrintSeq<'a>;
    type SerializeTupleStruct = PrintSeq<'a>;
    type SerializeTupleVariant = PrintSeq<'a>;
    type SerializeMap = PrintSeq<'a>;
    type SerializeStruct = PrintSeq<'a>;
    type SerializeStructVariant = PrintSeq<'a>;

    #[inline]
    fn serialize_bool(mut self, v: bool) -> Result<(), Error> {
        self.tag(1 + u8::from(v));
        Ok(())
    }
    #[inline]
    fn serialize_i8(mut self, v: i8) -> Result<(), Error> {
        self.tag(19);
        self.0.push(v as u8);
        Ok(())
    }
    #[inline]
    fn serialize_i16(mut self, v: i16) -> Result<(), Error> {
        self.tag(20);
        self.0.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_i32(mut self, v: i32) -> Result<(), Error> {
        self.tag(21);
        self.0.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_i64(mut self, v: i64) -> Result<(), Error> {
        self.tag(3);
        self.0.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_u8(mut self, v: u8) -> Result<(), Error> {
        self.tag(22);
        self.0.push(v);
        Ok(())
    }
    #[inline]
    fn serialize_u16(mut self, v: u16) -> Result<(), Error> {
        self.tag(23);
        self.0.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_u32(mut self, v: u32) -> Result<(), Error> {
        self.tag(24);
        self.0.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_u64(mut self, v: u64) -> Result<(), Error> {
        self.tag(4);
        self.0.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_f32(mut self, v: f32) -> Result<(), Error> {
        self.tag(5);
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_f64(mut self, v: f64) -> Result<(), Error> {
        self.tag(6);
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_char(mut self, v: char) -> Result<(), Error> {
        self.tag(7);
        self.0.extend_from_slice(&u32::from(v).to_le_bytes());
        Ok(())
    }
    #[inline]
    fn serialize_str(mut self, v: &str) -> Result<(), Error> {
        self.tag(8);
        self.text(v);
        Ok(())
    }
    #[inline]
    fn serialize_bytes(mut self, v: &[u8]) -> Result<(), Error> {
        self.tag(9);
        self.len(v.len());
        self.0.extend_from_slice(v);
        Ok(())
    }
    #[inline]
    fn serialize_none(mut self) -> Result<(), Error> {
        self.tag(10);
        Ok(())
    }
    #[inline]
    fn serialize_some<T: Serialize + ?Sized>(mut self, value: &T) -> Result<(), Error> {
        self.tag(11);
        value.serialize(Print(self.0, None))
    }
    #[inline]
    fn serialize_unit(mut self) -> Result<(), Error> {
        self.tag(0);
        Ok(())
    }
    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.serialize_unit()
    }
    #[inline]
    fn serialize_unit_variant(mut self, _name: &'static str, index: u32, _variant: &'static str) -> Result<(), Error> {
        match u8::try_from(index) {
            Ok(index) => {
                self.tag(25);
                self.0.push(index);
            }
            Err(_) => {
                self.tag(26);
                self.0.extend_from_slice(&index.to_le_bytes());
            }
        }
        Ok(())
    }
    #[inline]
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _name: &'static str, value: &T) -> Result<(), Error> {
        value.serialize(self)
    }
    #[inline]
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.tag(15);
        self.name(variant);
        value.serialize(Print(self.0, None))
    }
    #[inline]
    fn serialize_seq(mut self, _len: Option<usize>) -> Result<PrintSeq<'a>, Error> {
        self.tag(12);
        Ok(PrintSeq(self.0, 0, None))
    }
    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<PrintSeq<'a>, Error> {
        self.serialize_seq(Some(len))
    }
    #[inline]
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<PrintSeq<'a>, Error> {
        self.serialize_seq(Some(len))
    }
    #[inline]
    fn serialize_tuple_variant(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<PrintSeq<'a>, Error> {
        self.tag(15);
        self.name(variant);
        self.tag(12);
        Ok(PrintSeq(self.0, 0, None))
    }
    #[inline]
    fn serialize_map(mut self, _len: Option<usize>) -> Result<PrintSeq<'a>, Error> {
        self.tag(13);
        Ok(PrintSeq(self.0, 0, None))
    }
    #[inline]
    fn serialize_struct(mut self, _name: &'static str, _len: usize) -> Result<PrintSeq<'a>, Error> {
        self.tag(14);
        Ok(PrintSeq(self.0, 0, self.1))
    }
    #[inline]
    fn serialize_struct_variant(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<PrintSeq<'a>, Error> {
        self.tag(15);
        self.name(variant);
        self.tag(14);
        Ok(PrintSeq(self.0, 0, None))
    }
}

pub struct PrintSeq<'a>(&'a mut Vec<u8>, usize, Option<&'static Mask>);

impl PrintSeq<'_> {
    fn item<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.1 += 1;
        value.serialize(Print(self.0, None))
    }
    fn field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), Error> {
        let inside = match self.2 {
            Some(mask) if mask.skip.contains(&name) => return Ok(()),
            Some(mask) => mask.inside.iter().find(|(n, _)| *n == name).map(|(_, m)| m),
            None => None,
        };
        Print(self.0, None).tag(18);
        Print(self.0, None).name(name);
        self.1 += 1;
        value.serialize(Print(self.0, inside))
    }
    fn close(self) -> Result<(), Error> {
        let mut print = Print(self.0, None);
        print.tag(16);
        print.len(self.1);
        Ok(())
    }
}

impl ser::SerializeSeq for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value)
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}

impl ser::SerializeTuple for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value)
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}

impl ser::SerializeTupleStruct for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value)
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}

impl ser::SerializeTupleVariant for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value)
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}

impl ser::SerializeMap for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.item(key)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(Print(self.0, None))
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}

impl ser::SerializeStruct for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), Error> {
        self.field(name, value)
    }
    fn skip_field(&mut self, name: &'static str) -> Result<(), Error> {
        Print(self.0, None).tag(17);
        Print(self.0, None).name(name);
        Ok(())
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}

impl ser::SerializeStructVariant for PrintSeq<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), Error> {
        self.field(name, value)
    }
    fn skip_field(&mut self, name: &'static str) -> Result<(), Error> {
        Print(self.0, None).tag(17);
        Print(self.0, None).name(name);
        Ok(())
    }
    fn end(self) -> Result<(), Error> {
        self.close()
    }
}
