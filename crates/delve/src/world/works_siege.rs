//! What each stone of a castle stands on (the siege data of the Works).
//!
//! The architect (Blocks) writes, with a v2 export, a `rests` row for every stone: the
//! distances back, in lay order, to the stones it directly rests on, `0` for the
//! ground (`docs/CASTLE_EXPORT_V2.md`). A stone stands while enough of what it rests on
//! stands, so a lost stone brings down what it held and nothing else.
//!
//! The Keep's baked castle was made from a v1 export and carries no rows, so the same
//! rows are derived here from the stones themselves by Blocks' own rule: a stone rests on
//! the earlier stones whose tops meet its bottom under it (to six centimetres, with some
//! overlap), and on the ground when it sits on it; a wedge with nothing under it (a
//! voussoir, a springer, a jamb) rests on the stones it touches. [`Rests::of_json`]
//! reads a v2 export's own rows, and a test sets the two side by side.
//!
//! The rule the game plays is Blocks' `majority`: a stone needs more than half of what it
//! rests on, which is what brings down an overhang and what a breach leaves hanging.
//! A stone that rests on nothing at all is held by falsework, and stands.

use super::works::Castle;
use std::collections::{BTreeSet, HashMap};

/// A stone's top meets another's bottom when they are this close, metres.
const TOUCH: f32 = 0.06;
/// Faces in contact overlap by at least this much on both axes, metres.
const OVERLAP: f32 = 0.02;
/// A stone whose bottom is this near the ground (or under it) sits on it.
const GROUND: f32 = 0.06;
/// How near a wedge is to a stone it rests on by touching, metres.
const NEAR: f32 = 0.05;
/// A cell of the index, metres.
const CELL: f32 = 2.0;
/// The roles that rest on what they touch when nothing is under them.
const WEDGES: [&str; 7] = ["voussoir", "chevron", "spandrel", "passage-vault", "reveal", "jamb", "ashlar"];

/// Every stone's holders, in lay order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rests {
    start: Vec<u32>,
    data: Vec<u16>,
}

impl Rests {
    /// How many stones it has rows for.
    pub fn len(&self) -> usize {
        self.start.len().saturating_sub(1)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The distances back to what stone `i` rests on, ascending; a leading `0` is the ground.
    pub fn row(&self, i: u32) -> &[u16] {
        let i = i as usize;
        match (self.start.get(i), self.start.get(i + 1)) {
            (Some(&a), Some(&b)) => &self.data[a as usize..b as usize],
            _ => &[],
        }
    }

    fn from_rows(rows: impl IntoIterator<Item = Vec<u16>>) -> Rests {
        let mut r = Rests { start: vec![0], data: Vec::new() };
        for row in rows {
            r.data.extend(row);
            r.start.push(r.data.len() as u32);
        }
        r
    }

    /// A v2 export's own rows: `rests` is a list of lists of distances.
    pub fn of_json(text: &str) -> Result<Rests, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        let rows = v["rests"].as_array().ok_or("the export has no rests (it is a v1 file)")?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let r = row.as_array().ok_or("a rests row is not a list")?;
            out.push(r.iter().map(|d| d.as_u64().and_then(|n| u16::try_from(n).ok()).ok_or("a distance is not a number")).collect::<Result<Vec<u16>, _>>()?);
        }
        Ok(Rests::from_rows(out))
    }

    /// The rows derived from a castle's stones.
    pub fn of(castle: &Castle) -> Rests {
        let n = castle.stones.len();
        let boxes: Vec<[f32; 6]> = castle.stones.iter().map(|s| s.aabb().map(|v| f32::from(v) / 100.0)).collect();
        let wedge: Vec<bool> = castle.stones.iter().map(|s| WEDGES.contains(&castle.role(s.role))).collect();
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let cells = |b: &[f32; 6], pad: f32| {
            let (x0, x1) = (((b[0] - pad) / CELL).floor() as i32, ((b[3] + pad) / CELL).floor() as i32);
            let (y0, y1) = (((b[1] - pad) / CELL).floor() as i32, ((b[4] + pad) / CELL).floor() as i32);
            (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
        };
        let mut rows: Vec<Vec<u16>> = Vec::with_capacity(n);
        for i in 0..n {
            let a = boxes[i];
            let mut near: Vec<u32> = cells(&a, NEAR).filter_map(|c| grid.get(&c)).flatten().copied().collect();
            near.sort_unstable();
            near.dedup();
            let mut holders: Vec<u32> = near
                .iter()
                .copied()
                .filter(|&j| {
                    let b = boxes[j as usize];
                    (b[5] - a[2]).abs() <= TOUCH && b[0] < a[3] - OVERLAP && b[3] > a[0] + OVERLAP && b[1] < a[4] - OVERLAP && b[4] > a[1] + OVERLAP
                })
                .collect();
            let ground = a[2] <= GROUND;
            if holders.is_empty() && !ground {
                // Wedged, or held on falsework: what it touches, if it is a wedge.
                holders = near
                    .iter()
                    .copied()
                    .filter(|&j| {
                        let b = boxes[j as usize];
                        wedge[j as usize]
                            && b[0] <= a[3] + NEAR
                            && b[3] >= a[0] - NEAR
                            && b[1] <= a[4] + NEAR
                            && b[4] >= a[1] - NEAR
                            && b[2] <= a[5] + NEAR
                            && b[5] >= a[2] - NEAR
                    })
                    .collect();
            }
            let mut row: Vec<u16> = holders.iter().map(|&j| (i as u32 - j).min(u32::from(u16::MAX)) as u16).collect();
            row.sort_unstable();
            if ground {
                row.insert(0, 0);
            }
            rows.push(row);
            for c in cells(&a, 0.0) {
                grid.entry(c).or_default().push(i as u32);
            }
        }
        Rests::from_rows(rows)
    }

    /// Whether stone `i` stands when the stones before `from` all stand and `gone` are lost:
    /// the one-pass rule, the stones `from..to` judged in lay order.
    pub fn falls(&self, from: u32, to: u32, gone: &BTreeSet<u32>) -> Vec<u32> {
        let to = to.min(self.len() as u32);
        let mut standing: Vec<bool> = Vec::with_capacity(to.saturating_sub(from) as usize);
        let mut fell = Vec::new();
        for i in from..to {
            let ok = if gone.contains(&i) {
                false
            } else {
                let row = self.row(i);
                if row.is_empty() || row[0] == 0 {
                    true
                } else {
                    let up = row.iter().filter(|&&d| i.checked_sub(u32::from(d)).is_none_or(|j| j < from || standing[(j - from) as usize])).count();
                    up * 2 > row.len()
                }
            };
            if !ok && !gone.contains(&i) {
                fell.push(i);
            }
            standing.push(ok);
        }
        fell
    }

    /// What losing `stone` brings down with it, the stones laid before `to` that rest on what
    /// is gone (not `stone` itself).
    pub fn held_by(&self, stone: u32, from: u32, to: u32, gone: &BTreeSet<u32>) -> Vec<u32> {
        let mut all = gone.clone();
        all.insert(stone);
        self.falls(from, to, &all)
    }
}
