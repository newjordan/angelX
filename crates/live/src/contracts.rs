//! Cached policy contracts. JavaScript builds these once per accepted edit;
//! the render and simulation loops only read native values.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiBand {
    pub min_width: u16,
    pub transcript_width: u16,
    pub side_width: u16,
    pub idle_agent_height: u16,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiPolicy {
    pub bands: Vec<UiBand>,
    pub show_agent: bool,
    pub show_artifacts: bool,
    pub agent_first: bool,
    pub reasoning_percent: u16,
    pub idle_placeholder: String,
    pub busy_placeholder: String,
}

impl Default for UiPolicy {
    fn default() -> Self {
        Self {
            bands: vec![
                UiBand {
                    min_width: 0,
                    transcript_width: 64,
                    side_width: 48,
                    idle_agent_height: 13,
                },
                UiBand {
                    min_width: 140,
                    transcript_width: 80,
                    side_width: 64,
                    idle_agent_height: 13,
                },
                UiBand {
                    min_width: 150,
                    transcript_width: 80,
                    side_width: 64,
                    idle_agent_height: 14,
                },
                UiBand {
                    min_width: 180,
                    transcript_width: 105,
                    side_width: 80,
                    idle_agent_height: 14,
                },
            ],
            show_agent: true,
            show_artifacts: true,
            agent_first: true,
            reasoning_percent: 80,
            idle_placeholder: "Write a message…".into(),
            busy_placeholder: "Add guidance… Enter queues it".into(),
        }
    }
}

impl UiPolicy {
    pub fn band(&self, width: u16) -> &UiBand {
        self.bands
            .iter()
            .rev()
            .find(|band| width >= band.min_width)
            .unwrap_or(&self.bands[0])
    }

    pub fn side_width(&self, width: u16) -> u16 {
        let band = self.band(width);
        band.side_width.min(
            width
                .saturating_sub(band.transcript_width)
                .saturating_add(1),
        )
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.bands.is_empty()
            || self.bands.len() > 16
            || self.bands[0].min_width != 0
            || self
                .bands
                .windows(2)
                .any(|pair| pair[0].min_width >= pair[1].min_width)
            || self.bands.iter().any(|band| {
                band.transcript_width < 24
                    || band.side_width > 500
                    || !(3..=100).contains(&band.idle_agent_height)
            })
            || !(10..=95).contains(&self.reasoning_percent)
        {
            return Err("ui requires ordered width bands from 0, valid pane sizes and reasoning_percent 10–95".into());
        }
        for text in [&self.idle_placeholder, &self.busy_placeholder] {
            if text.len() > 256 || text.chars().any(char::is_control) {
                return Err("ui placeholders must be single-line text up to 256 bytes".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GamePolicy {
    /// An overlay, not a rewrite of the workspace's saved tuning. Omitted
    /// names inherit the host's normal tuning again on the next generation.
    pub knobs: BTreeMap<String, f64>,
}

impl GamePolicy {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.knobs.len() > 128
            || self.knobs.iter().any(|(name, value)| {
                name.is_empty()
                    || name.len() > 32
                    || !value.is_finite()
                    || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            })
        {
            return Err("game knobs must be finite, named values (at most 128)".into());
        }
        Ok(())
    }
}
