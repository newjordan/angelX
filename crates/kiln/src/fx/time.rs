//! An effect's clock: eases over `0..=1`, the beats an effect is timed in
//! (anticipation, action, follow-through, settle) and the moments a layer
//! starts and ends at, in frames or at a beat.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Frames a second unless a recipe says otherwise.
pub const FPS: u32 = 12;

/// An easing curve over `0..=1`. Each takes any number and clamps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    /// Even all the way.
    Linear,
    /// Slow away, fast in: what is drawn in toward a point.
    In,
    /// Fast away, gentle to rest: the shape of most releases.
    #[default]
    Out,
    /// Slow at both ends.
    Both,
    /// Past the mark and back: an opening that overshoots and settles.
    Back,
    /// A snap that rings once and stills.
    Spring,
    /// A fall that bounces to rest.
    Bounce,
}

impl Ease {
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::In => t * t * t,
            Ease::Out => {
                let u = 1.0 - t;
                1.0 - u * u * u
            }
            Ease::Both => t * t * (3.0 - 2.0 * t),
            Ease::Back => {
                let (u, s) = (t - 1.0, 1.9);
                1.0 + u * u * ((s + 1.0) * u + s)
            }
            Ease::Spring => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - (-6.0 * t).exp() * (t * 11.0).cos()
                }
            }
            Ease::Bounce => {
                let (n, d) = (7.5625, 2.75);
                if t < 1.0 / d {
                    n * t * t
                } else if t < 2.0 / d {
                    let t = t - 1.5 / d;
                    n * t * t + 0.75
                } else if t < 2.5 / d {
                    let t = t - 2.25 / d;
                    n * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d;
                    n * t * t + 0.984375
                }
            }
        }
    }

    /// Whether it goes past its mark: a clock run through it overshoots.
    pub fn overshoots(self) -> bool {
        matches!(self, Ease::Back | Ease::Spring | Ease::Bounce)
    }
}

/// Up and down again: zero at both ends, one in the middle.
pub fn bump(t: f32) -> f32 {
    (t.clamp(0.0, 1.0) * PI).sin()
}

/// An effect's beat sheet, in frames: the anticipation that gathers, the
/// action that throws, the follow-through that carries on past it, and the
/// settle as it comes to rest. Layers start and end at the beats by name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Beats {
    #[serde(default)]
    pub anticipation: f32,
    #[serde(default)]
    pub action: f32,
    #[serde(default)]
    pub follow: f32,
    #[serde(default)]
    pub settle: f32,
}

impl Beats {
    pub fn total(&self) -> f32 {
        self.anticipation + self.action + self.follow + self.settle
    }

    /// The frame a beat begins at; `end` is the last beat's end.
    pub fn mark(&self, name: &str) -> Option<f32> {
        Some(match name {
            "anticipation" | "start" => 0.0,
            "action" => self.anticipation,
            "follow" => self.anticipation + self.action,
            "settle" => self.anticipation + self.action + self.follow,
            "end" => self.total(),
            _ => return None,
        })
    }
}

/// A moment: a frame, or a beat's mark with frames added or taken
/// (`"action"`, `"follow+2"`, `"end-1"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Time {
    Frame(f32),
    Mark(String),
}

impl Time {
    /// The frame it names. `end` is the effect's last frame plus one when no
    /// beats are given.
    pub fn resolve(&self, beats: &Beats, end: f32) -> Result<f32, String> {
        match self {
            Time::Frame(f) => Ok(*f),
            Time::Mark(s) => {
                let s = s.trim();
                let (name, add) = match s.find(['+', '-']) {
                    Some(i) => {
                        let n: f32 = s[i + 1..].trim().parse().map_err(|_| format!("{s:?}: a beat, then + or - and a number of frames"))?;
                        (s[..i].trim(), if &s[i..i + 1] == "-" { -n } else { n })
                    }
                    None => (s, 0.0),
                };
                let at = if name == "end" && beats.total() <= 0.0 { Some(end) } else { beats.mark(name) };
                at.map(|f| f + add).ok_or_else(|| format!("{name:?} is no beat: anticipation action follow settle end"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_start_at_zero_and_end_at_one() {
        for e in [Ease::Linear, Ease::In, Ease::Out, Ease::Both, Ease::Back, Ease::Spring, Ease::Bounce] {
            assert!(e.at(0.0).abs() < 1e-4, "{e:?}");
            assert!((e.at(1.0) - 1.0).abs() < 1e-4, "{e:?}");
        }
        assert!(Ease::Back.at(0.7) > 1.0);
        assert!(Ease::Out.at(0.3) > Ease::In.at(0.3));
    }

    #[test]
    fn a_moment_is_a_frame_or_a_beat() {
        let b = Beats { anticipation: 3.0, action: 2.0, follow: 4.0, settle: 3.0 };
        assert_eq!(Time::Frame(5.0).resolve(&b, 0.0), Ok(5.0));
        assert_eq!(Time::Mark("action".into()).resolve(&b, 0.0), Ok(3.0));
        assert_eq!(Time::Mark("follow+2".into()).resolve(&b, 0.0), Ok(7.0));
        assert_eq!(Time::Mark("end-1".into()).resolve(&b, 0.0), Ok(11.0));
        assert_eq!(Time::Mark("end".into()).resolve(&Beats::default(), 9.0), Ok(9.0));
        assert!(Time::Mark("windup".into()).resolve(&b, 0.0).is_err());
    }
}
