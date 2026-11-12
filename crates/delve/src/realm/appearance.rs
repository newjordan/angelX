//! Character presentation data. Rules carry the resolved appearance unchanged;
//! Glass selects a cached frame without evaluating feature code during paint.
use serde::{Deserialize, Serialize};

/// Authored clip policy; lowered into legacy uniform frames before publication.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub frame: usize,
    pub ticks: u32,
}

pub fn validate_steps(steps: &[Step], frames: usize) -> Result<u32, String> {
    if steps.is_empty()
        || steps.len() > 32
        || steps
            .iter()
            .any(|step| step.frame >= frames || step.ticks == 0 || step.ticks > 300)
    {
        return Err(
            "animation needs 1..32 steps, available art frames, and durations 1..300".into(),
        );
    }
    let gcd = |mut a: u32, mut b: u32| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let quantum = steps.iter().fold(0, |q, step| gcd(q, step.ticks));
    if steps.iter().map(|step| step.ticks / quantum).sum::<u32>() > 8 {
        return Err(
            "animation exceeds eight legacy playback frames after exact duration lowering".into(),
        );
    }
    Ok(quantum)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Animation {
    pub frames: Vec<Vec<[u8; 4]>>,
    pub frame_ticks: u32,
    #[serde(default)]
    pub phase_offset: u32,
}
impl Animation {
    /// Exact GCD lowering preserves both old wire layout and old player timing.
    /// Validate the complete expansion before cloning any pixel buffers.
    pub fn with_steps(&self, steps: &[Step]) -> Result<Self, String> {
        let frame_ticks = validate_steps(steps, self.frames.len())?;
        let mut frames = Vec::new();
        for step in steps {
            for _ in 0..step.ticks / frame_ticks {
                frames.push(self.frames[step.frame].clone());
            }
        }
        Ok(Self {
            frames,
            frame_ticks,
            phase_offset: 0,
        })
    }
    pub fn period(&self) -> u32 {
        self.frame_ticks
            .saturating_mul(self.frames.len() as u32)
            .max(1)
    }
    pub fn phase(&self, tick: u64) -> u32 {
        (tick % u64::from(self.period()) + u64::from(self.phase_offset))
            .rem_euclid(u64::from(self.period())) as u32
    }
    pub fn continue_from(&mut self, previous: &Self, tick: u64) {
        let period = u64::from(self.period());
        let phase = u64::from(previous.phase(tick)) * period / u64::from(previous.period());
        self.phase_offset = ((phase + period - tick % period) % period) as u32;
    }
    pub fn pixels(&self, tick: u64) -> Option<&[[u8; 4]]> {
        if self.frame_ticks == 0 {
            return None;
        }
        self.frames
            .get((self.phase(tick) / self.frame_ticks) as usize)
            .filter(|frame| frame.len() == 32 * 32)
            .map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_clip_lowering_is_exact_bounded_and_rejects_before_allocation() {
        let art = Animation {
            frames: vec![vec![[1; 4]; 1024], vec![[2; 4]; 1024]],
            frame_ticks: 6,
            phase_offset: 0,
        };
        let clip = art
            .with_steps(&[
                Step {
                    frame: 1,
                    ticks: 12,
                },
                Step { frame: 0, ticks: 3 },
            ])
            .unwrap();
        assert_eq!(
            (clip.frames.len(), clip.frame_ticks, clip.period()),
            (5, 3, 15)
        );
        for tick in 0..30 {
            assert_eq!(
                clip.pixels(tick).unwrap()[0],
                if tick % 15 < 12 { [2; 4] } else { [1; 4] }
            );
        }
        for steps in [
            vec![],
            vec![Step { frame: 2, ticks: 1 }],
            vec![Step { frame: 0, ticks: 0 }],
            vec![Step {
                frame: 0,
                ticks: 301,
            }],
            vec![
                Step { frame: 0, ticks: 7 },
                Step {
                    frame: 1,
                    ticks: 11,
                },
            ],
            vec![Step { frame: 0, ticks: 1 }; 33],
        ] {
            assert!(art.with_steps(&steps).is_err());
        }
        assert_eq!(art.frames.len(), 2);
    }
    #[test]
    fn compatible_animation_swap_preserves_normalized_playback_phase() {
        let old = Animation {
            frames: vec![vec![[1; 4]; 1024]; 4],
            frame_ticks: 5,
            phase_offset: 0,
        };
        let mut next = Animation {
            frames: vec![vec![[2; 4]; 1024]; 8],
            frame_ticks: 5,
            phase_offset: 0,
        };
        next.continue_from(&old, 27);
        assert_eq!(old.phase(27), 7);
        assert_eq!(next.phase(27), 14);
        assert_eq!(next.pixels(27).unwrap()[0], [2; 4]);
    }
}
