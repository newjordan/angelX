//! Clocks on the client: a fixed step for sampling commands, and the
//! server's time estimated from when its snapshots arrive, so everyone else
//! can be drawn a fixed delay in the past between two snapshots.

pub const INTERP_MS: f64 = 50.0;
pub const CORRECT_MS: f64 = 100.0;
const WINDOW: usize = 90;
const MAX_CATCH_UP: u32 = 8;

pub struct FixedStep {
    step_ms: f64,
    next_ms: Option<f64>,
}

impl FixedStep {
    pub fn new(hz: u32) -> FixedStep {
        FixedStep { step_ms: 1000.0 / f64::from(hz.max(1)), next_ms: None }
    }

    pub fn step_ms(&self) -> f64 {
        self.step_ms
    }

    pub fn restart(&mut self, now_ms: f64) {
        self.next_ms = Some(now_ms + self.step_ms);
    }

    pub fn due(&mut self, now_ms: f64) -> u32 {
        let next = *self.next_ms.get_or_insert(now_ms);
        if now_ms < next {
            return 0;
        }
        let behind = ((now_ms - next) / self.step_ms).floor() as u64 + 1;
        if behind > u64::from(MAX_CATCH_UP) {
            self.next_ms = Some(now_ms + self.step_ms);
            return MAX_CATCH_UP;
        }
        self.next_ms = Some(next + behind as f64 * self.step_ms);
        behind as u32
    }
}

pub struct ServerClock {
    step_ms: f64,
    samples: [f64; WINDOW],
    filled: usize,
    next: usize,
    offset: Option<f64>,
}

impl ServerClock {
    pub fn new(tick_hz: u32) -> ServerClock {
        ServerClock {
            step_ms: 1000.0 / f64::from(tick_hz.max(1)),
            samples: [0.0; WINDOW],
            filled: 0,
            next: 0,
            offset: None,
        }
    }

    pub fn arrived(&mut self, tick: u32, now_ms: f64) {
        let sample = now_ms - f64::from(tick) * self.step_ms;
        self.samples[self.next] = sample;
        self.next = (self.next + 1) % WINDOW;
        self.filled = (self.filled + 1).min(WINDOW);
        let least = self.samples[..self.filled].iter().copied().fold(f64::INFINITY, f64::min);
        self.offset = Some(least);
    }

    pub fn reset(&mut self) {
        self.filled = 0;
        self.next = 0;
        self.offset = None;
    }

    pub fn tick_at(&self, now_ms: f64) -> Option<f64> {
        self.offset.map(|offset| (now_ms - offset) / self.step_ms)
    }

    pub fn render_tick(&self, now_ms: f64) -> Option<f64> {
        self.render_tick_behind(now_ms, INTERP_MS)
    }

    pub fn render_tick_behind(&self, now_ms: f64, behind_ms: f64) -> Option<f64> {
        self.tick_at(now_ms - behind_ms)
    }
}

pub fn bracket(ticks: &[u32], at: f64) -> Option<(usize, usize, f32)> {
    let last = ticks.len().checked_sub(1)?;
    if at <= f64::from(ticks[0]) {
        return Some((0, 0, 0.0));
    }
    if at >= f64::from(ticks[last]) {
        return Some((last, last, 0.0));
    }
    let after = ticks.partition_point(|&t| f64::from(t) <= at);
    let before = after - 1;
    let (a, b) = (f64::from(ticks[before]), f64::from(ticks[after]));
    Some((before, after, ((at - a) / (b - a)) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_step_counts_ticks_and_drops_a_long_sleep() {
        let mut step = FixedStep::new(30);
        assert_eq!(step.due(0.0), 1);
        assert_eq!(step.due(10.0), 0);
        assert_eq!(step.due(34.0), 1);
        assert_eq!(step.due(101.0), 2);
        assert_eq!(step.due(10_000.0), MAX_CATCH_UP);
        assert_eq!(step.due(10_001.0), 0);
        assert_eq!(step.due(10_034.0), 1);
    }

    #[test]
    fn the_server_clock_takes_the_least_delayed_arrival() {
        let mut clock = ServerClock::new(30);
        let step = 1000.0 / 30.0;
        for (tick, jitter) in [(30u32, 60.0), (31, 0.0), (32, 25.0), (33, 5.0)] {
            clock.arrived(tick, 5000.0 + f64::from(tick) * step + 40.0 + jitter);
        }
        let now = 5000.0 + 40.0 + 40.0 * step;
        let at = clock.tick_at(now).unwrap();
        assert!((at - 40.0).abs() < 1e-6, "{at}");
        let behind = clock.render_tick(now).unwrap();
        assert!((at - behind - INTERP_MS / step).abs() < 1e-6);
    }

    #[test]
    fn bracket_finds_the_two_ticks_around_a_time() {
        let ticks = [10, 11, 13];
        assert_eq!(bracket(&ticks, 9.0), Some((0, 0, 0.0)));
        assert_eq!(bracket(&ticks, 10.5), Some((0, 1, 0.5)));
        assert_eq!(bracket(&ticks, 12.0), Some((1, 2, 0.5)));
        assert_eq!(bracket(&ticks, 20.0), Some((2, 2, 0.0)));
        assert_eq!(bracket(&[], 1.0), None);
    }
}
