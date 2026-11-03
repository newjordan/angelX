//! Resident-state deltas (docs/architecture/delve-in-bend.md): `["s", path,
//! value]` sets, `["d", path]` deletes an object key, `["t", path, n]`
//! truncates an array. A path is object keys (strings) and indices (integers).
use serde_json::{Value, json};

/// Ops that turn `old` into `new`, applied in order.
pub fn diff(old: &Value, new: &Value) -> Vec<Value> {
    let mut ops = Vec::new();
    walk(old, new, &mut Vec::new(), &mut ops);
    ops
}
fn walk(old: &Value, new: &Value, path: &mut Vec<Value>, ops: &mut Vec<Value>) {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in b {
                path.push(key.as_str().into());
                match a.get(key) {
                    Some(old) => walk(old, value, path, ops),
                    None => ops.push(json!(["s", path, value])),
                }
                path.pop();
            }
            for key in a.keys().filter(|key| !b.contains_key(*key)) {
                path.push(key.as_str().into());
                ops.push(json!(["d", path]));
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, (old, new)) in a.iter().zip(b).enumerate() {
                path.push(i.into());
                walk(old, new, path, ops);
                path.pop();
            }
            if b.len() < a.len() {
                ops.push(json!(["t", path, b.len()]));
            }
            for (i, value) in b.iter().enumerate().skip(a.len()) {
                path.push(i.into());
                ops.push(json!(["s", path, value]));
                path.pop();
            }
        }
        _ if !same(old, new) => ops.push(json!(["s", path, new])),
        _ => {}
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
