//! The floor as a map: what the party knows of it, as plain data a screen
//! can draw. Nothing here draws, and nothing here is kept in the run or
//! sent over the wire: `Run::floor_map` reads the floor, the keys and the
//! party as they stand and hands back a fresh `FloorMap` each time.
//!
//! The map is made of **nodes** (a rectangle in map units, what the place
//! is, how much of it is known, and the icons to mark on it), **links**
//! (the doorways between two nodes, locked or open) and **markers** (each
//! counted knight, at the point inside its node where it stands). It knows
//! nothing about a grid of rooms: the floor today is a grid, so a node is a
//! cell and a link joins two cells that touch, but a floor of free-standing
//! chambers (rectangles anywhere, doorways anywhere on their edges) feeds
//! the same map by building the same three lists.
//!
//! **Fog of war.** A room is *visited* once a knight has stood in it
//! (`Room::visited`, or a knight stands in it now); *cleared* when it is
//! visited and its foes are done. A room through a doorway of a visited
//! room is *seen*: it is on the map, an outline, but what it is stays
//! hidden. Every other room is not in the map at all (`NodeState::Unknown`
//! is the state of everything left out). A vault behind a cracked wall is
//! left out until the wall is blown, whatever else is known of it.

use super::keyring;
use super::missions;
use super::*;
use std::collections::BTreeSet;

pub type NodeId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeState {
    Unknown,
    Seen,
    Visited,
    Cleared,
}

impl NodeState {
    pub fn visited(self) -> bool {
        self >= NodeState::Visited
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Unknown,
    Entrance,
    Chamber,
    Passage,
    Sanctuary,
    Treasure,
    Stairs,
    Lair,
    Vault,
    Pit,
    Wanted,
    Ledge,
    Nook,
    Forge,
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Icon {
    Guardian,
    Chief,
    Stairs,
    Sanctuary,
    Entrance,
    Treasure,
    Merchant,
    Key,
    Vault,
    Pit,
    Wanted,
    Ledge,
    Nook,
    Forge,
    Hold,
    Mission,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn centre(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let (x0, y0) = (self.x.min(other.x), self.y.min(other.y));
        let (x1, y1) = (
            (self.x + self.w).max(other.x + other.w),
            (self.y + self.h).max(other.y + other.h),
        );
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub rect: Rect,
    pub kind: NodeKind,
    pub state: NodeState,
    pub icons: Vec<Icon>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    pub a: NodeId,
    pub b: NodeId,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub knight: u32,
    pub name: String,
    pub node: NodeId,
    pub at: (f32, f32),
    pub own: bool,
    pub away: bool,
    pub fallen: bool,
    pub key: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FloorMap {
    pub depth: u32,
    pub pack: Option<Pack>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub party: Vec<Marker>,
    pub here: Option<NodeId>,
}

impl FloorMap {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn own(&self) -> Option<&Marker> {
        self.party.iter().find(|m| m.own)
    }

    pub fn bounds(&self) -> Option<Rect> {
        self.nodes.iter().map(|n| n.rect).reduce(|a, b| a.union(&b))
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

type Standing<'a> = (u32, usize, (f32, f32), Option<&'a Hero>);

const INSET: f32 = 0.125;

impl Run {
    pub fn floor_map(&self, seat: Option<u32>) -> FloorMap {
        let floor = &self.dungeon;
        let rooms = &floor.rooms;
        if floor.depth == 0 || rooms.is_empty() || self.room().kind.in_world() {
            return FloorMap::default();
        }
        let room_of = |id: u32| self.exits.outside.get(&id).map_or(self.at, |e| e.at);
        let mut standing: Vec<Standing> = Vec::new();
        for (&id, hero) in self.players.iter().filter(|(_, h)| !h.absent) {
            standing.push((id, room_of(id), (hero.x, hero.y), Some(hero)));
        }
        for (&id, entry) in &self.exits.outside {
            if !self.players.contains_key(&id) {
                let spot = entry.party.get(&id).copied().unwrap_or_default();
                standing.push((id, entry.at, spot, None));
            }
        }
        standing.retain(|&(_, room, _, _)| room < rooms.len());
        let occupied: BTreeSet<usize> = standing.iter().map(|&(_, room, _, _)| room).collect();

        let hidden = |i: usize| {
            rooms[i].kind == RoomKind::Secret
                && !floor.secret.is_some_and(|s| s.vault == i && s.found)
        };
        let visited = |i: usize| !hidden(i) && (rooms[i].visited || occupied.contains(&i));

        let mut seen = BTreeSet::new();
        let mut doors: BTreeSet<(usize, usize)> = BTreeSet::new();
        for from in (0..rooms.len()).filter(|&i| visited(i)) {
            for dir in 0..4 {
                let Some(to) = floor.neighbour(from, dir).filter(|&to| !hidden(to)) else {
                    continue;
                };
                if !visited(to) {
                    seen.insert(to);
                }
                doors.insert((from.min(to), from.max(to)));
            }
        }
        if self.freedom.charted {
            for from in (0..rooms.len()).filter(|&i| !hidden(i)) {
                if !visited(from) {
                    seen.insert(from);
                }
                for dir in 0..4 {
                    if let Some(to) = floor.neighbour(from, dir).filter(|&to| !hidden(to)) {
                        doors.insert((from.min(to), from.max(to)));
                    }
                }
            }
        }

        let guardian = self.guardian_icon_room();
        let chiefs: BTreeSet<usize> = self.boss_leader_rooms().collect();
        let camp = self.merchant_camp().map(|c| c.room);
        let key_at = match self.works.key.hold {
            keyring::Hold::Lying { room, .. } => Some(room),
            _ => None,
        };
        let carrier = self.key_bearer();
        let spots = seat.map(|id| self.mission_floor_spots(id)).unwrap_or_default();
        let down = rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .or_else(|| rooms.iter().position(|r| matches!(r.kind, RoomKind::Lair | RoomKind::Threshold)));
        let marked: BTreeSet<usize> = spots
            .iter()
            .filter_map(|spot| match spot {
                missions::Spot::Guardian => guardian,
                missions::Spot::Stairs => down,
                missions::Spot::Hall => rooms.iter().position(|r| r.kind == RoomKind::DwarfHall),
            })
            .collect();

        let mut nodes = Vec::new();
        for (i, room) in rooms.iter().enumerate() {
            let state = if visited(i) {
                if room.cleared {
                    NodeState::Cleared
                } else {
                    NodeState::Visited
                }
            } else if seen.contains(&i) {
                NodeState::Seen
            } else {
                continue;
            };
            let rect = Rect {
                x: room.cell.0 as f32 + INSET,
                y: room.cell.1 as f32 + INSET,
                w: 1.0 - 2.0 * INSET,
                h: 1.0 - 2.0 * INSET,
            };
            let (kind, icons) = if state.visited() {
                let mut icons = Vec::new();
                if guardian == Some(i) {
                    icons.push(Icon::Guardian);
                }
                if chiefs.contains(&i) {
                    icons.push(Icon::Chief);
                }
                icons.extend(kind_icon(room.kind));
                if camp == Some(i) {
                    icons.push(Icon::Merchant);
                }
                if key_at == Some(i) {
                    icons.push(Icon::Key);
                }
                if marked.contains(&i) {
                    icons.push(Icon::Mission);
                }
                (kind_of(room.kind), icons)
            } else {
                (NodeKind::Unknown, Vec::new())
            };
            nodes.push(Node {
                id: i,
                rect,
                kind,
                state,
                icons,
            });
        }

        let rect_of = |id: usize| nodes.iter().find(|n| n.id == id).map(|n| n.rect);
        let shut = self.boss_door_shut();
        let boss = self.boss_gates.as_ref().map(|g| g.guardian);
        let mut links = Vec::new();
        for (a, b) in doors {
            let (Some(ra), Some(rb)) = (rect_of(a), rect_of(b)) else {
                continue;
            };
            let ((ax, ay), (bx, by)) = (ra.centre(), rb.centre());
            let edge = |r: &Rect, toward: (f32, f32), (cx, cy): (f32, f32)| {
                let (dx, dy) = (toward.0 - cx, toward.1 - cy);
                if dx.abs() >= dy.abs() {
                    (cx + dx.signum() * r.w / 2.0, cy)
                } else {
                    (cx, cy + dy.signum() * r.h / 2.0)
                }
            };
            links.push(Link {
                a,
                b,
                from: edge(&ra, (bx, by), (ax, ay)),
                to: edge(&rb, (ax, ay), (bx, by)),
                locked: shut && (boss == Some(a) || boss == Some(b)),
            });
        }

        let mut party = Vec::new();
        for (id, room, (x, y), hero) in standing {
            let Some(rect) = rect_of(room) else {
                continue;
            };
            let r = &rooms[room];
            let (u, v) = (
                (x / r.width()).clamp(0.0, 1.0),
                (y / r.height()).clamp(0.0, 1.0),
            );
            party.push(Marker {
                knight: id,
                name: hero.map_or_else(
                    || format!("Knight {id}"),
                    |h| h.shown_name(),
                ),
                node: room,
                at: (rect.x + u * rect.w, rect.y + v * rect.h),
                own: seat == Some(id),
                away: hero.is_some_and(|h| h.away || h.stone),
                fallen: hero.is_some_and(|h| h.hp == 0),
                key: carrier == Some(id),
            });
        }

        let here = seat
            .filter(|id| self.players.contains_key(id))
            .map_or(self.at, room_of);
        let here = nodes.iter().any(|n| n.id == here).then_some(here);
        FloorMap {
            depth: floor.depth,
            pack: Some(floor.pack),
            nodes,
            links,
            party,
            here,
        }
    }

    fn guardian_icon_room(&self) -> Option<usize> {
        match &self.boss_gates {
            Some(g) => {
                let kind = self.dungeon.rooms.get(g.guardian)?.kind;
                let keeps = g.guardian_boss.is_some()
                    || matches!(kind, RoomKind::Lair | RoomKind::Threshold);
                (keeps && !g.guardian_defeated).then_some(g.guardian)
            }
            None => self.dungeon.rooms.iter().position(|r| {
                matches!(
                    r.kind,
                    RoomKind::Stairs | RoomKind::Lair | RoomKind::Threshold
                ) && !r.cleared
            }),
        }
    }
}

fn kind_of(kind: RoomKind) -> NodeKind {
    match kind {
        RoomKind::Start => NodeKind::Entrance,
        RoomKind::Hall => NodeKind::Passage,
        RoomKind::Sanctuary => NodeKind::Sanctuary,
        RoomKind::Treasure => NodeKind::Treasure,
        RoomKind::Stairs => NodeKind::Stairs,
        RoomKind::Lair | RoomKind::Threshold => NodeKind::Lair,
        RoomKind::Secret => NodeKind::Vault,
        RoomKind::Pit => NodeKind::Pit,
        RoomKind::Wanted => NodeKind::Wanted,
        RoomKind::Ledge => NodeKind::Ledge,
        RoomKind::Forge => NodeKind::Forge,
        RoomKind::Holding => NodeKind::Hold,
        k if k.nook() => NodeKind::Nook,
        _ => NodeKind::Chamber,
    }
}

fn kind_icon(kind: RoomKind) -> Option<Icon> {
    match kind {
        RoomKind::Start => Some(Icon::Entrance),
        RoomKind::Sanctuary => Some(Icon::Sanctuary),
        RoomKind::Treasure => Some(Icon::Treasure),
        RoomKind::Stairs => Some(Icon::Stairs),
        RoomKind::Secret => Some(Icon::Vault),
        RoomKind::Pit => Some(Icon::Pit),
        RoomKind::Wanted => Some(Icon::Wanted),
        RoomKind::Ledge => Some(Icon::Ledge),
        RoomKind::Forge => Some(Icon::Forge),
        RoomKind::Holding => Some(Icon::Hold),
        k if k.nook() => Some(Icon::Nook),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../tests/rules/floor_map__tests.rs"]
mod tests;
