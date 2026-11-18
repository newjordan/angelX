//! One wish's behavior, as a module a running game loads, calls, and swaps.
//!
//! The boundary is plain JSON across a C ABI on purpose: a module compiled
//! apart from the game cannot share Rust types with it, and a wish is rare
//! (once per room clear), so a few microseconds of JSON is nothing against a
//! rebuild of the game.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub unsafe extern "C" fn wish_grant(request: *const c_char) -> *mut c_char {
    let ask = unsafe { CStr::from_ptr(request) }.to_string_lossy().into_owned();
    let answer = grant(&ask);
    CString::new(answer).unwrap_or_default().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn wish_free(answer: *mut c_char) {
    if !answer.is_null() {
        drop(unsafe { CString::from_raw(answer) });
    }
}

fn grant(request: &str) -> String {
    let mut ask: serde_json::Value = serde_json::from_str(request).unwrap_or_default();
    let spec = ask["spec"].as_str().unwrap_or("").to_owned();
    let gold = ask["gold"].as_i64().unwrap_or(0).max(0);
    let (cost, says) = price(&spec);
    let left = (gold - cost).max(0);
    if gold < cost {
        return serde_json::json!({"spec": spec, "gold": gold, "says": "not enough gold"}).to_string();
    }
    ask["gold"] = serde_json::json!(left);
    ask["says"] = serde_json::json!(says);
    ask.to_string()
}

fn price(spec: &str) -> (i64, String) {
    match spec {
        "triple shot" => (7, "seven gold, a wish worth telling".to_owned()),
        "iron skin" => (40, "your skin turns aside the first blow".to_owned()),
        other => (10, format!("{other}: a small wish, granted")),
    }
}

#[no_mangle]
pub unsafe extern "C" fn bear_tick(world: *const c_char) -> *mut c_char {
    let text = unsafe { CStr::from_ptr(world) }.to_string_lossy().into_owned();
    let answer = bears(&text);
    CString::new(answer).unwrap_or_default().into_raw()
}

fn bears(world: &str) -> String {
    let seen: serde_json::Value = serde_json::from_str(world).unwrap_or_default();
    let me = &seen["me"];
    let mx = me["x"].as_f64().unwrap_or(0.0);
    let my = me["y"].as_f64().unwrap_or(0.0);
    let killed = seen["events"]
        .as_array()
        .is_some_and(|it| it.iter().any(|e| e.as_str() == Some("kill")));
    let enemies: Vec<(f64, f64, i64)> = seen["enemies"]
        .as_array()
        .map(|it| {
            it.iter()
                .map(|e| (e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0), e["id"].as_i64().unwrap_or(0)))
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for bear in seen["bears"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let id = bear["id"].as_i64().unwrap_or(0);
        let bx = bear["x"].as_f64().unwrap_or(0.0);
        let by = bear["y"].as_f64().unwrap_or(0.0);
        let mana = bear["mana"].as_i64().unwrap_or(0);
        let mut nearest: Option<(f64, i64)> = None;
        for (ex, ey, eid) in &enemies {
            let d = ((ex - bx).powi(2) + (ey - by).powi(2)).sqrt();
            if nearest.is_none_or(|(best, _)| d < best) {
                nearest = Some((d, *eid));
            }
        }
        let dx = mx - bx;
        let dy = my - by;
        let gap = (dx * dx + dy * dy).sqrt();
        let (step_x, step_y) = if gap > 1.5 {
            (dx / gap * 0.25, dy / gap * 0.25)
        } else {
            (0.0, 0.0)
        };
        let target = nearest.filter(|(d, _)| *d < 8.0).map(|(_, id)| id);
        let spell = (mana >= 10 && nearest.is_some_and(|(d, _)| d < 5.0)).then_some("fireball");
        out.push(serde_json::json!({
            "id": id,
            "move": {"x": step_x, "y": step_y},
            "fire": target,
            "dance": killed && target.is_none(),
            "spell": spell,
        }));
    }
    serde_json::json!({"bears": out}).to_string()
}

#[no_mangle]
pub unsafe extern "C" fn spell_cast(spec: *const c_char, world: *const c_char) -> *mut c_char {
    let spec = unsafe { CStr::from_ptr(spec) }.to_string_lossy().into_owned();
    let world = unsafe { CStr::from_ptr(world) }.to_string_lossy().into_owned();
    CString::new(compose(&spec, &world)).unwrap_or_default().into_raw()
}

fn compose(spec: &str, world: &str) -> String {
    let spell: serde_json::Value = serde_json::from_str(spec).unwrap_or_default();
    let seen: serde_json::Value = serde_json::from_str(world).unwrap_or_default();
    let at = &seen["me"];
    let (x, y) = (at["x"].as_f64().unwrap_or(0.0), at["y"].as_f64().unwrap_or(0.0));
    let aim = at["aim"].as_f64().unwrap_or(0.0);
    let count = spell["projectiles"].as_u64().unwrap_or(1).clamp(1, 12) as usize;
    let spread = spell["spread"].as_f64().unwrap_or(0.0).abs().min(1.5);
    let damage = spell["damage"].as_i64().unwrap_or(1).clamp(1, 50);
    let element = spell["element"].as_str().unwrap_or("plain");
    let on_hit = spell["on_hit"].as_str().unwrap_or("none");
    let slow = spell["slow_ms"].as_u64().unwrap_or(0).min(10_000);
    let mut bolts = Vec::with_capacity(count);
    for i in 0..count {
        let offset = match count {
            1 => 0.0,
            n => (i as f64 / (n - 1) as f64 - 0.5) * spread,
        };
        let angle = aim + offset;
        bolts.push(serde_json::json!({
            "kind": "projectile",
            "from": {"x": x, "y": y},
            "heading": {"x": angle.cos(), "y": angle.sin()},
            "damage": damage,
            "element": element,
            "on_hit": on_hit,
            "slow_ms": slow,
        }));
    }
    serde_json::json!({
        "spell": spell["name"].as_str().unwrap_or("an unnamed magic"),
        "effects": bolts,
    })
    .to_string()
}
