//! The tick's tree built from the last one (`Publisher`): each field of the
//! run is fingerprinted (`value::fingerprint`, no tree built) and built again
//! only when its fingerprint changed; a field built again keeps every part
//! of the last one that did not change (`intern`). Every part is shared
//! (`Value::Shared`), so each seat's copy of the tree and each client's
//! diff (`angel_net::doc::delta::diff`) pass the unchanged parts by.

use super::value::{self, Value};
use serde::ser::{self, Serialize};

/// How deep into a field that changed its parts are fingerprinted one by
/// one and built again only when they changed: the monsters, each
/// monster's fields, and theirs; below that a part is built whole.
pub const KEPT_DEPTH: u32 = 3;

/// A part of the run as last published: its fingerprint, what it was
/// published as, and its own parts, by place.
#[derive(Default)]
pub struct Node {
    name: &'static str,
    revision: Option<u64>,
    print: Vec<u8>,
    value: Option<Value>,
    parts: Vec<Node>,
}

/// The fields last published, in the run's order.
#[derive(Default)]
pub struct Fields {
    root: Node,
    /// Where the next field was found last time.
    at: usize,
    print: Vec<u8>,
}

impl Fields {
    /// Ready for the tick's fields, in the run's order.
    pub fn start(&mut self) {
        self.at = 0;
    }

    /// Field `name`: what it was published as when its fingerprint (with
    /// `key`, what else its shape depends on) is unchanged; else built
    /// again, its unchanged parts kept (`cached`), and shaped by `shape`.
    /// With no `key` it is built again every tick (it changes every tick:
    /// fingerprinting it whole would be for nothing), and with `ages` it is
    /// a list of things whose `age` is sent as the tick they were born.
    pub fn field(
        &mut self,
        name: &'static str,
        value: &dyn erased::Field,
        key: Option<&[u8]>,
        ages: Option<u64>,
        shape: impl FnOnce(&mut Value),
    ) -> Result<Value, value::Error> {
        let parts = &mut self.root.parts;
        let i = match parts.get(self.at) {
            Some(node) if node.name == name => self.at,
            _ => match parts.iter().position(|node| node.name == name) {
                Some(i) => i,
                None => {
                    parts.push(Node { name, ..Node::default() });
                    parts.len() - 1
                }
            },
        };
        self.at = i + 1;
        let node = &mut parts[i];
        node.revision = None;
        // The shape is the node's own: kept beside what it was built from.
        if let Some(key) = key {
            self.print.clear();
            value.print(&mut self.print)?;
            self.print.extend_from_slice(key);
            if node.print == self.print
                && let Some(kept) = &node.value
            {
                // The bytes match. The room they sit in may be a storm's.
                fit(&mut node.print);
                fit(&mut self.print);
                return Ok(kept.clone());
            }
            std::mem::swap(&mut node.print, &mut self.print);
            fit(&mut node.print);
            fit(&mut self.print);
        }
        // Its parts as last built, before their shape.
        if node.parts.is_empty() {
            node.parts.push(Node::default());
        }
        let mut built = value.cached(&mut node.parts[0], KEPT_DEPTH, ages, &mut self.print)?;
        shape(&mut built);
        let built = Value::shared(built);
        node.value = Some(built.clone());
        Ok(built)
    }

    /// Reuse a field before fingerprinting when its authoritative revision is
    /// unchanged. The caller must cover every value/shape change in `revision`
    /// and replace this cache after restoring or replacing that authority.
    /// Failed construction never publishes the new revision.
    pub fn revision_field(
        &mut self,
        name: &'static str,
        value: &dyn erased::Field,
        revision: u64,
        shape: impl FnOnce(&mut Value),
    ) -> Result<Value, value::Error> {
        if let Some(i) = self.root.parts.iter().position(|node| node.name == name) {
            let node = &self.root.parts[i];
            if node.revision == Some(revision)
                && let Some(kept) = &node.value
            {
                self.at = i + 1;
                return Ok(kept.clone());
            }
        }
        let built = self.field(name, value, None, None, shape)?;
        self.root.parts[self.at - 1].revision = Some(revision);
        Ok(built)
    }

    /// What the cache holds, one line a top-level field: its name, its own
    /// fingerprint, every part's fingerprints, their spare room, and how
    /// many parts.
    #[doc(hidden)]
    pub fn held(&self) -> Vec<(&'static str, usize, usize, usize, usize)> {
        self.root
            .parts
            .iter()
            .map(|node| {
                let (parts, prints, caps) = node.held_parts();
                (node.name, node.print.len(), prints, caps, parts)
            })
            .collect()
    }
}

/// A fingerprint keeps a little spare room for the next tick of the same
/// size, and gives the rest back. A bullet storm's print is far larger
/// than the field that holds it once the storm has passed; the scratch
/// buffer is swapped into whichever field changed last, and that room
/// would stay for the rest of the run.
pub fn fit(buf: &mut Vec<u8>) {
    let len = buf.len();
    let cap = buf.capacity();
    let keep = len.saturating_add(len / 4).max(len.saturating_add(64));
    if cap <= keep || cap <= 256 {
        return;
    }
    if cap.saturating_sub(len) < 1024 && cap < 4096 {
        return;
    }
    buf.shrink_to(keep);
}

impl Node {
    /// Cache an already-built projection through the same shared subtree
    /// interning used by typed fields. Custom erased fields need not expose
    /// their original private/native data to the generic serializer.
    pub fn retain_built(&mut self, value: Value, depth: u32) -> Value {
        let kept = intern(self.value.as_ref(), value, depth);
        self.value = Some(kept.clone());
        kept
    }

    fn held_parts(&self) -> (usize, usize, usize) {
        let mut parts = 1usize;
        let mut prints = self.print.len();
        let mut caps = self.print.capacity();
        for part in &self.parts {
            let (n, p, c) = part.held_parts();
            parts += n;
            prints += p;
            caps += c;
        }
        (parts, prints, caps)
    }
}

/// `value` as `node` last saw it when its fingerprint is unchanged; else
/// built again with its parts `depth` levels down each kept the same way.
fn cached<T: Serialize + ?Sized>(
    node: &mut Node,
    value: &T,
    depth: u32,
    print: &mut Vec<u8>,
) -> Result<Value, value::Error> {
    print.clear();
    value::fingerprint(value, print)?;
    if node.print == *print
        && let Some(kept) = &node.value
    {
        fit(&mut node.print);
        fit(print);
        return Ok(kept.clone());
    }
    std::mem::swap(&mut node.print, print);
    fit(&mut node.print);
    fit(print);
    let built = if depth == 0 {
        node.parts.clear();
        value::to_value(value)?
    } else {
        value.serialize(Parts { parts: &mut node.parts, depth: depth - 1, print, ages: Ages::No })?
    };
    let built = share_container(built);
    node.value = Some(built.clone());
    Ok(built)
}

/// A container shared (`Value::Shared`) so the next tick can keep it.
fn share_container(value: Value) -> Value {
    if container(&value) { Value::shared(value) } else { value }
}

/// `new`, keeping the parts of `old` (what it was last published as) that it
/// has unchanged, `depth` levels down; each part it has new made shareable.
/// `old` itself when nothing changed.
pub fn intern(old: Option<&Value>, new: Value, depth: u32) -> Value {
    let Some(old) = old else {
        return share(new, depth);
    };
    if depth == 0 || !container(&new) {
        return if *old == new { old.clone() } else { share(new, depth) };
    }
    let mut same = true;
    let built = match (old.get(), new) {
        (Value::Struct(was), Value::Struct(fields)) => {
            same &= was.len() == fields.len();
            Value::Struct(
                fields
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, value))| {
                        let before = match was.get(i) {
                            Some((n, v)) if *n == name => Some(v),
                            _ => {
                                same = false;
                                was.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
                            }
                        };
                        let value = keep(before, value, depth - 1, &mut same);
                        (name, value)
                    })
                    .collect(),
            )
        }
        (Value::Seq(was), Value::Seq(items)) => {
            same &= was.len() == items.len();
            let ids = |items: &[Value]| -> Option<Vec<u64>> {
                items.iter().map(|item| item.field("id").and_then(Value::as_id)).collect()
            };
            match (ids(was), ids(&items)) {
                // Things with ids: each kept by its id.
                (Some(was_ids), Some(new_ids)) => {
                    let built = items
                        .into_iter()
                        .zip(&new_ids)
                        .enumerate()
                        .map(|(i, (value, id))| {
                            let before = match was_ids.get(i) {
                                Some(w) if w == id => Some(&was[i]),
                                _ => {
                                    same = false;
                                    was_ids.iter().position(|w| w == id).map(|j| &was[j])
                                }
                            };
                            keep(before, value, depth - 1, &mut same)
                        })
                        .collect();
                    Value::Seq(built)
                }
                _ => Value::Seq(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, value)| keep(was.get(i), value, depth - 1, &mut same))
                        .collect(),
                ),
            }
        }
        (Value::Map(was), Value::Map(entries)) => {
            same &= was.len() == entries.len();
            Value::Map(
                entries
                    .into_iter()
                    .enumerate()
                    .map(|(i, (key, value))| {
                        let before = match was.get(i) {
                            Some((k, v)) if *k == key => Some(v),
                            _ => {
                                same = false;
                                was.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
                            }
                        };
                        let value = keep(before, value, depth - 1, &mut same);
                        (key, value)
                    })
                    .collect(),
            )
        }
        (Value::Some(was), Value::Some(inner)) => {
            Value::Some(Box::new(keep(Some(was), *inner, depth - 1, &mut same)))
        }
        (Value::Variant(name, was), Value::Variant(new_name, inner)) if *name == new_name => {
            Value::Variant(new_name, Box::new(keep(Some(was), *inner, depth - 1, &mut same)))
        }
        (_, new) => return share(new, depth),
    };
    if same { old.clone() } else { Value::shared(built) }
}

/// A part of a container being built again: the old part when unchanged
/// (clearing `same` when not).
fn keep(before: Option<&Value>, value: Value, depth: u32, same: &mut bool) -> Value {
    let kept = intern(before, value, depth);
    let reused = match (before, &kept) {
        (Some(Value::Shared(a)), Value::Shared(b)) => std::sync::Arc::ptr_eq(a, b),
        (Some(was), now) if !container(now) => was == now,
        _ => false,
    };
    *same &= reused;
    kept
}

fn container(value: &Value) -> bool {
    matches!(value.get(), Value::Struct(_) | Value::Seq(_) | Value::Map(_) | Value::Some(_) | Value::Variant(..))
}

/// `value` made shareable, its parts `depth` levels down too, so the next
/// tick can keep them.
fn share(value: Value, depth: u32) -> Value {
    if !container(&value) {
        return value;
    }
    if depth == 0 {
        return Value::shared(value);
    }
    let parts = |v: Value| share(v, depth - 1);
    Value::shared(match value {
        Value::Struct(fields) => Value::Struct(fields.into_iter().map(|(n, v)| (n, parts(v))).collect()),
        Value::Seq(items) => Value::Seq(items.into_iter().map(parts).collect()),
        Value::Map(entries) => Value::Map(entries.into_iter().map(|(k, v)| (k, parts(v))).collect()),
        Value::Some(inner) => Value::Some(Box::new(parts(*inner))),
        Value::Variant(name, inner) => Value::Variant(name, Box::new(parts(*inner))),
        other => other,
    })
}

/// The run's fields one by one (a struct's `Serialize`), each to `each`
/// with its name; anything but a struct is refused.
pub struct EachField<F>(pub F);

pub struct EachFieldStruct<F>(F);

impl<F> ser::SerializeStruct for EachFieldStruct<F>
where
    F: FnMut(&'static str, &dyn erased::Field) -> Result<(), value::Error>,
{
    type Ok = ();
    type Error = value::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), value::Error> {
        (self.0)(name, &erased::Of(value))
    }
    fn end(self) -> Result<(), value::Error> {
        Ok(())
    }
}

/// A field handed on without its type.
pub mod erased {
    use super::*;

    /// What `EachField` hands on: the field, to fingerprint and build.
    pub trait Field {
        fn print(&self, out: &mut Vec<u8>) -> Result<(), value::Error>;
        fn build(&self) -> Result<Value, value::Error>;
        /// Built from its parts as `node` last saw them (`Parts`).
        fn cached(&self, node: &mut Node, depth: u32, ages: Option<u64>, print: &mut Vec<u8>) -> Result<Value, value::Error>;
    }

    pub struct Of<'a, T: ?Sized>(pub &'a T);

    impl<T: Serialize + ?Sized> Field for Of<'_, T> {
        fn print(&self, out: &mut Vec<u8>) -> Result<(), value::Error> {
            value::fingerprint(self.0, out)
        }
        fn build(&self) -> Result<Value, value::Error> {
            value::to_value(self.0)
        }
        fn cached(&self, node: &mut Node, depth: u32, ages: Option<u64>, print: &mut Vec<u8>) -> Result<Value, value::Error> {
            if depth == 0 {
                node.parts.clear();
                return value::to_value(self.0);
            }
            let ages = ages.map_or(Ages::No, Ages::Items);
            self.0.serialize(Parts { parts: &mut node.parts, depth: depth - 1, print, ages })
        }
    }
}

macro_rules! refuse {
    ($($name:ident($($arg:ty),*)),* $(,)?) => {
        $(fn $name(self, $(_: $arg),*) -> Result<Self::Ok, Self::Error> {
            Err(ser::Error::custom("the run is a struct"))
        })*
    };
}

impl<F> ser::Serializer for EachField<F>
where
    F: FnMut(&'static str, &dyn erased::Field) -> Result<(), value::Error>,
{
    type Ok = ();
    type Error = value::Error;
    type SerializeSeq = ser::Impossible<(), value::Error>;
    type SerializeTuple = ser::Impossible<(), value::Error>;
    type SerializeTupleStruct = ser::Impossible<(), value::Error>;
    type SerializeTupleVariant = ser::Impossible<(), value::Error>;
    type SerializeMap = ser::Impossible<(), value::Error>;
    type SerializeStruct = EachFieldStruct<F>;
    type SerializeStructVariant = ser::Impossible<(), value::Error>;

    refuse!(
        serialize_bool(bool), serialize_i8(i8), serialize_i16(i16), serialize_i32(i32), serialize_i64(i64),
        serialize_u8(u8), serialize_u16(u16), serialize_u32(u32), serialize_u64(u64), serialize_f32(f32),
        serialize_f64(f64), serialize_char(char), serialize_str(&str), serialize_bytes(&[u8]),
        serialize_none(), serialize_unit(), serialize_unit_struct(&'static str),
        serialize_unit_variant(&'static str, u32, &'static str),
    );

    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<(), value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, _: &T) -> Result<(), value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, value::Error> {
        Ok(EachFieldStruct(self.0))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, value::Error> {
        Err(ser::Error::custom("the run is a struct"))
    }
}

/// A value built from its parts, each kept from `parts` when unchanged
/// (`cached`, `depth` levels further down).
struct Parts<'a> {
    parts: &'a mut Vec<Node>,
    depth: u32,
    print: &'a mut Vec<u8>,
    ages: Ages,
}

/// Things whose age runs a tick at a time, sent as the tick they were born
/// (`wire::reckon_ages`), on tick `.0`: a list of them, or one of them.
#[derive(Clone, Copy)]
enum Ages {
    No,
    Items(u64),
    Fields(u64),
}

impl<'a> Parts<'a> {
    /// Part `i` (a field `name`, or `""`): kept when it is the same part.
    fn part<T: Serialize + ?Sized>(&mut self, i: usize, name: &'static str, value: &T) -> Result<Value, value::Error> {
        if i == self.parts.len() {
            self.parts.push(Node { name, ..Node::default() });
        } else if self.parts[i].name != name {
            self.parts[i] = Node { name, ..Node::default() };
        }
        cached(&mut self.parts[i], value, self.depth, self.print)
    }
}

/// A container being built from its parts.
struct Built<'a> {
    parts: Parts<'a>,
    items: Vec<Value>,
    fields: Vec<(value::Name, Value)>,
    entries: Vec<(Value, Value)>,
    key: Option<Value>,
    variant: Option<&'static str>,
}

impl<'a> Built<'a> {
    fn new(parts: Parts<'a>, variant: Option<&'static str>) -> Built<'a> {
        Built { parts, items: Vec::new(), fields: Vec::new(), entries: Vec::new(), key: None, variant }
    }
    fn item<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), value::Error> {
        let i = self.items.len();
        let built = match self.parts.ages {
            // Each thing's age changes every tick: it is built again, its
            // other parts kept.
            Ages::Items(tick) => {
                if i == self.parts.parts.len() {
                    self.parts.parts.push(Node::default());
                }
                let node = &mut self.parts.parts[i];
                let depth = self.parts.depth.saturating_sub(1);
                let parts = Parts { parts: &mut node.parts, depth, print: &mut *self.parts.print, ages: Ages::Fields(tick) };
                share_container(value.serialize(parts)?)
            }
            _ => self.parts.part(i, "", value)?,
        };
        self.items.push(built);
        Ok(())
    }
    fn field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), value::Error> {
        let i = self.fields.len();
        if let Ages::Fields(tick) = self.parts.ages
            && name == "age"
        {
            let built = match value::to_value(value)? {
                Value::U(age) => Value::U(tick.wrapping_sub(age) & u64::from(u32::MAX)),
                other => other,
            };
            if i == self.parts.parts.len() {
                self.parts.parts.push(Node { name, ..Node::default() });
            }
            self.fields.push((std::borrow::Cow::Borrowed(name), built));
            return Ok(());
        }
        let built = self.parts.part(i, name, value)?;
        self.fields.push((std::borrow::Cow::Borrowed(name), built));
        Ok(())
    }
    fn wrap(self, inner: Value) -> Value {
        match self.variant {
            Some(name) => Value::Variant(std::borrow::Cow::Borrowed(name), Box::new(inner)),
            None => inner,
        }
    }
    fn seq(mut self) -> Value {
        self.parts.parts.truncate(self.items.len());
        let items = std::mem::take(&mut self.items);
        self.wrap(Value::Seq(items))
    }
    fn structure(mut self) -> Value {
        self.parts.parts.truncate(self.fields.len());
        let fields = std::mem::take(&mut self.fields);
        self.wrap(Value::Struct(fields))
    }
}

impl ser::SerializeSeq for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), value::Error> {
        self.item(value)
    }
    fn end(self) -> Result<Value, value::Error> {
        Ok(self.seq())
    }
}

impl ser::SerializeTuple for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), value::Error> {
        self.item(value)
    }
    fn end(self) -> Result<Value, value::Error> {
        Ok(self.seq())
    }
}

impl ser::SerializeTupleStruct for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), value::Error> {
        self.item(value)
    }
    fn end(self) -> Result<Value, value::Error> {
        Ok(self.seq())
    }
}

impl ser::SerializeTupleVariant for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), value::Error> {
        self.item(value)
    }
    fn end(self) -> Result<Value, value::Error> {
        Ok(self.seq())
    }
}

impl ser::SerializeMap for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), value::Error> {
        self.key = Some(value::to_value(key)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), value::Error> {
        let key = self.key.take().ok_or_else(|| ser::Error::custom("a map's value before its key"))?;
        let i = self.entries.len();
        let built = self.parts.part(i, "", value)?;
        self.entries.push((key, built));
        Ok(())
    }
    fn end(mut self) -> Result<Value, value::Error> {
        self.parts.parts.truncate(self.entries.len());
        let entries = std::mem::take(&mut self.entries);
        Ok(self.wrap(Value::Map(entries)))
    }
}

impl ser::SerializeStruct for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), value::Error> {
        self.field(name, value)
    }
    fn end(self) -> Result<Value, value::Error> {
        Ok(self.structure())
    }
}

impl ser::SerializeStructVariant for Built<'_> {
    type Ok = Value;
    type Error = value::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Result<(), value::Error> {
        self.field(name, value)
    }
    fn end(self) -> Result<Value, value::Error> {
        Ok(self.structure())
    }
}

impl<'a> ser::Serializer for Parts<'a> {
    type Ok = Value;
    type Error = value::Error;
    type SerializeSeq = Built<'a>;
    type SerializeTuple = Built<'a>;
    type SerializeTupleStruct = Built<'a>;
    type SerializeTupleVariant = Built<'a>;
    type SerializeMap = Built<'a>;
    type SerializeStruct = Built<'a>;
    type SerializeStructVariant = Built<'a>;

    fn serialize_bool(self, v: bool) -> Result<Value, value::Error> {
        Ok(Value::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Value, value::Error> {
        Ok(Value::I(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<Value, value::Error> {
        Ok(Value::I(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<Value, value::Error> {
        Ok(Value::I(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<Value, value::Error> {
        Ok(Value::I(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Value, value::Error> {
        Ok(Value::U(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<Value, value::Error> {
        Ok(Value::U(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<Value, value::Error> {
        Ok(Value::U(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<Value, value::Error> {
        Ok(Value::U(v))
    }
    fn serialize_f32(self, v: f32) -> Result<Value, value::Error> {
        Ok(Value::F32(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Value, value::Error> {
        Ok(Value::F64(v))
    }
    fn serialize_char(self, v: char) -> Result<Value, value::Error> {
        Ok(Value::Char(v))
    }
    fn serialize_str(self, v: &str) -> Result<Value, value::Error> {
        Ok(Value::Str(v.into()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Value, value::Error> {
        Ok(Value::Bytes(v.into()))
    }
    fn serialize_none(self) -> Result<Value, value::Error> {
        self.parts.clear();
        Ok(Value::None)
    }
    fn serialize_some<T: Serialize + ?Sized>(mut self, value: &T) -> Result<Value, value::Error> {
        self.parts.truncate(1);
        Ok(Value::Some(Box::new(self.part(0, "", value)?)))
    }
    fn serialize_unit(self) -> Result<Value, value::Error> {
        Ok(Value::Unit)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, value::Error> {
        Ok(Value::Unit)
    }
    fn serialize_unit_variant(self, _name: &'static str, _index: u32, variant: &'static str) -> Result<Value, value::Error> {
        Ok(Value::Variant(std::borrow::Cow::Borrowed(variant), Box::new(Value::Unit)))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _name: &'static str, value: &T) -> Result<Value, value::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, value::Error> {
        self.parts.truncate(1);
        let inner = self.part(0, variant, value)?;
        Ok(Value::Variant(std::borrow::Cow::Borrowed(variant), Box::new(inner)))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, None))
    }
    fn serialize_tuple(self, _len: usize) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, None))
    }
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, None))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, Some(variant)))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, None))
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, None))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Built<'a>, value::Error> {
        Ok(Built::new(self, Some(variant)))
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    use std::cell::Cell;

    struct Counted<'a> {
        value: u64,
        prints: &'a Cell<usize>,
        builds: &'a Cell<usize>,
        refuse: bool,
    }
    impl erased::Field for Counted<'_> {
        fn print(&self, out: &mut Vec<u8>) -> Result<(), value::Error> {
            self.prints.set(self.prints.get() + 1);
            erased::Of(&self.value).print(out)
        }
        fn build(&self) -> Result<Value, value::Error> {
            self.builds.set(self.builds.get() + 1);
            if self.refuse {
                return Err(ser::Error::custom("refused projection"));
            }
            erased::Of(&self.value).build()
        }
        fn cached(
            &self,
            _: &mut Node,
            _: u32,
            _: Option<u64>,
            _: &mut Vec<u8>,
        ) -> Result<Value, value::Error> {
            self.build()
        }
    }

    #[test]
    fn revision_hit_skips_print_and_build_and_failed_refresh_retries() {
        let prints = Cell::new(0);
        let builds = Cell::new(0);
        let mut cache = Fields::default();
        let field = |value, refuse| Counted {
            value,
            prints: &prints,
            builds: &builds,
            refuse,
        };
        cache.start();
        let first = cache
            .revision_field("book", &field(1, false), 1, |_| {})
            .unwrap();
        cache.start();
        assert_eq!(
            cache
                .revision_field("book", &field(1, true), 1, |_| {})
                .unwrap(),
            first
        );
        assert_eq!((prints.get(), builds.get()), (0, 1));
        cache.start();
        assert!(
            cache
                .revision_field("book", &field(2, true), 2, |_| {})
                .is_err()
        );
        cache.start();
        let second = cache
            .revision_field("book", &field(2, false), 2, |_| {})
            .unwrap();
        assert_ne!(first, second);
        assert_eq!((prints.get(), builds.get()), (0, 3));
        // A refused outer frame can reuse the newly built cache on retry;
        // the owner's published tree/revision remains a separate transaction.
        cache.start();
        assert_eq!(
            cache
                .revision_field("book", &field(2, true), 2, |_| {})
                .unwrap(),
            second
        );
        assert_eq!(builds.get(), 3);
        // Restore/new-session owner creates a new cache even with equal stamp.
        cache = Fields::default();
        cache.start();
        let restored = cache
            .revision_field("book", &field(9, false), 2, |_| {})
            .unwrap();
        assert_ne!(restored, second);
        assert_eq!((prints.get(), builds.get()), (0, 4));
    }
}
