//! A foreground paper surface, independent of game stages. Fixed-step motion
//! drives released paper length; the rollers turn by that length / radius.
use ratatui_core::{buffer::Buffer, layout::Rect};
use std::time::Instant;

const STEP: f32 = 1.0 / 120.0;
pub const RADIUS: f32 = 0.036;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub open: f32,
    pub sway: f32,
}
impl Pose {
    pub fn roller_angle(self) -> f32 {
        released(self) / RADIUS
    }
}

#[derive(Debug)]
pub struct Motion {
    position: f32,
    velocity: f32,
    sway: f32,
    angular_velocity: f32,
    target: f32,
    remainder: f32,
    last: Instant,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            position: 0.0,
            velocity: 0.0,
            sway: 0.0,
            angular_velocity: 0.0,
            target: 0.0,
            remainder: 0.0,
            last: Instant::now(),
        }
    }
}
impl Motion {
    pub fn target(&mut self, open: bool, now: Instant) {
        self.advance(now);
        self.target = if open { 1.0 } else { 0.0 };
    }
    pub fn snap(&mut self, open: bool, now: Instant) {
        self.target = if open { 1.0 } else { 0.0 };
        self.position = self.target;
        self.velocity = 0.0;
        self.sway = 0.0;
        self.angular_velocity = 0.0;
        self.remainder = 0.0;
        self.last = now;
    }
    pub fn advance(&mut self, now: Instant) -> Pose {
        let dt = now.saturating_duration_since(self.last).as_secs_f32();
        self.last = now;
        self.step(dt);
        self.pose()
    }
    pub fn pose(&self) -> Pose {
        Pose {
            open: self.position,
            sway: self.sway,
        }
    }
    pub fn hidden(&self) -> bool {
        self.target == 0.0 && self.position == 0.0
    }
    pub fn moving(&self) -> bool {
        (self.position - self.target).abs() > 0.0001
            || self.velocity.abs() > 0.0001
            || self.sway.abs() > 0.0001
            || self.angular_velocity.abs() > 0.0001
    }
    fn step(&mut self, dt: f32) {
        // Stall recovery is capped: at most 30 substeps, never a giant Euler step.
        if !dt.is_finite() || dt < 0.0 {
            return;
        }
        self.remainder += dt.min(0.25);
        while self.remainder + 1e-7 >= STEP {
            self.remainder = (self.remainder - STEP).max(0.0);
            let force = 155.0 * (self.target - self.position) - 21.0 * self.velocity;
            self.velocity += force * STEP;
            self.position = (self.position + self.velocity * STEP).clamp(0.0, 1.025);
            if self.position == 0.0 && self.velocity < 0.0 {
                self.velocity = 0.0;
            }
            // Roller inertia gives a small coupled pendular response, not idle noise.
            self.angular_velocity +=
                (-105.0 * self.sway - 17.0 * self.angular_velocity + self.velocity * 0.65) * STEP;
            self.sway = (self.sway + self.angular_velocity * STEP).clamp(-0.018, 0.018);
            if (self.position - self.target).abs() < 0.0003
                && self.velocity.abs() < 0.003
                && self.sway.abs() < 0.0003
                && self.angular_velocity.abs() < 0.003
            {
                self.position = self.target;
                self.velocity = 0.0;
                self.sway = 0.0;
                self.angular_velocity = 0.0;
            }
        }
    }
}

/// Text remains native atlas glyphs. The page is a bounded separate cell grid,
/// published atomically with the cockpit frame; it is never a screenshot.
#[derive(Clone, Debug)]
pub struct Page {
    pub area: Rect,
    pub buffer: Buffer,
    pub pose: Pose,
    pub interactive: bool,
}
impl Page {
    pub fn new(area: Rect, buffer: Buffer, pose: Pose) -> Option<Self> {
        (area.width > 0
            && area.height > 0
            && area.width <= 160
            && area.height <= 64
            && buffer.area == Rect::new(0, 0, area.width, area.height)
            && pose.open.is_finite()
            && pose.sway.is_finite())
        .then_some(Self {
            area,
            buffer,
            interactive: true,
            pose: Pose {
                open: pose.open.clamp(0.0, 1.025),
                sway: pose.sway.clamp(-0.018, 0.018),
            },
        })
    }
    /// Convert displayed cells back to the uncurled page. During the short
    /// transition clicks are ignored, so hidden rows cannot activate actions.
    pub fn hit(&self, col: u16, row: u16) -> Option<(u16, u16)> {
        if !self.interactive || self.pose.open < 0.995 || self.pose.sway.abs() > 0.001 {
            return None;
        }
        let v = (f32::from(row) + 0.5 - f32::from(self.area.y)) / f32::from(self.area.height);
        let s = inverse_y(self.pose, v)?;
        let [_, z, _] = surface(self.pose, s);
        let scale = 1.0 / (1.0 - z * 0.65);
        let x = ((f32::from(col) + 0.5 - f32::from(self.area.x)) / f32::from(self.area.width)
            - 0.5)
            / scale
            + 0.5;
        if !(0.0..1.0).contains(&x) {
            return None;
        }
        Some((
            self.area.x + (x * f32::from(self.area.width)) as u16,
            self.area.y + (s * f32::from(self.area.height)) as u16,
        ))
    }
}

/// Rows of a fully open page `height` rows tall that lie on its flat middle:
/// the material past `(1 - 2 top) - edge` curls under the lower roller
/// (`surface`), so anything a reader needs (the key bar) ends above it.
pub fn readable_rows(height: u16) -> u16 {
    let top = RADIUS * 1.15;
    let edge = RADIUS * 1.35;
    (((1.0 - 2.0 * top) - edge) * f32::from(height)).floor() as u16
}

/// Material coordinate s -> projected y, depth toward viewer, and light.
/// The paper keeps its arc length around each cylinder: y=R sin(a), z=R(1-cos(a)).
/// The flat middle is not scaled to fake unrolling: unreleased rows are hidden.
pub fn surface(pose: Pose, s: f32) -> [f32; 3] {
    let open = pose.open.clamp(0.0, 1.025);
    let top = RADIUS * 1.15;
    let travel = (open * (1.0 - 2.0 * top)).max(0.002);
    let edge = RADIUS * 1.35;
    let d = s.min(travel);
    let (y, z, normal) = if d < edge {
        let a = (1.0 - d / edge) * 0.95;
        (
            top + edge - RADIUS * (a.sin() / 0.95) * 1.35,
            RADIUS * (1.0 - a.cos()),
            a.sin(),
        )
    } else if d > travel - edge {
        let a = ((d - (travel - edge)) / edge).clamp(0.0, 1.0) * 1.25;
        (
            top + travel - edge + RADIUS * a.sin() * 1.35 / 1.25,
            RADIUS * (1.0 - a.cos()),
            -a.sin(),
        )
    } else {
        (top + d, 0.0, 0.0)
    };
    let lift = (1.0 - open.min(1.0)).powi(2) * 0.10;
    [
        y - lift + pose.sway * d,
        z,
        (0.91 + normal * 0.17).clamp(0.62, 1.05),
    ]
}

pub fn released(pose: Pose) -> f32 {
    (pose.open * (1.0 - 2.0 * RADIUS * 1.15)).clamp(0.0, 1.0)
}
fn inverse_y(pose: Pose, y: f32) -> Option<f32> {
    let hi = released(pose);
    if y < surface(pose, 0.0)[0] || y >= surface(pose, hi)[0] {
        return None;
    }
    let (mut lo, mut hi) = (0.0, hi);
    for _ in 0..18 {
        let mid = (lo + hi) * 0.5;
        if surface(pose, mid)[0] < y {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo + hi) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scroll_physics_is_bounded_reversible_and_eventually_quiet() {
        let mut m = Motion::default();
        m.target = 1.0;
        for _ in 0..22 {
            m.step(1.0 / 120.0);
        }
        let before = m.pose();
        m.target = 0.0;
        assert_eq!(
            before,
            m.pose(),
            "reversal must not reset position or velocity"
        );
        for _ in 0..20 {
            m.step(0.8);
            assert!((0.0..=1.025).contains(&m.pose().open));
        }
        assert!(m.hidden());
        m.target = 1.0;
        for _ in 0..300 {
            m.step(1.0 / 120.0);
        }
        assert_eq!(
            m.pose(),
            Pose {
                open: 1.0,
                sway: 0.0
            }
        );
        assert!(!m.moving());
        assert!((m.pose().roller_angle() * RADIUS - released(m.pose())).abs() < 1e-6);
    }
    #[test]
    fn scroll_projection_is_monotone_and_hit_mapping_round_trips() {
        let pose = Pose {
            open: 1.0,
            sway: 0.0,
        };
        let mut last = -1.0;
        for i in 0..100 {
            let s = released(pose) * i as f32 / 100.0;
            let [y, z, light] = surface(pose, s);
            assert!(y > last && z >= 0.0 && light.is_finite());
            assert!((inverse_y(pose, y).unwrap() - s).abs() < 0.0001);
            last = y;
        }
        let area = Rect::new(10, 5, 90, 28);
        let page = Page::new(area, Buffer::empty(Rect::new(0, 0, 90, 28)), pose).unwrap();
        let hit = page.hit(55, 18).unwrap();
        assert_eq!(hit.0, 55);
        assert!(hit.1 >= area.y && hit.1 < area.bottom());
        assert!(page.hit(0, 0).is_none());
        let moving = Page {
            pose: Pose {
                open: 0.4,
                sway: 0.0,
            },
            ..page
        };
        assert!(moving.hit(55, 18).is_none());
    }
    #[test]
    fn scroll_fixed_steps_do_not_depend_on_display_rate() {
        let mut a = Motion::default();
        let mut b = Motion::default();
        a.target = 1.0;
        b.target = 1.0;
        for _ in 0..30 {
            a.step(1.0 / 60.0);
        }
        for _ in 0..60 {
            b.step(1.0 / 120.0);
        }
        assert!((a.pose().open - b.pose().open).abs() < 1e-5);
        a.step(f32::NAN);
        assert!(a.pose().open.is_finite());
    }
    #[test]
    fn scroll_off_motion_and_closed_pages_do_not_keep_input() {
        let now = Instant::now();
        let mut motion = Motion::default();
        motion.snap(true, now);
        assert!(!motion.moving());
        assert_eq!(
            motion.pose(),
            Pose {
                open: 1.0,
                sway: 0.0
            }
        );
        let area = Rect::new(4, 2, 80, 30);
        let mut page =
            Page::new(area, Buffer::empty(Rect::new(0, 0, 80, 30)), motion.pose()).unwrap();
        assert!(page.hit(20, 12).is_some());
        page.interactive = false;
        assert!(
            page.hit(20, 12).is_none(),
            "a visual exit must never retain menu input"
        );
        motion.snap(false, now);
        assert!(motion.hidden());
        assert!(!motion.moving());
        assert_eq!(motion.pose().roller_angle(), 0.0);
        assert!(
            (Pose {
                open: 1.0,
                sway: 0.0
            }
            .roller_angle()
                * RADIUS
                - released(Pose {
                    open: 1.0,
                    sway: 0.0
                }))
            .abs()
                < 1e-6
        );
    }

    #[test]
    fn scroll_page_rejects_unbounded_or_invalid_geometry() {
        let invalid = Pose {
            open: f32::NAN,
            sway: 0.0,
        };
        assert!(
            Page::new(
                Rect::new(0, 0, 80, 30),
                Buffer::empty(Rect::new(0, 0, 80, 30)),
                invalid
            )
            .is_none()
        );
        assert!(
            Page::new(
                Rect::new(0, 0, 161, 30),
                Buffer::empty(Rect::new(0, 0, 161, 30)),
                Pose::default()
            )
            .is_none()
        );
        assert!(
            Page::new(
                Rect::new(0, 0, 80, 30),
                Buffer::empty(Rect::new(0, 0, 79, 30)),
                Pose::default()
            )
            .is_none()
        );
    }
}
