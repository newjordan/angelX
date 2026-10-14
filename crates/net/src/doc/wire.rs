//! Trees and patches as bytes: a tag a value, integers as varints, floats
//! as their bits, and field and variant names sent once per connection then
//! by number (both ends keep the same table, in the order names were met).

pub use crate::codec::{Bytes, Malformed, put_varint};
use crate::codec::{unzigzag, zigzag};
use super::delta::Patch;
use super::value::{Name, Value};
use std::borrow::Cow;
use std::collections::HashMap;

const UNIT: u8 = 0;
const FALSE: u8 = 1;
const TRUE: u8 = 2;
const INT: u8 = 3;
const UINT: u8 = 4;
const F32: u8 = 5;
const F64: u8 = 6;
const CHAR: u8 = 7;
const STR: u8 = 8;
const BYTES: u8 = 9;
const NONE: u8 = 10;
const SOME: u8 = 11;
const SEQ: u8 = 12;
const MAP: u8 = 13;
const STRUCT: u8 = 14;
const VARIANT: u8 = 15;

const P_SET: u8 = 0;
const P_STRUCT: u8 = 1;
const P_SEQ: u8 = 2;
const P_KEYED: u8 = 3;
const P_MAP: u8 = 4;
const P_SOME: u8 = 5;
const P_VARIANT: u8 = 6;

/// Nothing decoded is allowed to claim more than this many items.
const MAX_ITEMS: u64 = 1 << 20;
/// Nor nest deeper than this.
const MAX_DEPTH: u32 = 64;


/// The writing end's half of a connection's name table.
#[derive(Default)]
pub struct Writer {
    names: HashMap<String, u64>,
}

impl Writer {
    fn name(&mut self, out: &mut Vec<u8>, name: &str) {
        match self.names.get(name) {
            Some(&id) => put_varint(out, id + 1),
            None => {
                out.push(0);
                put_varint(out, name.len() as u64);
                out.extend_from_slice(name.as_bytes());
                let id = self.names.len() as u64;
                self.names.insert(name.to_string(), id);
            }
        }
    }

    pub fn value(&mut self, out: &mut Vec<u8>, value: &Value) {
        match value {
            Value::Unit => out.push(UNIT),
            Value::Bool(false) => out.push(FALSE),
            Value::Bool(true) => out.push(TRUE),
            Value::I(n) => {
                out.push(INT);
                put_varint(out, zigzag(*n));
            }
            Value::U(n) => {
                out.push(UINT);
                put_varint(out, *n);
            }
            Value::F32(f) => {
                out.push(F32);
                out.extend_from_slice(&f.to_bits().to_le_bytes());
            }
            Value::F64(f) => {
                out.push(F64);
                out.extend_from_slice(&f.to_bits().to_le_bytes());
            }
            Value::Char(c) => {
                out.push(CHAR);
                put_varint(out, u64::from(*c));
            }
            Value::Str(s) => {
                out.push(STR);
                put_varint(out, s.len() as u64);
                out.extend_from_slice(s.as_bytes());
            }
            Value::Bytes(b) => {
                out.push(BYTES);
                put_varint(out, b.len() as u64);
                out.extend_from_slice(b);
            }
            Value::None => out.push(NONE),
            Value::Some(v) => {
                out.push(SOME);
                self.value(out, v);
            }
            Value::Seq(items) => {
                out.push(SEQ);
                put_varint(out, items.len() as u64);
                for item in items {
                    self.value(out, item);
                }
            }
            Value::Map(entries) => {
                out.push(MAP);
                put_varint(out, entries.len() as u64);
                for (k, v) in entries {
                    self.value(out, k);
                    self.value(out, v);
                }
            }
            Value::Struct(fields) => {
                out.push(STRUCT);
                put_varint(out, fields.len() as u64);
                for (name, v) in fields {
                    self.name(out, name);
                    self.value(out, v);
                }
            }
            Value::Variant(name, payload) => {
                out.push(VARIANT);
                self.name(out, name);
                self.value(out, payload);
            }
        }
    }

    pub fn patch(&mut self, out: &mut Vec<u8>, patch: &Patch) {
        match patch {
            Patch::Set(v) => {
                out.push(P_SET);
                self.value(out, v);
            }
            Patch::Struct { set, gone } => {
                out.push(P_STRUCT);
                put_varint(out, set.len() as u64);
                for (name, p) in set {
                    self.name(out, name);
                    self.patch(out, p);
                }
                put_varint(out, gone.len() as u64);
                for name in gone {
                    self.name(out, name);
                }
            }
            Patch::Seq { len, items } => {
                out.push(P_SEQ);
                put_varint(out, u64::from(*len));
                put_varint(out, items.len() as u64);
                for (i, p) in items {
                    put_varint(out, u64::from(*i));
                    self.patch(out, p);
                }
            }
            Patch::Keyed { order, items } => {
                out.push(P_KEYED);
                put_varint(out, order.len() as u64);
                // Ids rise more often than not: send each as the step from
                // the one before.
                let mut last = 0u64;
                for &id in order {
                    put_varint(out, zigzag(id.wrapping_sub(last) as i64));
                    last = id;
                }
                put_varint(out, items.len() as u64);
                for (id, p) in items {
                    put_varint(out, *id);
                    self.patch(out, p);
                }
            }
            Patch::Map { gone, items } => {
                out.push(P_MAP);
                put_varint(out, gone.len() as u64);
                for k in gone {
                    self.value(out, k);
                }
                put_varint(out, items.len() as u64);
                for (k, p) in items {
                    self.value(out, k);
                    self.patch(out, p);
                }
            }
            Patch::Some(p) => {
                out.push(P_SOME);
                self.patch(out, p);
            }
            Patch::Variant(p) => {
                out.push(P_VARIANT);
                self.patch(out, p);
            }
        }
    }
}

/// The reading end's half of a connection's name table.
#[derive(Default)]
pub struct Reader {
    names: Vec<Name>,
}

/// No list or text may claim more items than there are bytes left, nor more
/// than `MAX_ITEMS`.
fn count(bytes: &mut Bytes) -> Result<usize, Malformed> {
    let n = bytes.varint()?;
    if n > MAX_ITEMS || n as usize > bytes.rest.len() {
        return Err(Malformed);
    }
    Ok(n as usize)
}

impl Reader {
    fn name(&mut self, bytes: &mut Bytes) -> Result<Name, Malformed> {
        match bytes.varint()? {
            0 => {
                let len = count(bytes)?;
                let text = std::str::from_utf8(bytes.take(len)?).map_err(|_| Malformed)?;
                let name: Name = Cow::Owned(text.to_string());
                self.names.push(name.clone());
                Ok(name)
            }
            id => self.names.get(id as usize - 1).cloned().ok_or(Malformed),
        }
    }

    pub fn value(&mut self, bytes: &mut Bytes) -> Result<Value, Malformed> {
        self.value_at(bytes, 0)
    }

    fn value_at(&mut self, bytes: &mut Bytes, depth: u32) -> Result<Value, Malformed> {
        if depth > MAX_DEPTH {
            return Err(Malformed);
        }
        let d = depth + 1;
        Ok(match bytes.byte()? {
            UNIT => Value::Unit,
            FALSE => Value::Bool(false),
            TRUE => Value::Bool(true),
            INT => Value::I(unzigzag(bytes.varint()?)),
            UINT => Value::U(bytes.varint()?),
            F32 => Value::F32(f32::from_bits(u32::from_le_bytes(
                bytes.take(4)?.try_into().map_err(|_| Malformed)?,
            ))),
            F64 => Value::F64(f64::from_bits(u64::from_le_bytes(
                bytes.take(8)?.try_into().map_err(|_| Malformed)?,
            ))),
            CHAR => Value::Char(
                char::from_u32(u32::try_from(bytes.varint()?).map_err(|_| Malformed)?)
                    .ok_or(Malformed)?,
            ),
            STR => {
                let len = count(bytes)?;
                let text = std::str::from_utf8(bytes.take(len)?).map_err(|_| Malformed)?;
                Value::Str(text.into())
            }
            BYTES => {
                let len = count(bytes)?;
                Value::Bytes(bytes.take(len)?.into())
            }
            NONE => Value::None,
            SOME => Value::Some(Box::new(self.value_at(bytes, d)?)),
            SEQ => {
                let n = count(bytes)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.value_at(bytes, d)?);
                }
                Value::Seq(items)
            }
            MAP => {
                let n = count(bytes)?;
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    let k = self.value_at(bytes, d)?;
                    entries.push((k, self.value_at(bytes, d)?));
                }
                Value::Map(entries)
            }
            STRUCT => {
                let n = count(bytes)?;
                let mut fields = Vec::with_capacity(n);
                for _ in 0..n {
                    let name = self.name(bytes)?;
                    fields.push((name, self.value_at(bytes, d)?));
                }
                Value::Struct(fields)
            }
            VARIANT => {
                let name = self.name(bytes)?;
                Value::Variant(name, Box::new(self.value_at(bytes, d)?))
            }
            _ => return Err(Malformed),
        })
    }

    pub fn patch(&mut self, bytes: &mut Bytes) -> Result<Patch, Malformed> {
        self.patch_at(bytes, 0)
    }

    fn patch_at(&mut self, bytes: &mut Bytes, depth: u32) -> Result<Patch, Malformed> {
        if depth > MAX_DEPTH {
            return Err(Malformed);
        }
        let d = depth + 1;
        Ok(match bytes.byte()? {
            P_SET => Patch::Set(self.value_at(bytes, d)?),
            P_STRUCT => {
                let n = count(bytes)?;
                let mut set = Vec::with_capacity(n);
                for _ in 0..n {
                    let name = self.name(bytes)?;
                    set.push((name, self.patch_at(bytes, d)?));
                }
                let n = count(bytes)?;
                let mut gone = Vec::with_capacity(n);
                for _ in 0..n {
                    gone.push(self.name(bytes)?);
                }
                Patch::Struct { set, gone }
            }
            P_SEQ => {
                let len = u32::try_from(bytes.varint()?).map_err(|_| Malformed)?;
                if u64::from(len) > MAX_ITEMS {
                    return Err(Malformed);
                }
                let n = count(bytes)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    let i = u32::try_from(bytes.varint()?).map_err(|_| Malformed)?;
                    items.push((i, self.patch_at(bytes, d)?));
                }
                Patch::Seq { len, items }
            }
            P_KEYED => {
                let n = count(bytes)?;
                let mut order = Vec::with_capacity(n);
                let mut last = 0u64;
                for _ in 0..n {
                    last = last.wrapping_add(unzigzag(bytes.varint()?) as u64);
                    order.push(last);
                }
                let n = count(bytes)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    let id = bytes.varint()?;
                    items.push((id, self.patch_at(bytes, d)?));
                }
                Patch::Keyed { order, items }
            }
            P_MAP => {
                let n = count(bytes)?;
                let mut gone = Vec::with_capacity(n);
                for _ in 0..n {
                    gone.push(self.value_at(bytes, d)?);
                }
                let n = count(bytes)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    let k = self.value_at(bytes, d)?;
                    items.push((k, self.patch_at(bytes, d)?));
                }
                Patch::Map { gone, items }
            }
            P_SOME => Patch::Some(Box::new(self.patch_at(bytes, d)?)),
            P_VARIANT => Patch::Variant(Box::new(self.patch_at(bytes, d)?)),
            _ => return Err(Malformed),
        })
    }
}
