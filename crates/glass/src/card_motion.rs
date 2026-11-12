//! Prepared, bounded card actor tracks. State names are neutral card facts;
//! pose, direction and movement are authored by the selected squire pack.
use crate::squire::{ClipStep, ShowcaseSquireClip};
use std::collections::BTreeMap;

#[cfg(feature = "remote")]
fn is_false(value: &bool) -> bool {
    !value
}

const STATES: [&str; 7] = ["sit", "wave", "present", "busy", "oops", "cheer", "guard"];

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize))]
pub struct CardMotion {
    pub(crate) states: BTreeMap<String, Track>,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Track {
    steps: Vec<Step>,
    repeat: bool,
    #[cfg_attr(feature = "remote", serde(skip_serializing_if = "Option::is_none"))]
    then: Option<String>,
    #[cfg_attr(feature = "remote", serde(skip))]
    ends: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Step {
    frame: u8,
    #[cfg_attr(feature = "remote", serde(skip_serializing_if = "Option::is_none"))]
    clip: Option<String>,
    millis: u16,
    #[cfg_attr(feature = "remote", serde(skip_serializing_if = "Anchor::is_stand"))]
    anchor: Anchor,
    offset: [f64; 2],
    #[cfg_attr(feature = "remote", serde(skip_serializing_if = "is_false"))]
    left: bool,
    interpolate: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "remote",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub(crate) enum Anchor {
    Seat,
    Stand,
}
impl Anchor {
    #[cfg(feature = "remote")]
    fn is_stand(&self) -> bool {
        matches!(self, Self::Stand)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sample {
    pub frame: u8,
    pub anchor: Anchor,
    pub offset: [f32; 2],
    pub left: bool,
}
impl CardMotion {
    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        use serde_json::Value;
        fn object<'a>(
            v: &'a Value,
            allowed: &[&str],
        ) -> Result<&'a serde_json::Map<String, Value>, String> {
            let o = v.as_object().ok_or("card motion needs an object")?;
            if o.keys().any(|k| !allowed.contains(&k.as_str())) {
                return Err("unknown card motion field".into());
            }
            Ok(o)
        }
        object(value, &["states"])?;
        let states = value["states"]
            .as_object()
            .ok_or("card motion needs states")?;
        if states.len() != STATES.len() || STATES.iter().any(|s| !states.contains_key(*s)) {
            return Err("card motion needs exactly seven card states".into());
        }
        let mut prepared = BTreeMap::new();
        let mut total_steps = 0;
        for (name, track) in states {
            object(track, &["steps", "repeat", "then"])?;
            let repeat = track["repeat"]
                .as_bool()
                .ok_or("card motion repeat must be boolean")?;
            let then = match track.get("then") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if STATES.contains(&s.as_str()) => Some(s.clone()),
                _ => return Err("card motion tail must name a state".into()),
            };
            if repeat && then.is_some() {
                return Err("repeating card motion cannot have a tail".into());
            }
            let values = track["steps"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 2048)
                .ok_or("card motion needs 1..2048 steps")?;
            total_steps += values.len();
            if total_steps > 8192 {
                return Err("card motion step budget exceeded".into());
            }
            let mut steps = Vec::with_capacity(values.len());
            let mut ends = Vec::with_capacity(values.len());
            let mut elapsed = 0u32;
            for v in values {
                object(
                    v,
                    &[
                        "frame",
                        "clip",
                        "millis",
                        "anchor",
                        "offset",
                        "left",
                        "interpolate",
                    ],
                )?;
                let frame = v["frame"]
                    .as_u64()
                    .filter(|n| *n < 9)
                    .ok_or("card motion frame must be 0..8")? as u8;
                let clip = match v.get("clip") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(s)) if ["idle", "wave"].contains(&s.as_str()) => {
                        Some(s.clone())
                    }
                    _ => return Err("card motion clip must be idle or wave".into()),
                };
                let millis = v["millis"]
                    .as_u64()
                    .filter(|n| (1..=10_000).contains(n))
                    .ok_or("card motion duration must be 1..10000ms")?
                    as u16;
                elapsed = elapsed
                    .checked_add(u32::from(millis))
                    .filter(|n| *n <= 60_000)
                    .ok_or("card motion exceeds 60 seconds")?;
                ends.push(elapsed);
                let anchor = match v.get("anchor") {
                    None => Anchor::Stand,
                    Some(Value::String(name)) if name == "stand" => Anchor::Stand,
                    Some(Value::String(name)) if name == "seat" => Anchor::Seat,
                    _ => return Err("card motion anchor must be seat or stand".into()),
                };
                let xy = v["offset"]
                    .as_array()
                    .filter(|a| a.len() == 2)
                    .ok_or("card motion offset needs two numbers")?;
                let mut offset = [0.; 2];
                for (out, raw) in offset.iter_mut().zip(xy) {
                    *out = raw
                        .as_f64()
                        .filter(|n| n.is_finite() && (-128.0..=128.0).contains(n))
                        .ok_or("card motion offset exceeds 128 pixels")?;
                }
                let left = match v.get("left") {
                    None => false,
                    Some(value) => value.as_bool().ok_or("card motion left must be boolean")?,
                };
                let interpolate = v["interpolate"]
                    .as_bool()
                    .ok_or("card motion interpolate must be boolean")?;
                steps.push(Step {
                    frame,
                    clip,
                    millis,
                    anchor,
                    offset,
                    left,
                    interpolate,
                });
            }
            prepared.insert(
                name.clone(),
                Track {
                    steps,
                    repeat,
                    then,
                    ends,
                },
            );
        }
        for track in prepared.values() {
            if let Some(next) = &track.then {
                let target = &prepared[next];
                if !target.repeat || target.then.is_some() {
                    return Err("card motion tail must end in a repeating state".into());
                }
            }
        }
        Ok(Self { states: prepared })
    }
    pub(crate) fn sample(
        &self,
        state: &str,
        elapsed: f32,
        idle: &[ClipStep],
        wave: &[ClipStep],
    ) -> Option<Sample> {
        let t = if elapsed.is_finite() {
            f64::from(elapsed.max(0.) * 1000.)
        } else {
            0.
        };
        let mut track = self.states.get(state)?;
        let mut time = t;
        let duration = f64::from(*track.ends.last()?);
        if !track.repeat && time >= duration {
            if let Some(next) = &track.then {
                track = self.states.get(next)?;
                time -= duration;
            }
        }
        let total = f64::from(*track.ends.last()?);
        let phase = if track.repeat {
            time % total
        } else {
            time.min(total)
        };
        let index = track
            .ends
            .partition_point(|end| f64::from(*end) <= phase)
            .min(track.steps.len() - 1);
        let step = &track.steps[index];
        let start = if index == 0 { 0 } else { track.ends[index - 1] };
        let next = track
            .steps
            .get(index + 1)
            .or_else(|| track.repeat.then(|| &track.steps[0]));
        let mut offset = step.offset.map(|coordinate| coordinate as f32);
        if step.interpolate
            && let Some(next) = next
            && step.anchor == next.anchor
        {
            let u = ((phase - f64::from(start)) / f64::from(step.millis)).clamp(0., 1.);
            offset = std::array::from_fn(|i| {
                (step.offset[i] + (next.offset[i] - step.offset[i]) * u) as f32
            });
        }
        let frame = match step.clip.as_deref() {
            Some("idle") => ShowcaseSquireClip::frame_at(idle, (time / 1000.) as f32, true).0,
            Some("wave") => ShowcaseSquireClip::frame_at(wave, (time / 1000.) as f32, true).0,
            _ => step.frame,
        };
        Some(Sample {
            frame,
            anchor: step.anchor,
            offset,
            left: step.left,
        })
    }
}

// Rebuild the bounded lookup cache when decoding JSON; it is never wire data.
#[cfg(feature = "remote")]
impl<'de> serde::Deserialize<'de> for CardMotion {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let value = <serde_json::Value as serde::Deserialize>::deserialize(decoder)?;
        Self::from_json(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn authored() -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../cockpit/live/showcase-card-motion/pack.json"
        ))
        .unwrap()
    }
    #[test]
    fn bounded_tracks_reject_invalid_or_ambiguous_policy() {
        let valid = authored();
        assert!(CardMotion::from_json(&valid).is_ok());
        for (key, replacement) in [
            ("millis", serde_json::json!(0)),
            ("millis", serde_json::json!(10001)),
            ("offset", serde_json::json!([129, 0])),
            ("frame", serde_json::json!(9)),
            ("clip", serde_json::json!("unknown")),
            ("interpolate", serde_json::Value::Null),
        ] {
            let mut bad = valid.clone();
            bad["states"]["busy"]["steps"][0][key] = replacement;
            assert!(CardMotion::from_json(&bad).is_err(), "{key}");
        }
        let mut bad = valid.clone();
        bad["states"]["oops"]["then"] = serde_json::json!("cheer");
        assert!(
            CardMotion::from_json(&bad).is_err(),
            "tails cannot chain or cycle"
        );
        bad = valid.clone();
        bad["states"].as_object_mut().unwrap().remove("guard");
        assert!(CardMotion::from_json(&bad).is_err());
        bad = valid.clone();
        bad["states"]["sit"]["steps"] =
            serde_json::json!(vec![valid["states"]["sit"]["steps"][0].clone(); 2049]);
        assert!(CardMotion::from_json(&bad).is_err());
    }
    #[test]
    fn one_shot_tail_and_repeat_boundaries_use_authored_tracks() {
        let motion = CardMotion::from_json(&authored()).unwrap();
        let idle = [ClipStep {
            frame: 7,
            millis: 1000,
        }];
        let wave = [ClipStep {
            frame: 8,
            millis: 1000,
        }];
        assert_eq!(
            motion.sample("oops", 0.1, &idle, &wave).unwrap().anchor,
            Anchor::Stand
        );
        assert_eq!(motion.sample("oops", 0.701, &idle, &wave).unwrap().frame, 5);
        assert_eq!(
            motion.sample("cheer", 1.301, &idle, &wave).unwrap().anchor,
            Anchor::Seat
        );
        let first = motion.sample("busy", 0.55, &idle, &wave).unwrap();
        let repeated = motion.sample("busy", 20.35, &idle, &wave).unwrap();
        assert_eq!((first.frame, first.left), (repeated.frame, repeated.left));
        assert!((first.offset[0] - repeated.offset[0]).abs() < 0.001);
        assert_eq!(
            motion.sample("busy", f32::NAN, &idle, &wave),
            motion.sample("busy", 0., &idle, &wave)
        );
    }
    #[cfg(feature = "remote")]
    #[test]
    fn json_decode_reconstructs_prepared_sampling_cache() {
        let motion = CardMotion::from_json(&authored()).unwrap();
        let decoded: CardMotion =
            serde_json::from_value(serde_json::to_value(&motion).unwrap()).unwrap();
        assert_eq!(motion, decoded);
    }
}
