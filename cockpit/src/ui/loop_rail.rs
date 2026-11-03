//! A read-only, glass-only toy on the window's outer chrome. No hit targets.
#![cfg_attr(not(feature = "glass"), allow(dead_code))]

#[path = "loop_rail/art.rs"]
pub(crate) mod art;

use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Pose {
    pub x: f32,
    pub y: f32,
    pub heading: f32,
    pub bank: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct Path {
    pub width: f32,
    pub height: f32,
    pub inset: f32,
    pub radius: f32,
}

impl Path {
    pub(crate) fn length(self) -> f32 {
        let w = (self.width - 2.0 * self.inset).max(0.0);
        let h = (self.height - 2.0 * self.inset).max(0.0);
        2.0 * (w + h - 4.0 * self.radius) + TAU * self.radius
    }

    pub(crate) fn pace(self, turn: f64) -> f64 {
        let ahead = self.at(turn + 52.0 / self.length() as f64);
        let behind = self.at(turn - 40.0 / self.length() as f64);
        let bend = (ahead.heading - behind.heading + PI).rem_euclid(TAU) - PI;
        1.0 / (1.0 + bend.abs() as f64 * 9.0)
    }

    pub(crate) fn at(self, turn: f64) -> Pose {
        let r = self.radius;
        let (left, top) = (self.inset, self.inset);
        let (right, bottom) = (self.width - left, self.height - top);
        let (w, h) = (right - left - 2.0 * r, bottom - top - 2.0 * r);
        let arc = PI * r / 2.0;
        let mut d = ((turn * self.length() as f64 + w as f64 * 0.5)
            .rem_euclid(self.length() as f64)) as f32;
        // Start midway across the top edge; clockwise in window coordinates.
        for (side, straight) in [w, h, w, h].into_iter().enumerate() {
            let heading = side as f32 * PI / 2.0;
            if d < straight {
                let (x, y) = match side {
                    0 => (left + r + d, top),
                    1 => (right, top + r + d),
                    2 => (right - r - d, bottom),
                    _ => (left, bottom - r - d),
                };
                return Pose {
                    x,
                    y,
                    heading,
                    bank: 0.0,
                };
            }
            d -= straight;
            if d < arc {
                let u = d / arc;
                let a = heading - PI / 2.0 + u * PI / 2.0;
                let (cx, cy) = match side {
                    0 => (right - r, top + r),
                    1 => (right - r, bottom - r),
                    2 => (left + r, bottom - r),
                    _ => (left + r, top + r),
                };
                return Pose {
                    x: cx + r * a.cos(),
                    y: cy + r * a.sin(),
                    heading: heading + u * PI / 2.0,
                    bank: (u * PI).sin(),
                };
            }
            d -= arc;
        }
        Pose {
            x: left + r,
            y: top,
            heading: 0.0,
            bank: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Activity {
    Working,
    Waiting,
    Operator,
    Finished,
}

impl Activity {
    fn speed(self) -> f32 {
        match self {
            Self::Working => 1.0,
            Self::Waiting => 0.32,
            Self::Operator => 0.08,
            Self::Finished => 0.0,
        }
    }

    pub(crate) fn bob(self, t: f32, phase: f32) -> f32 {
        let speed = self.speed();
        (t * (2.5 + speed * 5.5) + phase).sin() * (0.15 + speed * 0.65)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Snapshot {
    pub run: u64,
    pub done: usize,
    pub activity: Activity,
}

#[derive(Default)]
pub(crate) struct State {
    pub turns: f64,
    pub laps: usize,
    pub speed: f32,
    run: Option<u64>,
    last: f64,
    finished: Option<f64>,
    finish_speed: f32,
}

impl State {
    #[cfg(test)]
    pub(crate) fn step(&mut self, snapshot: Option<Snapshot>, now: f64) -> bool {
        self.step_paced(snapshot, now, 1.0)
    }

    pub(crate) fn step_paced(&mut self, snapshot: Option<Snapshot>, now: f64, pace: f64) -> bool {
        if let Some(s) = snapshot {
            if self.run.is_none() && s.activity == Activity::Finished {
                return false;
            }
            if self.run != Some(s.run) {
                *self = Self {
                    run: Some(s.run),
                    turns: s.done as f64,
                    laps: s.done,
                    last: now,
                    ..Self::default()
                };
            }
            self.laps = s.done;
            if s.activity == Activity::Finished && self.finished.is_none() {
                self.finish_speed = self.speed;
                self.finished = Some(now);
            }
        } else if self.run.is_some() {
            if self.finished.is_none() {
                self.finish_speed = self.speed;
                self.finished = Some(now);
            }
        } else {
            return false;
        }
        let dt = (now - self.last).clamp(0.0, 0.25);
        self.last = now;
        let finishing = self.finished.map(|at| (now - at).max(0.0));
        if finishing.is_some_and(|age| age >= 6.0) {
            self.speed = 0.0;
            return false;
        }
        let activity = snapshot.map_or(Activity::Finished, |s| s.activity);
        let target = finishing.map_or(activity.speed(), |age| {
            (1.0 - age as f32 / 3.5).max(0.0) * self.finish_speed
        });
        self.speed += (target - self.speed) * (1.0 - (-dt as f32 * 5.0).exp());
        // One border circuit is one observed iteration. In an unusually long
        // iteration the racer idles short of the line, rather than inventing laps.
        let ceiling = self.laps as f64 + 0.96;
        let room = (ceiling - self.turns).max(0.0);
        self.turns += (dt * self.speed as f64 * 0.13 * pace).min(room * (1.0 - (-dt * 1.5).exp()));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_rail_display_gates_terminal_small_disabled_and_calm() {
        assert!(Display::from_setting(None).enabled);
        assert!(Display::from_setting(Some("on")).enabled);
        for v in ["0", "off", "FALSE", " no "] {
            assert!(!Display::from_setting(Some(v)).enabled);
        }
        let on = Display { enabled: true };
        assert!(on.allows(true, true, 150, 44));
        assert!(!on.allows(false, true, 150, 44));
        assert!(!on.allows(true, false, 150, 44));
        assert!(!on.allows(true, true, 30, 10));
        assert!(!Display { enabled: false }.allows(true, true, 150, 44));
        let mut s = State::default();
        assert!(
            !s.step(snap(12, Activity::Finished), 20.0),
            "restored completed loops stay asleep"
        );
    }

    fn path() -> Path {
        Path {
            width: 1500.0,
            height: 880.0,
            inset: 5.0,
            radius: 24.0,
        }
    }
    fn snap(done: usize, activity: Activity) -> Option<Snapshot> {
        Some(Snapshot {
            run: 1,
            done,
            activity,
        })
    }

    #[test]
    fn loop_rail_path_is_clockwise_continuous_and_closes() {
        let p = path();
        let mut distance = 0.0;
        let mut prev = p.at(0.0);
        for i in 1..=10000 {
            let next = p.at(i as f64 / 10000.0);
            let d = (next.x - prev.x).hypot(next.y - prev.y);
            assert!(d < 1.0, "path jump at {i}: {d}");
            distance += d;
            prev = next;
        }
        assert!(distance > 4600.0);
        assert!((prev.x - p.at(0.0).x).abs() < 0.01);
        assert!((prev.y - p.at(0.0).y).abs() < 0.01);
        assert!(p.at(0.05).x > p.at(0.0).x);
        assert!(p.at(0.35).y > p.at(0.30).y);
    }

    #[test]
    fn loop_rail_corner_pacing_leaves_time_to_bank() {
        let p = path();
        assert_eq!(p.pace(0.0), 1.0);
        let mut slow = 0;
        for i in 0..1000 {
            let turn = i as f64 / 1000.0;
            if p.at(turn).bank > 0.1 {
                assert!(p.pace(turn) < 0.12);
                slow += 1;
            }
        }
        assert!(slow > 10);
    }

    #[test]
    fn loop_rail_corners_bank_with_a_continuous_tangent() {
        let p = path();
        let mut banked = 0;
        let mut prev = p.at(0.0);
        for i in 1..=20000 {
            let next = p.at(i as f64 / 20000.0);
            let delta = (next.heading - prev.heading + PI).rem_euclid(TAU) - PI;
            assert!(delta.abs() < 0.04, "heading snapped at {i}");
            if next.bank > 0.1 {
                banked += 1;
            }
            prev = next;
        }
        assert!(banked > 200, "all four corners have a bank");
    }

    #[test]
    fn loop_rail_activity_controls_speed_and_out_of_phase_bob() {
        assert!(Activity::Working.speed() > Activity::Waiting.speed());
        assert!(Activity::Waiting.speed() > Activity::Operator.speed());
        let amplitude = |a: Activity| {
            (0..1000)
                .map(|i| a.bob(i as f32 / 100.0, 0.0).abs())
                .fold(0.0f32, f32::max)
        };
        assert!(amplitude(Activity::Working) > amplitude(Activity::Operator));
        assert_ne!(
            Activity::Working.bob(0.12, 0.0),
            Activity::Working.bob(0.12, 1.7)
        );
    }

    #[test]
    fn loop_rail_counts_only_observed_iterations_and_resets_for_a_new_run() {
        let mut s = State::default();
        assert!(s.step(snap(0, Activity::Working), 0.0));
        for i in 1..2000 {
            s.step(snap(0, Activity::Working), i as f64 / 10.0);
        }
        assert_eq!(s.laps, 0);
        assert!(
            s.turns < 1.0,
            "waiting on iteration one must not invent laps"
        );
        s.step(snap(7, Activity::Working), 200.0);
        assert_eq!(s.laps, 7);
        s.step(snap(7, Activity::Working), 200.1);
        assert_eq!(s.laps, 7);
        s.step(
            Some(Snapshot {
                run: 2,
                done: 0,
                activity: Activity::Waiting,
            }),
            201.0,
        );
        assert_eq!(s.laps, 0);
        assert_eq!(s.turns, 0.0);
    }

    #[test]
    fn loop_rail_finish_never_accelerates_out_of_a_pit_stop() {
        let mut s = State::default();
        for i in 0..60 {
            s.step(snap(2, Activity::Operator), i as f64 / 30.0);
        }
        let pit_speed = s.speed;
        s.step(snap(2, Activity::Finished), 2.0);
        assert!(s.speed <= pit_speed);
        for i in 61..150 {
            let before = s.speed;
            s.step(snap(2, Activity::Finished), i as f64 / 30.0);
            assert!(s.speed <= before);
        }
    }

    #[test]
    fn loop_rail_off_without_a_loop_and_chequers_expire_after_slowdown() {
        let mut s = State::default();
        assert!(!s.step(None, 0.0));
        s.step(snap(1, Activity::Working), 1.0);
        s.step(snap(1, Activity::Working), 2.0);
        let fast = s.speed;
        assert!(s.step(snap(1, Activity::Finished), 2.1));
        s.step(snap(1, Activity::Finished), 5.0);
        assert!(s.speed < fast);
        assert!(!s.step(snap(1, Activity::Finished), 9.0));
        assert!(!s.step(None, 10.0));
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Display {
    pub enabled: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self::from_setting(std::env::var("ANGEL_LOOP_RAIL").ok().as_deref())
    }
}

impl Display {
    fn from_setting(value: Option<&str>) -> Self {
        Self {
            enabled: !value.is_some_and(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "0" | "off" | "false" | "no"
                )
            }),
        }
    }

    pub(crate) fn allows(self, glass: bool, full_motion: bool, width: u16, height: u16) -> bool {
        self.enabled && glass && full_motion && width >= 48 && height >= 12
    }
}
