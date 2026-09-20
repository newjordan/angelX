//! Pure progressive-wave decision logic.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopReason {
    MaxWidth,
    MaxWaves,
    Failed,
    LowNovelty,
    Agree,
    Stable,
}

impl StopReason {
    pub(crate) fn label(self) -> &'static str {
        match self {
            StopReason::MaxWidth => "max-width",
            StopReason::MaxWaves => "max-waves",
            StopReason::Failed => "failed",
            StopReason::LowNovelty => "low-novelty",
            StopReason::Agree => "agree",
            StopReason::Stable => "stable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Decision {
    Stop(StopReason),
    Expand { next_wave: usize },
}

impl Decision {
    pub(crate) fn label(self) -> String {
        match self {
            Decision::Stop(reason) => format!("stop:{}", reason.label()),
            Decision::Expand { next_wave } => format!("expand:{next_wave}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WaveInputs {
    pub(crate) wave_index: usize,
    pub(crate) max_waves: usize,
    pub(crate) launched_total: usize,
    pub(crate) max_width: usize,
    pub(crate) returned: usize,
    pub(crate) novel: usize,
    pub(crate) wave_size: usize,
    pub(crate) dissent: Option<f64>,
    pub(crate) prev_dissent: Option<f64>,
    pub(crate) novelty_floor: f64,
    pub(crate) dissent_lo: f64,
    pub(crate) dissent_epsilon: f64,
    pub(crate) growth: f64,
}

pub(crate) fn decide_wave(i: WaveInputs) -> Decision {
    if i.launched_total >= i.max_width {
        return Decision::Stop(StopReason::MaxWidth);
    }
    if i.wave_index + 1 >= i.max_waves {
        return Decision::Stop(StopReason::MaxWaves);
    }
    if i.returned == 0 {
        return Decision::Stop(StopReason::Failed);
    }
    let novelty = i.novel as f64 / i.returned.max(1) as f64;
    if novelty < i.novelty_floor {
        return Decision::Stop(StopReason::LowNovelty);
    }
    if matches!(i.dissent, Some(d) if d <= i.dissent_lo) {
        return Decision::Stop(StopReason::Agree);
    }
    if let (Some(prev), Some(now)) = (i.prev_dissent, i.dissent)
        && (now - prev).abs() <= i.dissent_epsilon
    {
        return Decision::Stop(StopReason::Stable);
    }
    Decision::Expand {
        next_wave: next_wave_size(i.wave_size, i.growth, i.max_width - i.launched_total),
    }
}

pub(crate) fn next_wave_size(current: usize, growth: f64, remaining: usize) -> usize {
    if remaining == 0 {
        return 0;
    }
    let growth = if growth.is_finite() && growth > 1.0 {
        growth
    } else {
        1.0
    };
    let next = ((current.max(1) as f64) * growth).ceil() as usize;
    next.max(1).min(remaining)
}
