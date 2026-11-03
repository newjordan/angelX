//! The seam to the Bend leg `deathmatch-rules` (`cockpit/live/deathmatch-rules`).
//! Rust measures (distances, sight, the kit's numbers) and applies; the leg
//! decides. A question is one JSON object with an `op`, its facts named as
//! Rust's own fields; the answer is applied by the same names. A missing or
//! refused answer leaves the round where it stood.
use super::*;
use serde_json::Value;

pub(super) fn ask(op: &str, mut body: Value) -> Result<Value, String> {
    body["op"] = op.into();
    let answer = crate::scripted::step("deathmatch-rules", &body);
    #[cfg(test)]
    if let Err(e) = &answer
        && e.starts_with("no ")
    {
        panic!("a test that plays Knights Wish needs the cockpit's engine: {e}");
    }
    answer
}
/// A number out of an answer.
pub(super) fn num(v: &Value) -> u32 {
    v.as_u64().unwrap_or(0) as u32
}
/// The named fields of `$o`, as the JSON object a question carries.
macro_rules! pick {
    ($o:expr; $($f:ident),+) => { serde_json::json!({ $(stringify!($f): $o.$f),+ }) };
}
/// The named fields of `$o`, set from the answer `$v`.
macro_rules! put {
    ($v:expr => $o:expr; $($f:ident),+) => {
        $( $o.$f = serde_json::from_value($v[stringify!($f)].clone()).map_err(|e| e.to_string())?; )+
    };
}
impl Fighter {
    /// The sound a fresh muzzle flash makes: a blade swings, a bow twangs.
    pub(super) fn shot_sound(&self) -> &'static str {
        let h = &self.hero;
        if h.forged.as_ref().is_some_and(|w| w.melee.is_some()) || h.unarmed() {
            "swing"
        } else {
            classes::shot_sound(h).unwrap_or(match h.weapon {
                Weapon::Bow => "shot_bow",
                Weapon::Crossbow => "shot_crossbow",
                Weapon::Handgonne => "shot_handgonne",
                _ => "shot_spark",
            })
        }
    }
}
