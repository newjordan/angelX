//! What changed between two trees, and putting it right on the other side.
//!
//! A struct sends only its changed fields, a map only its changed entries,
//! and a list of things with ids (monsters, shots, sparks) only the things
//! that moved, came or went, by id. Everything else is sent whole when it
//! differs. Nothing here knows the run's shape.

use super::value::{Name, Value};

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
    /// A list of things with ids: their order now, and those that changed
    /// or are new.
    Keyed {
        order: Vec<u64>,
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

/// What turns `old` into `new`, or None when they are the same.
pub fn diff(old: &Value, new: &Value) -> Option<Patch> {
    if old == new {
        return None;
    }
    Some(match (old, new) {
        (Value::Struct(a), Value::Struct(b)) => {
            let mut set = Vec::new();
            for (name, value) in b {
                match a.iter().find(|(n, _)| n == name) {
                    Some((_, was)) => {
                        if let Some(patch) = diff(was, value) {
                            set.push((name.clone(), patch));
                        }
                    }
                    None => set.push((name.clone(), Patch::Set(value.clone()))),
                }
            }
            let gone = a
                .iter()
                .filter(|(n, _)| !b.iter().any(|(m, _)| m == n))
                .map(|(n, _)| n.clone())
                .collect();
            Patch::Struct { set, gone }
        }
        (Value::Seq(a), Value::Seq(b)) => seq(a, b),
        (Value::Map(a), Value::Map(b)) => map(a, b),
        (Value::Some(a), Value::Some(b)) => match diff(a, b) {
            Some(patch) => Patch::Some(Box::new(patch)),
            None => return None,
        },
        (Value::Variant(n, a), Value::Variant(m, b)) if n == m => match diff(a, b) {
            Some(patch) => Patch::Variant(Box::new(patch)),
            None => return None,
        },
        _ => Patch::Set(new.clone()),
    })
}

/// A thing's id, when every item of a list is a struct with a distinct one.
fn ids(items: &[Value]) -> Option<Vec<u64>> {
    let ids: Option<Vec<u64>> = items
        .iter()
        .map(|item| item.field("id").and_then(Value::as_id))
        .collect();
    let ids = ids?;
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    (sorted.len() == ids.len()).then_some(ids)
}

fn seq(a: &[Value], b: &[Value]) -> Patch {
    if !b.is_empty()
        && let (Some(old_ids), Some(new_ids)) = (ids(a), ids(b))
    {
        let mut items = Vec::new();
        for (id, value) in new_ids.iter().zip(b) {
            match old_ids.iter().position(|o| o == id) {
                Some(i) => {
                    if let Some(patch) = diff(&a[i], value) {
                        items.push((*id, patch));
                    }
                }
                None => items.push((*id, Patch::Set(value.clone()))),
            }
        }
        return Patch::Keyed {
            order: new_ids,
            items,
        };
    }
    let items = b
        .iter()
        .enumerate()
        .filter_map(|(i, value)| match a.get(i) {
            Some(was) => diff(was, value).map(|patch| (i as u32, patch)),
            None => Some((i as u32, Patch::Set(value.clone()))),
        })
        .collect();
    Patch::Seq {
        len: b.len() as u32,
        items,
    }
}

fn map(a: &[(Value, Value)], b: &[(Value, Value)]) -> Patch {
    let mut items = Vec::new();
    for (key, value) in b {
        match a.iter().find(|(k, _)| k == key) {
            Some((_, was)) => {
                if let Some(patch) = diff(was, value) {
                    items.push((key.clone(), patch));
                }
            }
            None => items.push((key.clone(), Patch::Set(value.clone()))),
        }
    }
    let gone = a
        .iter()
        .filter(|(k, _)| !b.iter().any(|(m, _)| m == k))
        .map(|(k, _)| k.clone())
        .collect();
    Patch::Map { gone, items }
}

/// Why a patch did not fit the tree it was applied to.
#[derive(Debug, PartialEq)]
pub struct Mismatch;

/// Turn `value` into what the patch describes.
pub fn apply(value: &mut Value, patch: Patch) -> Result<(), Mismatch> {
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
        Patch::Keyed { order, items } => {
            let Value::Seq(list) = value else {
                return Err(Mismatch);
            };
            let mut old: Vec<(u64, Value)> = std::mem::take(list)
                .into_iter()
                .filter_map(|item| item.field("id").and_then(Value::as_id).map(|id| (id, item)))
                .collect();
            let mut items = items.into_iter().peekable();
            for id in order {
                let mut item = match old.iter().position(|(o, _)| *o == id) {
                    Some(i) => old.swap_remove(i).1,
                    None => Value::Unit,
                };
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
