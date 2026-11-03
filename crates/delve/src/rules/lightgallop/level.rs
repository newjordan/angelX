//! The Moonwell Road, laid out once: four acts of one road at dusk, the
//! Downhill Gate, the Wheel Arch, the Reed Fen and the Bell Gate. The module
//! (`cockpit/live/lightgallop-rules`) builds it, rides it and keeps its
//! rules; this asks it for the road once so the picture and the save check
//! can read the same curves, springs, posts and foes.
use super::world::Level;
use std::sync::OnceLock;

/// The road, asked for once.
pub fn level() -> &'static Level {
    static LEVEL: OnceLock<Level> = OnceLock::new();
    LEVEL.get_or_init(|| {
        let mut value: serde_json::Value = super::ask(serde_json::json!({ "op": "level" })).expect("the road the module lays out");
        unpack_road(&mut value).expect("valid road geometry");
        serde_json::from_value(value).expect("the road the module lays out")
    })
}

/// Expand the module's compact signed-delta geometry; old oracle answers
/// already contain arrays and remain unchanged.
pub fn unpack_road(value: &mut serde_json::Value) -> Result<(), String> {
    let paths = value.pointer_mut("/terrain/paths").and_then(|v| v.as_array_mut()).ok_or("missing road paths")?;
    for path in paths {
        for (name, paired) in [("pts", true), ("cum", false), ("dir", true), ("mid", false)] {
            let field = path.get_mut(name).ok_or("missing geometry")?;
            let Some(encoded) = field.as_str() else { continue };
            let mut values = Vec::new();
            let mut last = 0i64;
            let mut number = 0u64;
            let mut shift = 0u32;
            for byte in encoded.bytes() {
                let digit = match byte { b'A'..=b'Z' => byte-b'A', b'a'..=b'z' => byte-b'a'+26, b'0'..=b'9' => byte-b'0'+52, b'-' => 62, b'_' => 63, _ => return Err("invalid road digit".into()) };
                if shift >= 35 { return Err("road integer overflow".into()); }
                number |= u64::from(digit & 31) << shift;
                if digit & 32 != 0 { shift += 5; continue; }
                let delta = if number & 1 == 0 { (number / 2) as i64 } else { -((number / 2) as i64) - 1 };
                last = last.checked_add(delta).ok_or("road integer overflow")?;
                let n = i32::try_from(last).map_err(|_| "road integer out of range")?;
                values.push(serde_json::json!(n));
                number = 0; shift = 0;
            }
            if shift != 0 { return Err("truncated road integer".into()); }
            if paired {
                if values.len() % 2 != 0 { return Err("unpaired road coordinate".into()); }
                *field = serde_json::Value::Array(values.chunks(2).map(|v| serde_json::Value::Array(v.to_vec())).collect());
            } else { *field = serde_json::Value::Array(values); }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_broken_packed_geometry() {
        for packed in ["?", "g", "B", "ggggggggA"] {
            let mut value = serde_json::json!({"terrain":{"paths":[{"pts":packed,"cum":[],"dir":[],"mid":[]}]}});
            assert!(unpack_road(&mut value).is_err(), "{packed}");
        }
    }
}
