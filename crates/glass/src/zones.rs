//! Pointer zones: regions of the window where a producer drawing layers
//! takes the left button (a floating panel you can drag), while the
//! cockpit keeps every other press, move and wheel turn.

/// What a zone does with a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneKind {
    /// Dragging moves what is there; a click without a drag goes through
    /// to the text under it, as if the zone were not there.
    Move,
    /// Dragging resizes what is there.
    Resize,
    /// A button: its click is the producer's alone.
    Button,
}

/// A region of the window, `[x, y, width, height]` in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub rect: [f32; 4],
    pub kind: ZoneKind,
}

impl Zone {
    pub fn new(rect: [f32; 4], kind: ZoneKind) -> Self {
        Self { rect, kind }
    }

    /// Whether `p` is in it: left and top edges in, right and bottom out.
    pub fn holds(&self, p: [f32; 2]) -> bool {
        let [x, y, w, h] = self.rect;
        p[0] >= x && p[0] < x + w && p[1] >= y && p[1] < y + h
    }
}

/// What the pointer did in a producer's zones, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
    /// The left button went down in zone `zone` (its place in the list
    /// the producer last set), with Shift or Control held or not.
    Press {
        at: [f32; 2],
        zone: usize,
        kind: ZoneKind,
        shift: bool,
        ctrl: bool,
    },
    /// The pointer moved while that button is held.
    Move { at: [f32; 2] },
    /// The button came up. `click`: it never moved further than a few
    /// pixels from where it went down.
    Release { at: [f32; 2], click: bool },
}

/// How far a press may wander and still be a click, in pixels.
pub(crate) const CLICK_SLOP: f32 = 4.0;

/// The most events kept for a producer that is not reading them.
pub(crate) const MAX_EVENTS: usize = 256;

/// The first zone holding `p`, with its group and place in that group's
/// list.
pub(crate) fn zone_at(zones: &[(u32, Zone)], p: [f32; 2]) -> Option<(u32, usize, Zone)> {
    let mut place: Vec<(u32, usize)> = Vec::new();
    for (group, zone) in zones {
        let i = match place.iter_mut().find(|(g, _)| g == group) {
            Some((_, n)) => {
                *n += 1;
                *n - 1
            }
            None => {
                place.push((*group, 1));
                0
            }
        };
        if zone.holds(p) {
            return Some((*group, i, *zone));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_zone_holding_the_pointer_wins_and_knows_its_place() {
        let zones = vec![
            (7, Zone::new([0.0, 0.0, 10.0, 10.0], ZoneKind::Button)),
            (7, Zone::new([0.0, 0.0, 100.0, 100.0], ZoneKind::Move)),
            (9, Zone::new([50.0, 50.0, 100.0, 100.0], ZoneKind::Resize)),
        ];
        assert_eq!(
            zone_at(&zones, [5.0, 5.0]).map(|z| (z.0, z.1)),
            Some((7, 0))
        );
        assert_eq!(
            zone_at(&zones, [60.0, 60.0]).map(|z| (z.0, z.1)),
            Some((7, 1))
        );
        assert_eq!(
            zone_at(&zones, [120.0, 120.0]).map(|z| (z.0, z.1, z.2.kind)),
            Some((9, 0, ZoneKind::Resize))
        );
        assert_eq!(zone_at(&zones, [200.0, 5.0]), None);
        // The right and bottom edges are outside.
        assert_eq!(zone_at(&zones[2..], [150.0, 60.0]), None);
    }

    #[test]
    fn a_zone_without_area_or_with_a_nan_holds_nothing() {
        assert!(Zone::new([0.0, 0.0, 1.0, 1.0], ZoneKind::Move).holds([0.5, 0.5]));
        assert!(!Zone::new([0.0, 0.0, 0.0, 1.0], ZoneKind::Move).holds([0.0, 0.5]));
        assert!(!Zone::new([f32::NAN, 0.0, 1.0, 1.0], ZoneKind::Move).holds([0.5, 0.5]));
    }
}
