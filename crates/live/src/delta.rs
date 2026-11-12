//! Resident-state deltas (docs/architecture/delve-in-bend.md): `["s", path,
//! value]` sets, `["d", path]` deletes an object key, `["t", path, n]`
//! truncates an array. A path is object keys (strings) and indices (integers).
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Key<'a> {
    Str(&'a str),
    Idx(usize),
}

fn to_path(path: &[Key<'_>]) -> Value {
    Value::Array(
        path.iter()
            .map(|k| match *k {
                Key::Str(s) => Value::String(s.to_owned()),
                Key::Idx(i) => Value::from(i),
            })
            .collect(),
    )
}

/// Ops that turn `old` into `new`, applied in order.
pub fn diff(old: &Value, new: &Value) -> Vec<Value> {
    let mut ops = Vec::new();
    walk(old, new, &mut Vec::with_capacity(16), &mut ops);
    ops
}
/// How far down a subtree is first asked whether it is untouched whole
/// (`identical`): a run's fields and their own children. Most of a state is
/// the same as it was a tick ago, and one comparison of a subtree costs a
/// third of walking it; a subtree that did change is left on its first
/// difference, so asking it too is cheap, and below this the walk goes on.
const WHOLE_FROM: usize = 2;
const WHOLE_TO: usize = 3;

fn walk<'a>(old: &'a Value, new: &'a Value, path: &mut Vec<Key<'a>>, ops: &mut Vec<Value>) {
    if (WHOLE_FROM..=WHOLE_TO).contains(&path.len()) && identical(old, new) {
        return;
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in b {
                path.push(Key::Str(key.as_str()));
                match a.get(key) {
                    Some(old) => walk(old, value, path, ops),
                    None => ops.push(json!(["s", to_path(path), value])),
                }
                path.pop();
            }
            for key in a.keys().filter(|key| !b.contains_key(*key)) {
                path.push(Key::Str(key.as_str()));
                ops.push(json!(["d", to_path(path)]));
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, (old, new)) in a.iter().zip(b).enumerate() {
                path.push(Key::Idx(i));
                walk(old, new, path, ops);
                path.pop();
            }
            if b.len() < a.len() {
                ops.push(json!(["t", to_path(path), b.len()]));
            }
            for (i, value) in b.iter().enumerate().skip(a.len()) {
                path.push(Key::Idx(i));
                ops.push(json!(["s", to_path(path), value]));
                path.pop();
            }
        }
        _ if !same(old, new) => ops.push(json!(["s", to_path(path), new])),
        _ => {}
    }
}

/// Nothing differs between the trees, as `walk` would find: equal, and bit for
/// bit where a float is.
fn identical(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|((k, v), (l, w))| k == l && identical(v, w))
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| identical(v, w)),
        _ => same(a, b),
    }
}

/// Equal, and bit for bit when a float: a worker must see `-0.0` become `0.0`.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x == y && x.as_f64().map(f64::to_bits) == y.as_f64().map(f64::to_bits)
        }
        _ => a == b,
    }
}

/// Applies `ops` in order. On an error `v` may be partly changed: patch a copy.
pub fn patch(v: &mut Value, ops: &[Value]) -> Result<(), String> {
    ops.iter()
        .try_for_each(|op| apply(v, op).map_err(|e| format!("delta op {op}: {e}")))
}
fn descend<'a>(mut v: &'a mut Value, path: &[Value]) -> Result<&'a mut Value, String> {
    for key in path {
        v = match (v, key) {
            (Value::Object(map), Value::String(key)) => map.get_mut(key),
            (Value::Array(items), key) => key.as_u64().and_then(|i| items.get_mut(i as usize)),
            _ => None,
        }
        .ok_or_else(|| format!("no {key} in the value"))?;
    }
    Ok(v)
}
fn parent<'a>(
    root: &'a mut Value,
    path: &'a [Value],
) -> Result<(&'a mut Value, &'a Value), String> {
    let (last, rest) = path.split_last().ok_or("the root has no parent")?;
    Ok((descend(root, rest)?, last))
}
fn apply(root: &mut Value, op: &Value) -> Result<(), String> {
    let op = op.as_array().ok_or("op is not an array")?;
    let path = op
        .get(1)
        .and_then(Value::as_array)
        .ok_or("op has no path")?;
    match (op[0].as_str(), op.len()) {
        (Some("s"), 3) if path.is_empty() => *root = op[2].clone(),
        (Some("s"), 3) => match parent(root, path)? {
            (Value::Object(map), Value::String(key)) => {
                map.insert(key.clone(), op[2].clone());
            }
            (Value::Array(items), i) => match i.as_u64().map(|i| i as usize) {
                Some(i) if i < items.len() => items[i] = op[2].clone(),
                Some(i) if i == items.len() => items.push(op[2].clone()),
                _ => return Err("index out of range".into()),
            },
            _ => return Err("path does not fit the value".into()),
        },
        (Some("d"), 2) => match parent(root, path)? {
            (Value::Object(map), Value::String(key)) => {
                map.remove(key).ok_or("no such object key")?;
            }
            _ => return Err("path does not fit the value".into()),
        },
        (Some("t"), 3) => match (descend(root, path)?, op[2].as_u64()) {
            (Value::Array(items), Some(n)) if n as usize <= items.len() => {
                items.truncate(n as usize)
            }
            _ => return Err("not an array that long".into()),
        },
        _ => return Err("unknown op".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `diff` as it was before whole subtrees were asked about first: every node walked.
    fn walk_all<'a>(old: &'a Value, new: &'a Value, path: &mut Vec<Key<'a>>, ops: &mut Vec<Value>) {
        match (old, new) {
            (Value::Object(a), Value::Object(b)) => {
                for (key, value) in b {
                    path.push(Key::Str(key.as_str()));
                    match a.get(key) {
                        Some(old) => walk_all(old, value, path, ops),
                        None => ops.push(json!(["s", to_path(path), value])),
                    }
                    path.pop();
                }
                for key in a.keys().filter(|key| !b.contains_key(*key)) {
                    path.push(Key::Str(key.as_str()));
                    ops.push(json!(["d", to_path(path)]));
                    path.pop();
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                for (i, (old, new)) in a.iter().zip(b).enumerate() {
                    path.push(Key::Idx(i));
                    walk_all(old, new, path, ops);
                    path.pop();
                }
                if b.len() < a.len() {
                    ops.push(json!(["t", to_path(path), b.len()]));
                }
                for (i, value) in b.iter().enumerate().skip(a.len()) {
                    path.push(Key::Idx(i));
                    ops.push(json!(["s", to_path(path), value]));
                    path.pop();
                }
            }
            _ if !same(old, new) => ops.push(json!(["s", to_path(path), new])),
            _ => {}
        }
    }

    struct Dice(u64);
    impl Dice {
        fn next(&mut self, below: u64) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) % below
        }
    }

    /// A tree like a state: objects of objects, lists, text, whole numbers and floats.
    fn tree(dice: &mut Dice, depth: u32) -> Value {
        match if depth == 0 { dice.next(4) } else { dice.next(7) } {
            0 => json!(dice.next(100)),
            1 => json!(format!("text {}", dice.next(5))),
            2 => json!([0.0, -0.0, 1.5, 2.25][dice.next(4) as usize]),
            3 => Value::Bool(dice.next(2) == 0),
            4 | 5 => Value::Array((0..dice.next(5)).map(|_| tree(dice, depth - 1)).collect()),
            _ => Value::Object((0..dice.next(5)).map(|k| (format!("k{k}"), tree(dice, depth - 1))).collect()),
        }
    }

    /// A tree with a few places changed, as a tick changes a state.
    fn changed(dice: &mut Dice, v: &Value) -> Value {
        match v {
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, v) in map {
                    if dice.next(12) == 0 {
                        continue;
                    }
                    let kept = if dice.next(3) == 0 { changed(dice, v) } else { v.clone() };
                    out.insert(k.clone(), kept);
                }
                if dice.next(10) == 0 {
                    out.insert(format!("new{}", dice.next(3)), json!(dice.next(9)));
                }
                Value::Object(out)
            }
            Value::Array(items) => {
                let mut out = Vec::new();
                for v in items {
                    out.push(if dice.next(3) == 0 { changed(dice, v) } else { v.clone() });
                }
                match dice.next(8) {
                    0 => {
                        out.pop();
                    }
                    1 => out.push(json!("pushed")),
                    _ => {}
                }
                Value::Array(out)
            }
            Value::Number(_) if dice.next(4) == 0 => json!([0.0, -0.0, 7.5][dice.next(3) as usize]),
            Value::String(_) if dice.next(4) == 0 => json!("other"),
            other => other.clone(),
        }
    }

    #[test]
    fn asking_whole_subtrees_first_finds_the_same_ops_as_walking_every_node() {
        let mut dice = Dice(0xde17a);
        let mut with_ops = 0;
        for case in 0..400 {
            // A state: a run of fields, each a tree, under the checkpoint's envelope.
            let run: Value = Value::Object((0..8).map(|k| (format!("field{k}"), tree(&mut dice, 5))).collect());
            let old = json!({"schema": 8, "run": run});
            let new = changed(&mut dice, &old);
            let mut reference = Vec::new();
            walk_all(&old, &new, &mut Vec::new(), &mut reference);
            let ops = diff(&old, &new);
            assert_eq!(ops, reference, "case {case}");
            with_ops += usize::from(!ops.is_empty());
            let mut patched = old.clone();
            patch(&mut patched, &ops).unwrap();
            assert!(identical(&patched, &new), "case {case}: the ops turn old into new");
        }
        assert!(with_ops > 100, "the cases change something often enough ({with_ops})");
    }

    #[test]
    fn a_float_that_differs_only_in_its_sign_of_zero_is_still_an_op() {
        let old = json!({"schema": 8, "run": {"field": {"x": 0.0, "list": [1, 2]}}});
        let new = json!({"schema": 8, "run": {"field": {"x": -0.0, "list": [1, 2]}}});
        assert_eq!(old, new, "serde_json's own comparison calls them equal");
        let ops = diff(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0][0], "s");
        assert_eq!(ops[0][1], json!(["run", "field", "x"]));
    }
}
