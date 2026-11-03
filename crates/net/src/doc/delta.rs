//! What changed between two trees, and putting it right on the other side.
//!
//! A struct sends only its changed fields, a map only its changed entries,
//! and a list of things with ids (monsters, shots, sparks) only the things
//! that moved, came or went, by id. Everything else is sent whole when it
//! differs. Nothing here knows the run's shape.

use super::value::{Name, Value};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A hasher for a list's ids (small whole numbers, one apiece): one
/// multiply, where the standard one is a cipher. Ids come from the host
/// that is sending the list, who could already send whatever they like.
#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

type ById<V> = HashMap<u64, V, BuildHasherDefault<IdHasher>>;

#[derive(Clone, Debug, PartialEq)]
pub enum Patch {
    /// The value, whole.
    Set(Value),
    /// Changed fields, and the names of fields now absent.
    Struct {
        set: Vec<(Name, Patch)>,
        gone: Vec<Name>,
    },
    /// A list by position: its new length and the items that changed.
    Seq { len: u32, items: Vec<(u32, Patch)> },
    /// A list by position whose first `skip` items went (a list of what
    /// fades, the oldest first): the rest moved up, then as `Seq`.
    SeqShift { skip: u32, len: u32, items: Vec<(u32, Patch)> },
    /// A list of things with ids: their order now, and those that changed
    /// or are new.
    Keyed {
        order: Vec<u64>,
        items: Vec<(u64, Patch)>,
    },
    /// A list of things with ids still in the order it had, some gone and
    /// new ones after the rest (shots loosed, monsters come): the ids gone,
    /// and those that changed or are new, the new in their order.
    KeyedEdit {
        gone: Vec<u64>,
        items: Vec<(u64, Patch)>,
    },
    /// A map: keys gone, and entries that changed or are new.
    Map {
        gone: Vec<Value>,
        items: Vec<(Value, Patch)>,
    },
    /// Still `Some`, and what changed inside.
    Some(Box<Patch>),
    /// The same variant, and what changed inside.
    Variant(Box<Patch>),
}

/// What turns `old` into `new`, or None when they are the same. A subtree
/// both trees share (`Value::Shared`) is passed by unread; any other is
/// walked once, not compared whole at each level on the way down.
pub fn diff(old: &Value, new: &Value) -> Option<Patch> {
    diff_with_order(old, new, false)
}

/// Diff with exact Struct/Map entry order for private owner read views.
/// Legacy wire callers retain their established compact patch representation.
pub fn diff_exact(old: &Value, new: &Value) -> Option<Patch> {
    diff_with_order(old, new, true)
}

fn diff_with_order(old: &Value, new: &Value, exact: bool) -> Option<Patch> {
    if let (Value::Shared(a), Value::Shared(b)) = (old, new)
        && std::sync::Arc::ptr_eq(a, b)
    {
        return None;
    }
    match (old.get(), new.get()) {
        (Value::Struct(a), Value::Struct(b)) => {
            let same_names = a.len() == b.len() && a.iter().zip(b).all(|((n, _), (m, _))| n == m);
            // Struct patches retain surviving fields then append additions.
            // Optional serde fields can reappear in the middle; send the whole
            // subtree when that order cannot be represented by this patch.
            if exact && !same_names && !append_order(a, b) {
                return Some(Patch::Set(new.clone()));
            }
            let mut set = Vec::new();
            for (i, (name, value)) in b.iter().enumerate() {
                // A struct's fields come in the same order tick to tick.
                let was = match a.get(i) {
                    Some((n, v)) if n == name => Some(v),
                    _ => a.iter().find(|(n, _)| n == name).map(|(_, v)| v),
                };
                match was {
                    Some(was) => {
                        if let Some(patch) = diff_with_order(was, value, exact) {
                            set.push((name.clone(), patch));
                        }
                    }
                    None => set.push((name.clone(), Patch::Set(value.clone()))),
                }
            }
            let gone: Vec<Name> = if same_names {
                Vec::new()
            } else {
                a.iter().filter(|(n, _)| !b.iter().any(|(m, _)| m == n)).map(|(n, _)| n.clone()).collect()
            };
            (!set.is_empty() || !gone.is_empty()).then_some(Patch::Struct { set, gone })
        }
        (Value::Seq(a), Value::Seq(b)) => seq(a, b, exact),
        (Value::Map(a), Value::Map(b)) => map(a, b, exact),
        (Value::Some(a), Value::Some(b)) => diff_with_order(a, b, exact).map(|patch| Patch::Some(Box::new(patch))),
        (Value::Variant(n, a), Value::Variant(m, b)) if n == m => {
            diff_with_order(a, b, exact).map(|patch| Patch::Variant(Box::new(patch)))
        }
        (a, b) => (a != b).then(|| Patch::Set(new.clone())),
    }
}

/// A thing's id: a struct's `id` field. A list of things puts it last
/// (`angel_delve::wire::flight::shot_value`), so the walk starts there.
fn id_of(item: &Value) -> Option<u64> {
    match item.get() {
        Value::Struct(fields) => fields.iter().rev().find(|(n, _)| n == "id").and_then(|(_, v)| v.as_id()),
        _ => None,
    }
}

/// A thing's id, when every item of a list is a struct with a distinct one.
fn ids(items: &[Value]) -> Option<Vec<u64>> {
    let ids: Option<Vec<u64>> = items.iter().map(id_of).collect();
    let ids = ids?;
    // Things numbered as they come are in order, and so distinct.
    if ids.windows(2).all(|w| w[0] < w[1]) {
        return Some(ids);
    }
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    (sorted.len() == ids.len()).then_some(ids)
}

fn seq(a: &[Value], b: &[Value], exact: bool) -> Option<Patch> {
    if !b.is_empty()
        && let (Some(old_ids), Some(new_ids)) = (ids(a), ids(b))
    {
        return keyed(a, b, old_ids, new_ids, exact);
    }
    let skip = shift(a, b);
    let rest = &a[skip..];
    let items: Vec<(u32, Patch)> = b
        .iter()
        .enumerate()
        .filter_map(|(i, value)| match rest.get(i) {
            Some(was) => diff_with_order(was, value, exact).map(|patch| (i as u32, patch)),
            None => Some((i as u32, Patch::Set(value.clone()))),
        })
        .collect();
    let len = b.len() as u32;
    if skip > 0 {
        return Some(Patch::SeqShift { skip: skip as u32, len, items });
    }
    (!items.is_empty() || a.len() != b.len()).then_some(Patch::Seq { len, items })
}

/// The furthest a list's front is looked for further down the old list.
const MOST_SHIFT: usize = 64;

/// A few ids are found by a walk of the list; more are found by where each
/// stands.
const SMALL: usize = 8;

/// How many items went from the front of a list of structs, when more of
/// what is left lines up with the new list that way than as it stood: 0
/// when none did.
fn shift(a: &[Value], b: &[Value]) -> usize {
    let (Some(first), Some(was)) = (b.first(), a.first()) else {
        return 0;
    };
    if !matches!(first.get(), Value::Struct(_)) || was == first {
        return 0;
    }
    let Some(skip) = a.iter().take(MOST_SHIFT).skip(1).position(|v| v == first).map(|i| i + 1) else {
        return 0;
    };
    let alike = |from: &[Value]| from.iter().zip(b).filter(|(x, y)| x == y).count();
    if alike(&a[skip..]) > alike(a) { skip } else { 0 }
}

/// A list of things with ids: as an edit of the old order when it is one
/// (`KeyedEdit`), else with the whole order (`Keyed`).
fn keyed(a: &[Value], b: &[Value], old_ids: Vec<u64>, new_ids: Vec<u64>, exact: bool) -> Option<Patch> {
    let mut items = Vec::new();
    let mut gone = Vec::new();
    let (mut at, mut tail, mut edit) = (0, false, true);
    // Where each old id stands, worked out once the first of the rest asks
    // (a walk of the old order for each would be the square of the list).
    let mut stands: Option<ById<usize>> = None;
    for (&id, value) in new_ids.iter().zip(b) {
        if !tail {
            if let Some(off) = old_ids[at..].iter().position(|&o| o == id) {
                gone.extend_from_slice(&old_ids[at..at + off]);
                if let Some(patch) = diff_with_order(&a[at + off], value, exact) {
                    items.push((id, patch));
                }
                at += off + 1;
                continue;
            }
            tail = true;
            gone.extend_from_slice(&old_ids[at..]);
            at = old_ids.len();
        }
        // After the old ones, only new ones.
        if stood(&old_ids, &mut stands, id).is_some() {
            edit = false;
            break;
        }
        items.push((id, Patch::Set(value.clone())));
    }
    if edit {
        gone.extend_from_slice(&old_ids[at..]);
        return (!gone.is_empty() || !items.is_empty()).then_some(Patch::KeyedEdit { gone, items });
    }
    let mut items = Vec::new();
    for (id, value) in new_ids.iter().zip(b) {
        match stood(&old_ids, &mut stands, *id) {
            Some(i) => {
                if let Some(patch) = diff_with_order(&a[i], value, exact) {
                    items.push((*id, patch));
                }
            }
            None => items.push((*id, Patch::Set(value.clone()))),
        }
    }
    Some(Patch::Keyed { order: new_ids, items })
}

/// Where `id` stands in `old_ids`: found by a walk of a short list, by
/// the places worked out once (`stands`) of a long one.
fn stood(old_ids: &[u64], stands: &mut Option<ById<usize>>, id: u64) -> Option<usize> {
    if old_ids.len() <= SMALL * 4 {
        return old_ids.iter().position(|&o| o == id);
    }
    stands
        .get_or_insert_with(|| old_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect())
        .get(&id)
        .copied()
}

/// Whether retaining old surviving entries and appending new ones produces
/// exactly the requested order. This is the order Struct/Map patches encode.
fn append_order<K: PartialEq, V>(a: &[(K, V)], b: &[(K, V)]) -> bool {
    let mut surviving = a.iter().filter(|(key, _)| b.iter().any(|(other, _)| key == other));
    let mut appended = false;
    for (key, _) in b {
        if a.iter().any(|(old, _)| old == key) {
            if appended || surviving.next().is_none_or(|(old, _)| old != key) {
                return false;
            }
        } else {
            appended = true;
        }
    }
    true
}

fn map(a: &[(Value, Value)], b: &[(Value, Value)], exact: bool) -> Option<Patch> {
    let same_keys = a.len() == b.len() && a.iter().zip(b).all(|((k, _), (m, _))| k == m);
    if exact && !same_keys && !append_order(a, b) {
        return Some(Patch::Set(Value::Map(b.to_vec())));
    }
    let mut items = Vec::new();
    for (i, (key, value)) in b.iter().enumerate() {
        // A map's keys come in the same order tick to tick.
        let was = match a.get(i) {
            Some((k, v)) if k == key => Some(v),
            _ => a.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        };
        match was {
            Some(was) => {
                if let Some(patch) = diff_with_order(was, value, exact) {
                    items.push((key.clone(), patch));
                }
            }
            None => items.push((key.clone(), Patch::Set(value.clone()))),
        }
    }
    let gone: Vec<Value> = if same_keys {
        Vec::new()
    } else {
        a.iter().filter(|(k, _)| !b.iter().any(|(m, _)| m == k)).map(|(k, _)| k.clone()).collect()
    };
    (!gone.is_empty() || !items.is_empty()).then_some(Patch::Map { gone, items })
}

/// Why a patch did not fit the tree it was applied to.
#[derive(Debug, PartialEq)]
pub struct Mismatch;

/// Turn `value` into what the patch describes.
pub fn apply(value: &mut Value, patch: Patch) -> Result<(), Mismatch> {
    if !matches!(patch, Patch::Set(_)) {
        // Into what a shared subtree holds, copied first if need be.
        return apply_into(value.get_mut(), patch);
    }
    apply_into(value, patch)
}

fn apply_into(value: &mut Value, patch: Patch) -> Result<(), Mismatch> {
    match patch {
        Patch::Set(new) => *value = new,
        Patch::Struct { set, gone } => {
            let Value::Struct(fields) = value else {
                return Err(Mismatch);
            };
            fields.retain(|(n, _)| !gone.contains(n));
            for (name, patch) in set {
                match fields.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, field)) => apply(field, patch)?,
                    None => match patch {
                        Patch::Set(new) => fields.push((name, new)),
                        _ => return Err(Mismatch),
                    },
                }
            }
        }
        Patch::Seq { len, items } => {
            let Value::Seq(list) = value else {
                return Err(Mismatch);
            };
            let len = len as usize;
            list.truncate(len);
            for (i, patch) in items {
                let i = i as usize;
                if i < list.len() {
                    apply(&mut list[i], patch)?;
                } else if i == list.len()
                    && let Patch::Set(new) = patch
                {
                    list.push(new);
                } else {
                    return Err(Mismatch);
                }
            }
            if list.len() != len {
                return Err(Mismatch);
            }
        }
        Patch::SeqShift { skip, len, items } => {
            let Value::Seq(list) = value else {
                return Err(Mismatch);
            };
            let skip = skip as usize;
            if skip > list.len() {
                return Err(Mismatch);
            }
            list.drain(..skip);
            apply_into(value, Patch::Seq { len, items })?;
        }
        Patch::KeyedEdit { gone, items } => {
            let Value::Seq(list) = value else {
                return Err(Mismatch);
            };
            // A crowd of shots is a crowd of ids to drop and to find: each
            // looked up where it stands, never by a walk of the list.
            if gone.len() > SMALL {
                let gone: ById<()> = gone.iter().map(|&id| (id, ())).collect();
                list.retain(|item| id_of(item).is_none_or(|id| !gone.contains_key(&id)));
            } else if !gone.is_empty() {
                list.retain(|item| id_of(item).is_none_or(|id| !gone.contains(&id)));
            }
            let mut stands: Option<ById<usize>> = (items.len() > SMALL)
                .then(|| list.iter().enumerate().filter_map(|(i, item)| id_of(item).map(|id| (id, i))).collect());
            for (id, patch) in items {
                let at = match &stands {
                    Some(stands) => stands.get(&id).copied(),
                    None => list.iter().position(|item| id_of(item) == Some(id)),
                };
                match at {
                    Some(i) => apply(&mut list[i], patch)?,
                    None => match patch {
                        Patch::Set(new) if id_of(&new) == Some(id) => {
                            if let Some(stands) = &mut stands {
                                stands.insert(id, list.len());
                            }
                            list.push(new);
                        }
                        _ => return Err(Mismatch),
                    },
                }
            }
        }
        Patch::Keyed { order, items } => {
            let Value::Seq(list) = value else {
                return Err(Mismatch);
            };
            let mut old: ById<Value> = std::mem::take(list)
                .into_iter()
                .filter_map(|item| id_of(&item).map(|id| (id, item)))
                .collect();
            let mut items = items.into_iter().peekable();
            for id in order {
                let mut item = old.remove(&id).unwrap_or(Value::Unit);
                if items.peek().is_some_and(|(i, _)| *i == id) {
                    let (_, patch) = items.next().ok_or(Mismatch)?;
                    apply(&mut item, patch)?;
                }
                if item == Value::Unit {
                    return Err(Mismatch);
                }
                list.push(item);
            }
            if items.next().is_some() {
                return Err(Mismatch);
            }
        }
        Patch::Map { gone, items } => {
            let Value::Map(entries) = value else {
                return Err(Mismatch);
            };
            entries.retain(|(k, _)| !gone.contains(k));
            for (key, patch) in items {
                match entries.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, entry)) => apply(entry, patch)?,
                    None => match patch {
                        Patch::Set(new) => entries.push((key, new)),
                        _ => return Err(Mismatch),
                    },
                }
            }
        }
        Patch::Some(patch) => {
            let Value::Some(inner) = value else {
                return Err(Mismatch);
            };
            apply(inner, *patch)?;
        }
        Patch::Variant(patch) => {
            let Value::Variant(_, inner) = value else {
                return Err(Mismatch);
            };
            apply(inner, *patch)?;
        }
    }
    Ok(())
}
