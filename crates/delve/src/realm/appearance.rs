//! Character presentation data. Rules carry the resolved appearance unchanged;
//! Glass selects a cached frame without evaluating feature code during paint.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Animation {
    pub frames: Vec<Vec<[u8; 4]>>,
    pub frame_ticks: u32,
    #[serde(default)]
    pub phase_offset: u32,
}
impl Animation {
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
