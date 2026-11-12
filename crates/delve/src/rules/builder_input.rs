//! Bounded, replayable build-desk pointer actions and pure hit testing.
use super::{RoomKind, Run};
use crate::kingdom::{HEIGHT, WIDTH, catalog};
use angel_net::codec::{Bytes, put_varint};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BuilderAction {
    Plot { tile: (u8, u8), build: bool },
    Page(u8),
    Plan(u8),
    Selection(u16),
    Confirm,
    NextPlan { back: bool },
    /// The mouse button still held, and the pointer on a new tile (a row laid, a line drawn).
    Hold((u8, u8)),
    /// The button let go on a tile.
    Let((u8, u8)),
    /// A hotbar slot clicked (0 to 8).
    Slot(u8),
}
impl BuilderAction {
    pub fn valid(self) -> bool {
        match self {
            // (The hillside's tiles are the realm's, 240 by 176; the valley's desk is 72 by 54.)
            Self::Plot { tile, .. } => i16::from(tile.0) < crate::kingdom::REALM_WIDTH && i16::from(tile.1) < crate::kingdom::REALM_HEIGHT,
            Self::Page(n) => n < 9,
            Self::Plan(n) => usize::from(n) <= catalog().menu.len().max(super::hillside::tools().len()),
            Self::Selection(n) => n < 512,
            Self::Hold(tile) | Self::Let(tile) => i16::from(tile.0) < crate::kingdom::REALM_WIDTH && i16::from(tile.1) < crate::kingdom::REALM_HEIGHT,
            Self::Slot(n) => usize::from(n) < super::builder_rig::SLOTS,
            Self::Confirm | Self::NextPlan { .. } => true,
        }
    }
    pub(super) fn write(self, out: &mut Vec<u8>) {
        match self {
            Self::Plot { tile, build } => {
                out.extend_from_slice(&[0, tile.0, tile.1, u8::from(build)])
            }
            Self::Page(n) => out.extend_from_slice(&[1, n]),
            Self::Plan(n) => out.extend_from_slice(&[2, n]),
            Self::Selection(n) => {
                out.push(3);
                put_varint(out, u64::from(n));
            }
            Self::Confirm => out.push(4),
            Self::NextPlan { back } => out.extend_from_slice(&[5, u8::from(back)]),
            Self::Hold(tile) => out.extend_from_slice(&[6, tile.0, tile.1]),
            Self::Let(tile) => out.extend_from_slice(&[7, tile.0, tile.1]),
            Self::Slot(n) => out.extend_from_slice(&[8, n]),
        }
    }
    pub(super) fn read(b: &mut Bytes) -> Option<Self> {
        let boolean = |v| match v {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        let action = match b.byte().ok()? {
            0 => Self::Plot {
                tile: (b.byte().ok()?, b.byte().ok()?),
                build: boolean(b.byte().ok()?)?,
            },
            1 => Self::Page(b.byte().ok()?),
            2 => Self::Plan(b.byte().ok()?),
            3 => Self::Selection(u16::try_from(b.varint().ok()?).ok()?),
            4 => Self::Confirm,
            5 => Self::NextPlan {
                back: boolean(b.byte().ok()?)?,
            },
            6 => Self::Hold((b.byte().ok()?, b.byte().ok()?)),
            7 => Self::Let((b.byte().ok()?, b.byte().ok()?)),
            8 => Self::Slot(b.byte().ok()?),
            _ => return None,
        };
        action.valid().then_some(action)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub page: u8,
    pub selection: usize,
    pub plot: (i16, i16),
    pub camera: (i16, i16),
    pub zoom: bool,
    pub counts: [usize; 9],
    pub confirm: bool,
}
pub fn layout(run: &Run, knight: u32) -> Option<Layout> {
    let board = run.kingdom.industry.get(&knight).filter(|b| b.open)?;
    if run.dungeon.depth != 0
        || run.room().kind != RoomKind::Kingdom
        || run
            .players
            .get(&knight)
            .is_none_or(|h| h.absent || h.mirror.open)
    {
        return None;
    }
    let k = &run.home.industry;
    let c = catalog();
    let page = if board.gallery { 5 } else { board.page };
    let counts = [
        c.menu.len() + 1,
        c.technologies.len(),
        super::kingdom_gear::ITEMS.len(),
        k.claims.len(),
        super::geology::ROWS.len(),
        0,
        k.population.citizens.len(),
        9,
        k.buildings.len(),
    ];
    let confirm = match page {
        0 => k.at(board.tile).is_none(),
        1 => c
            .technologies
            .iter()
            .nth(board.selection)
            .is_some_and(|(id, _)| !k.researched.contains(id)),
        2 => board.selection < super::kingdom_gear::ITEMS.len(),
        7 => k
            .expansion_offer(((board.selection % 3) as i16, (board.selection / 3) as i16))
            .is_ok_and(|offer| offer.blocked.is_none()),
        8 => board.selection < k.buildings.len(),
        _ => false,
    };
    Some(Layout {
        page,
        selection: board.selection,
        plot: board.tile,
        camera: board.camera.unwrap_or(board.tile),
        zoom: board.zoom,
        counts,
        confirm,
    })
}

/// Invert the exact build-desk fit inside the room picture, excluding the
/// party/HUD bands and the desk's letterbox. Inputs are frame pixels.
pub fn logical_point(x: u32, y: u32, view_w: u32, view_h: u32, top: u32) -> Option<(u32, u32)> {
    if view_w == 0 || view_h == 0 || x >= view_w {
        return None;
    }
    let y = y.checked_sub(top)?;
    if y >= view_h {
        return None;
    }
    let scale = (view_w as f32 / 768.).min(view_h as f32 / 448.);
    let w = (768. * scale) as u32;
    let h = (448. * scale) as u32;
    if w == 0 || h == 0 {
        return None;
    }
    let x = x.checked_sub((view_w - w) / 2)?;
    let y = y.checked_sub((view_h - h) / 2)?;
    (x < w && y < h).then(|| {
        (
            (u64::from(x) * 768 / u64::from(w)) as u32,
            (u64::from(y) * 448 / u64::from(h)) as u32,
        )
    })
}

fn within(x: u32, y: u32, left: u32, top: u32, w: u32, h: u32) -> bool {
    x >= left && x < left + w && y >= top && y < top + h
}
pub fn hit(layout: &Layout, x: u32, y: u32, button: u8) -> Option<BuilderAction> {
    if x >= 768 || y >= 448 || !matches!(button, 0 | 2) {
        return None;
    }
    if button == 0 && within(x, y, 14, 65, 747, 18) {
        return Some(BuilderAction::Page(((x - 14) / 83) as u8));
    }
    if layout.page == 0 {
        // The map's first 16 pixels are its status strip, not a plot.
        if within(x, y, 12, 101, 448, 292) {
            let t = if layout.zoom { 48 } else { 32 };
            let ox =
                (i32::from(layout.camera.0) * t + t / 2 - 224).clamp(0, i32::from(WIDTH) * t - 448);
            let oy = (i32::from(layout.camera.1) * t + t / 2 - 154)
                .clamp(0, i32::from(HEIGHT) * t - 308);
            let tile = (
                ((x as i32 - 12 + ox) / t) as u8,
                ((y as i32 - 85 + oy) / t) as u8,
            );
            return Some(BuilderAction::Plot {
                tile,
                build: button == 0,
            });
        }
        if button == 0 && within(x, y, 490, 95, 254, 40) {
            return Some(BuilderAction::NextPlan { back: x < 617 });
        }
    }
    if button != 0 {
        return None;
    }
    let count = layout
        .counts
        .get(usize::from(layout.page))
        .copied()
        .unwrap_or(0);
    let selected = match layout.page {
        1 => row(x, y, 120, 49, 44, count, layout.selection.saturating_sub(4)),
        2 => (0..count.min(6)).find(|&i| {
            within(
                x,
                y,
                20 + (i % 3) as u32 * 144,
                92 + (i / 3) as u32 * 134,
                136,
                124,
            )
        }),
        3 | 8 => row(x, y, 122, 48, 43, count, layout.selection.saturating_sub(4)),
        4 => row(x, y, 123, 60, 54, count, 0),
        6 => row(x, y, 135, 44, 40, count, layout.selection.saturating_sub(4)),
        7 => (0..9).find(|&i| {
            within(
                x,
                y,
                24 + (i % 3) as u32 * 142,
                122 + (i / 3) as u32 * 82,
                134,
                76,
            )
        }),
        _ => None,
    };
    if let Some(i) = selected {
        return u16::try_from(i)
            .ok()
            .filter(|&i| i < 512)
            .map(BuilderAction::Selection);
    }
    let action_y = match layout.page {
        0 => Some(240),
        1 => Some(345),
        2 => Some(290),
        7 => Some(341),
        8 => Some(148),
        _ => None,
    };
    if layout.confirm && action_y.is_some_and(|yy| within(x, y, 486, yy, 258, 28)) {
        return Some(BuilderAction::Confirm);
    }
    None
}
fn row(
    x: u32,
    y: u32,
    top: u32,
    stride: u32,
    height: u32,
    count: usize,
    start: usize,
) -> Option<usize> {
    (start..count.min(start.saturating_add(5)))
        .find(|&i| within(x, y, 24, top + (i - start) as u32 * stride, 424, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Input, command::DelveCmd};
    use angel_net::cmd::Command;
    fn board() -> Layout {
        Layout {
            page: 0,
            selection: 0,
            plot: (8, 8),
            camera: (8, 8),
            zoom: true,
            counts: [0, 5, 6, 12, 4, 0, 20, 9, 12],
            confirm: true,
        }
    }
    #[test]
    fn exact_letterbox_and_bands_are_excluded_before_hits() {
        assert_eq!(logical_point(384, 28 + 224, 768, 448, 28), Some((384, 224)));
        assert_eq!(logical_point(0, 27, 768, 448, 28), None);
        assert_eq!(logical_point(0, 28 + 448, 768, 448, 28), None);
        // Tall room: the desk has black rows above and below it.
        assert_eq!(logical_point(100, 28, 768, 600, 28), None);
        assert_eq!(logical_point(384, 28 + 300, 768, 600, 28), Some((384, 224)));
        assert_eq!(logical_point(u32::MAX, u32::MAX, 768, 448, 28), None);
    }
    #[test]
    fn map_clicks_respect_frozen_camera_zoom_and_button() {
        let mut b = board();
        assert_eq!(
            hit(&b, 236, 239, 0),
            Some(BuilderAction::Plot {
                tile: (8, 8),
                build: true
            })
        );
        b.plot = (30, 20);
        assert_eq!(
            hit(&b, 236, 239, 2),
            Some(BuilderAction::Plot {
                tile: (8, 8),
                build: false
            })
        );
        b.zoom = false;
        b.camera = (71, 53);
        assert_eq!(
            hit(&b, 459, 392, 0),
            Some(BuilderAction::Plot {
                tile: (71, 53),
                build: true
            })
        );
        assert_eq!(hit(&b, 460, 392, 0), None);
        assert_eq!(hit(&b, 236, 100, 0), None);
        assert_eq!(hit(&b, 236, 239, 1), None);
    }
    #[test]
    fn tabs_cards_scrolled_lists_and_gaps_hit_only_the_visible_control() {
        let mut b = board();
        assert_eq!(hit(&b, 680, 72, 0), Some(BuilderAction::Page(8)));
        assert_eq!(
            hit(&b, 500, 110, 0),
            Some(BuilderAction::NextPlan { back: true })
        );
        b.page = 2;
        assert_eq!(hit(&b, 180, 240, 0), Some(BuilderAction::Selection(4)));
        assert_eq!(hit(&b, 157, 240, 0), None);
        b.page = 6;
        b.selection = 9;
        assert_eq!(hit(&b, 30, 136, 0), Some(BuilderAction::Selection(5)));
        assert_eq!(hit(&b, 30, 175, 0), None);
        b.page = 7;
        assert_eq!(hit(&b, 310, 288, 0), Some(BuilderAction::Selection(8)));
        b.confirm = false;
        assert_eq!(hit(&b, 500, 350, 0), None);
        b.confirm = true;
        assert_eq!(hit(&b, 500, 350, 0), Some(BuilderAction::Confirm));
    }

    #[test]
    fn builder_codec_is_bounded_and_leaves_old_command_bytes_unchanged() {
        let encode = |cmd: &DelveCmd| {
            let mut bytes = Vec::new();
            cmd.encode(&mut bytes);
            bytes
        };
        assert_eq!(encode(&DelveCmd::default()), vec![0x55, 0, 0x55, 0]);
        for action in [
            BuilderAction::Plot {
                tile: (71, 53),
                build: true,
            },
            // The hillside's tiles are the realm's: 240 by 176.
            BuilderAction::Plot {
                tile: (239, 175),
                build: true,
            },
            BuilderAction::Plot {
                tile: (0, 0),
                build: false,
            },
            BuilderAction::Page(8),
            BuilderAction::Plan(catalog().menu.len() as u8),
            BuilderAction::Selection(511),
            BuilderAction::Confirm,
            BuilderAction::NextPlan { back: true },
            BuilderAction::NextPlan { back: false },
        ] {
            let input = Input {
                builder: Some(action),
                ..Input::default()
            };
            let cmd = DelveCmd {
                held: input,
                pressed: input,
            };
            let bytes = encode(&cmd);
            assert_eq!(DelveCmd::decode(&bytes), Some(cmd));
            for cut in 0..bytes.len() {
                assert!(DelveCmd::decode(&bytes[..cut]).is_none());
            }
            let restored: Input =
                serde_json::from_str(&serde_json::to_string(&input).unwrap()).unwrap();
            assert_eq!(input, restored);
            assert!(cmd.held_only().input().builder.is_none());
        }
        for action in [
            BuilderAction::Plot {
                tile: (240, 0),
                build: true,
            },
            BuilderAction::Plot {
                tile: (0, 176),
                build: false,
            },
            BuilderAction::Page(9),
            BuilderAction::Plan(255),
            BuilderAction::Selection(512),
        ] {
            assert!(!action.valid());
            assert!(
                DelveCmd::decode(&encode(&DelveCmd {
                    pressed: Input {
                        builder: Some(action),
                        ..Input::default()
                    },
                    ..DelveCmd::default()
                }))
                .is_none()
            );
        }
        for payload in [vec![6], vec![0, 1, 1, 2], vec![5, 2], vec![3, 0x80, 0x04]] {
            let mut bytes = vec![0x55, 0x80, 0x80, 0x40];
            bytes.extend(payload);
            bytes.extend([0x55, 0]);
            assert!(DelveCmd::decode(&bytes).is_none());
        }
    }

    #[test]
    fn latest_pointer_action_survives_command_pileup_but_never_repeats() {
        let older = DelveCmd {
            pressed: Input {
                builder: Some(BuilderAction::Plot {
                    tile: (8, 8),
                    build: true,
                }),
                ..Input::default()
            },
            ..DelveCmd::default()
        };
        let mut newer = DelveCmd {
            pressed: Input {
                builder: Some(BuilderAction::Page(7)),
                ..Input::default()
            },
            ..DelveCmd::default()
        };
        newer.absorb(&older);
        assert_eq!(newer.input().builder, Some(BuilderAction::Page(7)));
        assert_eq!(newer.held_only().input().builder, None);
        let mut delayed = DelveCmd::default();
        delayed.absorb(&older);
        assert_eq!(delayed.input().builder, older.input().builder);
    }
}
