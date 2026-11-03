//! The scripted bot: what a knight presses when nobody is at the keys. It
//! plays the whole adventure with the same buttons a person has: locks on,
//! circles, parries a lunge, lights the braziers from the ember, pushes the
//! block, shields the turret's bolt toward the crystal, burns the hood off,
//! carries the Boss Key, lights the Warden's heart and strikes while it is
//! open. It reads only the session, so it is a pure function of the state.
use super::foes::Danger;
use super::kit::*;
use super::world::*;
use super::*;
use std::collections::VecDeque;

/// How near a foe must be for the bot to take it on, in pixels.
const ENGAGE: i32 = 150;
/// The bot raises its shield this many ticks before a blow lands.
const PARRY_LEAD: u16 = 5;

fn sign(v: i32, dead: i32) -> i8 {
    if v > dead {
        1
    } else if v < -dead {
        -1
    } else {
        0
    }
}

impl Session {
    /// What bot `id` presses this tick.
    pub fn bot_input(&self, id: u32, _hero: &Hero, r: &Rules) -> Input {
        let Some(me) = self.seats.get(&id) else {
            return Input::default();
        };
        if !me.up() || me.act == Act::Item || me.act == Act::Fall {
            return Input::default();
        }
        if self.done.is_some() {
            return self.bot_finale();
        }
        if let Some(w) = &self.boss
            && w.hp > 0
            && w.st != Ws::Asleep
        {
            return self.bot_boss(id, me, w, r);
        }
        let foe = self
            .foes
            .iter()
            .filter(|f| f.alive() && f.kind == Fk::Snuffer)
            .min_by_key(|f| dist(f.x - me.x, f.y - me.y));
        if let Some(f) = foe
            && dist(f.x - me.x, f.y - me.y) < ENGAGE * Q
        {
            return self.bot_fight(id, me, f, r);
        }
        // A foe still asleep across the room: wake it.
        if let Some(f) = foe {
            return self.bot_goto(me, (f.x, f.y), 30, false);
        }
        // A friend on the ground, and nothing about: lift them.
        if let Some(o) = self.seats.values().find(|o| o.id != me.id && o.act == Act::Down) {
            let mut i = self.bot_goto(me, (o.x, o.y), 10, false);
            if dist(o.x - me.x, o.y - me.y) < 18 * Q {
                i.interact = true;
            }
            return i;
        }
        self.bot_objective(id, me, r)
    }

    // ----- walking ------------------------------------------------------

    /// The way to walk now to get to tile `to` from where the knight is:
    /// breadth-first over the tiles a body can enter.
    fn bot_step(&self, me: &Seat, to: (i32, i32)) -> (i8, i8) {
        let p = plan(usize::from(self.room));
        let (w, h) = (p.w(), p.h());
        let from = (tile_of(me.x), tile_of(me.y));
        if from.0 < 0 || from.1 < 0 || from.0 >= w || from.1 >= h {
            return (0, 0);
        }
        if from == to {
            return (sign(middle(to.0) - me.x, 2 * Q), sign(middle(to.1) - me.y, 2 * Q));
        }
        let open = |x: i32, y: i32| -> bool {
            if x < 0 || y < 0 || x >= w || y >= h {
                return false;
            }
            (x, y) == to
                || (!wall(p.at(x, y))
                    && p.at(x, y) != b'='
                    && !self.props.iter().any(|q| q.blocks(middle(x), middle(y))))
        };
        let mut came = vec![None::<(i32, i32)>; (w * h) as usize];
        let mut queue = VecDeque::from([from]);
        came[(from.1 * w + from.0) as usize] = Some(from);
        while let Some((x, y)) = queue.pop_front() {
            if (x, y) == to {
                break;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if open(nx, ny) && came[(ny * w + nx) as usize].is_none() {
                    came[(ny * w + nx) as usize] = Some((x, y));
                    queue.push_back((nx, ny));
                }
            }
        }
        if to.0 < 0 || to.1 < 0 || to.0 >= w || to.1 >= h || came[(to.1 * w + to.0) as usize].is_none() {
            return (0, 0);
        }
        // Back from the goal to the tile after ours.
        let mut at = to;
        while let Some(prev) = came[(at.1 * w + at.0) as usize] {
            if prev == from {
                break;
            }
            at = prev;
        }
        // Aim for the middle of that tile, so a corner is cut cleanly.
        (sign(middle(at.0) - me.x, Q), sign(middle(at.1) - me.y, Q))
    }

    /// Walk to a point; press E there when `press`.
    fn bot_goto(&self, me: &Seat, to: (i32, i32), reach_px: i32, press: bool) -> Input {
        let mut i = Input::default();
        let d = dist(to.0 - me.x, to.1 - me.y);
        if d > reach_px * Q {
            let (mx, my) = self.bot_step(me, (tile_of(to.0), tile_of(to.1)));
            i.move_x = mx;
            i.move_y = my;
            if mx == 0 && my == 0 {
                i.move_x = sign(to.0 - me.x, Q);
                i.move_y = sign(to.1 - me.y, Q);
            }
        }
        // Face it with the arrows, and press E every other tick (a press is
        // an edge).
        i.aim_x = sign(to.0 - me.x, 3 * Q);
        i.aim_y = sign(to.1 - me.y, 3 * Q);
        if press && d <= (reach_px + 6) * Q {
            i.interact = self.elapsed.is_multiple_of(2);
        }
        i
    }

    // ----- fighting a snuffer ---------------------------------------------

    fn bot_fight(&self, id: u32, me: &Seat, f: &Foe, r: &Rules) -> Input {
        let d = dist(f.x - me.x, f.y - me.y);
        let toward = unit(f.x - me.x, f.y - me.y);
        let mut i = Input {
            ult: d < r.lock,
            ..Default::default()
        };
        let reach = (18 + 6) * Q;
        // The hood: light first, then the blade.
        if f.hooded && f.lit < 12 && self.inv.lantern && me.flame > 5 && d < FLARE_R * Q {
            i.fire = true;
        }
        let in_line = f.danger != Danger::None && f.danger.hits(me.x, me.y, KNIGHT_R + 2 * Q);
        match f.st {
            Fs::Tell | Fs::Strike if in_line => {
                if f.st == Fs::Strike || f.t <= PARRY_LEAD {
                    i.dash = true;
                } else {
                    // Out of the line, sideways.
                    let side = (-toward.1, toward.0);
                    let s = if id.is_multiple_of(2) { 1 } else { -1 };
                    i.move_x = sign(side.0 * s, Q / 4);
                    i.move_y = sign(side.1 * s, Q / 4);
                }
            }
            _ => {
                if d > reach {
                    i.move_x = sign(toward.0, Q / 4);
                    i.move_y = sign(toward.1, Q / 4);
                } else if d < 12 * Q {
                    i.move_x = -sign(toward.0, Q / 4);
                    i.move_y = -sign(toward.1, Q / 4);
                } else if f.vulnerable() {
                    i.swing = self.elapsed.is_multiple_of(2);
                    // A little circling keeps it from lunging into the face.
                    if f.st == Fs::Drift {
                        let side = (-toward.1, toward.0);
                        let s = if id.is_multiple_of(2) { 1 } else { -1 };
                        i.move_x = sign(side.0 * s, Q / 2);
                        i.move_y = sign(side.1 * s, Q / 2);
                    }
                }
            }
        }
        // Whatever is not walking faces its foe.
        i.aim_x = sign(f.x - me.x, 4 * Q);
        i.aim_y = sign(f.y - me.y, 4 * Q);
        i
    }

    // ----- the Warden -------------------------------------------------------

    fn bot_boss(&self, id: u32, me: &Seat, w: &Warden, r: &Rules) -> Input {
        let seats = self.seats.len() as u32;
        let (hx, hy) = w.heart();
        let d = dist(hx - me.x, hy - me.y);
        let toward = unit(hx - me.x, hy - me.y);
        let mut i = Input {
            ult: d < r.lock,
            ..Default::default()
        };
        let exposed = self.boss_exposed();
        // A snuffer on top of us comes first, unless the heart is open.
        if !exposed
            && let Some(f) = self
                .foes
                .iter()
                .filter(|f| f.alive() && f.kind == Fk::Snuffer && dist(f.x - me.x, f.y - me.y) < 45 * Q)
                .min_by_key(|f| dist(f.x - me.x, f.y - me.y))
        {
            return self.bot_fight(id, me, f, r);
        }
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        let rank = ids.iter().position(|&x| x == id).unwrap_or(0) as u32;
        let lighter = rank < w.need(seats);
        let open_for = w.open_for(self.elapsed, seats);
        let near = w.radius() + 12 * Q;
        // Be where the job is: lighters at range, strikers at the heart.
        let strike = exposed && (!lighter || open_for > 30) && w.st != Ws::Dying;
        let stand = if strike { near } else { 62 * Q };
        if d > stand + 6 * Q {
            let (mx, my) = self.bot_step(me, (tile_of(hx), tile_of(hy)));
            i.move_x = if mx != 0 { mx } else { sign(toward.0, Q / 4) };
            i.move_y = if my != 0 { my } else { sign(toward.1, Q / 4) };
        } else if d < stand - 8 * Q {
            i.move_x = -sign(toward.0, Q / 4);
            i.move_y = -sign(toward.1, Q / 4);
        } else {
            let side = (-toward.1, toward.0);
            let s = if id.is_multiple_of(2) { 1 } else { -1 };
            i.move_x = sign(side.0 * s, Q / 2);
            i.move_y = sign(side.1 * s, Q / 2);
        }
        i.aim_x = sign(hx - me.x, 4 * Q);
        i.aim_y = sign(hy - me.y, 4 * Q);
        // Light the heart: a tap of the lantern, every so often.
        if !exposed || (lighter && open_for < 24) {
            let mine = w.lit[(id as usize - 1).min(5)];
            let cost = r.flame / BOLT_COST_PARTS;
            // A light holds the heart for `EXPOSE_TICKS`: a fresh one is thrown
            // as the last runs low, not before.
            if lighter
                && me.flame >= cost
                && self.elapsed.saturating_sub(mine) + 28 > EXPOSE_TICKS
                && d < 110 * Q
                && self.line_open((me.x, me.y), (hx, hy))
            {
                i.fire = self.elapsed % 6 < 2;
            }
        }
        if strike && d <= near + 14 * Q {
            i.swing = self.elapsed.is_multiple_of(2);
        }
        // Its blows.
        let dm = dist(w.x - me.x, w.y - me.y);
        match (w.st, w.atk) {
            (Ws::Strike, Atk::Slam) => {
                let elapsed = i32::from(SLAM_TICKS - w.t);
                let ring = (12 + SLAM_SPEED * elapsed) * Q;
                // The ring is three or four ticks off: up and over.
                if (6 * Q..=20 * Q).contains(&(dm - ring)) && me.z == 0 && !matches!(me.act, Act::Hop | Act::Roll | Act::Item) {
                    // Over it: a backflip, locked on.
                    i.interact = self.elapsed.is_multiple_of(2);
                    i.ult = true;
                    i.move_x = 0;
                    i.move_y = 0;
                    i.swing = false;
                }
            }
            // A beam's tell: all but one leave the wedge; that one waits to
            // parry it.
            (Ws::Tell, Atk::Beam) if rank > 0 => {
                let mine = angle_of(me.x - w.x, me.y - w.y);
                let off = turn(w.aim, mine);
                if off.abs() < BEAM_HALF + 14 && dm < BEAM_LEN * Q {
                    // Out of it by the nearer edge, round the Warden.
                    let side = if off >= 0 { 1 } else { -1 };
                    let a = w.aim + side * (BEAM_HALF + 24);
                    let target = ((w.x + cos_q(a) * 70), (w.y + sin_q(a) * 70));
                    let (mx, my) = (sign(target.0 - me.x, Q), sign(target.1 - me.y, Q));
                    i.move_x = mx;
                    i.move_y = my;
                    i.swing = false;
                    i.fire = false;
                }
            }
            (Ws::Strike, Atk::Beam) => {
                let elapsed = i32::from(BEAM_TICKS - w.t);
                let beam = w.aim - BEAM_HALF + BEAM_HALF * 2 * elapsed / i32::from(BEAM_TICKS);
                let mine = angle_of(me.x - w.x, me.y - w.y);
                let beams = if w.phase >= 2 { 2 } else { 1 };
                // The beam sweeps toward larger angles: it is coming when it
                // is a few degrees behind our bearing.
                let coming = (0..beams).any(|b| (-10..=14).contains(&turn(beam + 180 * b, mine)));
                if coming && dm < BEAM_LEN * Q {
                    i.dash = true;
                    i.swing = false;
                    i.fire = false;
                    i.move_x = 0;
                    i.move_y = 0;
                }
            }
            _ => {}
        }
        i
    }

    // ----- the rooms ----------------------------------------------------------

    /// The nearest of `tiles` to the knight.
    fn bot_nearest(&self, me: &Seat, tiles: &[(i32, i32)]) -> Option<(i32, i32)> {
        tiles
            .iter()
            .copied()
            .min_by_key(|&(tx, ty)| dist(middle(tx) - me.x, middle(ty) - me.y))
    }

    fn bot_exit(&self, me: &Seat) -> Input {
        let exits = plan(usize::from(self.room)).find(b'Q');
        match self.bot_nearest(me, &exits) {
            Some((tx, ty)) => self.bot_goto(me, (middle(tx), middle(ty)), 3, false),
            None => Input::default(),
        }
    }

    fn bot_objective(&self, id: u32, me: &Seat, r: &Rules) -> Input {
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        let first = ids.first() == Some(&id);
        let rank = ids.iter().position(|&x| x == id).unwrap_or(0);
        let prop_pos = |kind: Pk, i: u8| -> Option<(i32, i32)> {
            self.prop_of(kind, i).map(|j| (self.props[j].x, self.props[j].y))
        };
        match (self.room, self.stage) {
            (0, 0) => {
                // Nothing awake: walk up to the snuffer.
                match self
                    .foes
                    .iter()
                    .filter(|f| f.alive())
                    .min_by_key(|f| dist(f.x - me.x, f.y - me.y))
                {
                    Some(f) => self.bot_goto(me, (f.x, f.y), 14, false),
                    None => Input::default(),
                }
            }
            (0, _) => self.bot_exit(me),
            (1, 0) => {
                // Three braziers, one ember: each fetches a flame, lights one.
                let unlit: Vec<usize> = (0..self.props.len())
                    .filter(|&j| self.props[j].kind == Pk::Brazier && !self.props[j].lit())
                    .collect();
                if unlit.is_empty() {
                    return Input::default();
                }
                if me.taper > 20 {
                    let j = unlit[rank % unlit.len()];
                    let p = &self.props[j];
                    return self.bot_goto(me, (p.x, p.y), 12, true);
                }
                match self.props.iter().find(|p| p.kind == Pk::Ember) {
                    Some(p) => self.bot_goto(me, (p.x, p.y), 12, true),
                    None => Input::default(),
                }
            }
            (1, 1) => match prop_pos(Pk::Chest, 0) {
                Some(c) if first => self.bot_goto(me, c, 14, true),
                Some(c) => self.bot_goto(me, (c.0, c.1 + 3 * TILE * Q), 8, false),
                None => Input::default(),
            },
            (1, 3) => {
                // South of the cage, the lantern thrown north through the bars.
                let spot = (middle(24), middle(7));
                let mut i = self.bot_goto(me, spot, 4, false);
                i.aim_x = 0;
                i.aim_y = -1;
                let cost = r.flame / BOLT_COST_PARTS;
                if dist(spot.0 - me.x, spot.1 - me.y) < 8 * Q && me.flame >= cost && me.lamp_cd == 0 {
                    i.fire = self.elapsed % 8 < 3;
                }
                i
            }
            (1, 2) => Input::default(),
            (1, _) => self.bot_exit(me),
            (2, 0) if self.props.iter().any(|p| p.kind == Pk::Gate && p.id == 1 && p.on == 1) => {
                self.bot_gallery(id, me)
            }
            // The second knight holds the second plate for the rest.
            (2, 0) if rank == 1 => match self.props.iter().find(|p| p.kind == Pk::Plate && p.id == 1) {
                Some(p) => self.bot_goto(me, (p.x, p.y), 2, false),
                None => Input::default(),
            },
            (2, 0) => {
                let block = self.props.iter().find(|p| p.kind == Pk::Block);
                let plate = self.props.iter().find(|p| p.kind == Pk::Plate);
                match (block, plate) {
                    (Some(b), Some(p)) if first && b.tx != p.tx => {
                        // Behind the block, then east into it.
                        let behind = (b.x - TILE * Q, b.y);
                        if me.x < b.x - 10 * Q && dist(behind.0 - me.x, behind.1 - me.y) > 6 * Q {
                            self.bot_goto(me, behind, 3, false)
                        } else {
                            Input {
                                move_x: 1,
                                aim_x: 1,
                                ..Default::default()
                            }
                        }
                    }
                    (_, Some(p)) => {
                        // The rest wait by the gate for it to rise.
                        self.bot_goto(me, (p.x + TILE * Q, p.y + 2 * TILE * Q), 10, false)
                    }
                    _ => Input::default(),
                }
            }
            (2, 1) => self.bot_exit(me),
            (2, _) => self.bot_gallery(id, me),
            (3, 0) => {
                // The dark: head for the nearest snuffer's glow.
                match self
                    .foes
                    .iter()
                    .filter(|f| f.alive())
                    .min_by_key(|f| dist(f.x - me.x, f.y - me.y))
                {
                    Some(f) => {
                        let mut i = self.bot_goto(me, (f.x, f.y), 30, false);
                        i.ult = dist(f.x - me.x, f.y - me.y) < r.lock;
                        i
                    }
                    None => Input::default(),
                }
            }
            (3, 1) => match prop_pos(Pk::Chest, 1) {
                Some(c) if first => self.bot_goto(me, c, 14, true),
                Some(c) => self.bot_goto(me, (c.0, c.1 + 3 * TILE * Q), 8, false),
                None => Input::default(),
            },
            (3, 2) => {
                if first {
                    let spot = (middle(14), middle(4));
                    let mut i = self.bot_goto(me, spot, 3, false);
                    if dist(spot.0 - me.x, spot.1 - me.y) < 6 * Q {
                        i.aim_x = 0;
                        i.aim_y = -1;
                        i.interact = self.elapsed.is_multiple_of(2);
                    }
                    i
                } else {
                    self.bot_goto(me, (middle(14), middle(8)), 10, false)
                }
            }
            (3, 3) => match prop_pos(Pk::Chest, 0) {
                Some(c) if first => {
                    let spot = (c.0, c.1 + TILE * Q);
                    let mut i = self.bot_goto(me, spot, 5, false);
                    if dist(spot.0 - me.x, spot.1 - me.y) < 8 * Q {
                        i.aim_x = 0;
                        i.aim_y = -1;
                        i.interact = self.elapsed.is_multiple_of(2);
                    }
                    i
                }
                _ => self.bot_goto(me, (middle(14), middle(8)), 10, false),
            },
            (3, 4) => {
                // The carrier opens the Boss Key door; the rest wait.
                let carrier = matches!(self.inv.boss, Hold::Carried(c, ..) if u32::from(c) == id);
                let door = plan(3).find(b'M').first().copied().unwrap_or((29, 8));
                if carrier {
                    let spot = (middle(door.0 - 1), middle(door.1));
                    let mut i = self.bot_goto(me, spot, 3, false);
                    if dist(spot.0 - me.x, spot.1 - me.y) < 8 * Q {
                        i.aim_x = 1;
                        i.aim_y = 0;
                        i.interact = self.elapsed.is_multiple_of(2);
                    }
                    i
                } else if let Hold::Lying(x, y) = self.inv.boss {
                    self.bot_goto(me, (x, y), 4, false)
                } else {
                    self.bot_goto(me, (middle(door.0 - 4), middle(door.1)), 10, false)
                }
            }
            (3, _) => self.bot_exit(me),
            (4, 2) => match self.drops.iter().find(|d| d.kind == Dk::Container) {
                Some(d) => self.bot_goto(me, (d.x, d.y), 4, false),
                None => Input::default(),
            },
            (4, 3) => match self.props.iter().find(|p| p.kind == Pk::Beacon) {
                Some(p) => {
                    let spot = (p.x, p.y + TILE * Q);
                    let mut i = self.bot_goto(me, spot, 8, false);
                    if dist(spot.0 - me.x, spot.1 - me.y) < 14 * Q {
                        i.aim_x = 0;
                        i.aim_y = -1;
                        i.interact = true;
                    }
                    i
                }
                None => Input::default(),
            },
            _ => Input::default(),
        }
    }

    /// The Gallery's zone two: one knight stands where the turret's bolt, met
    /// with a shield in its first ticks, goes to the crystal; the rest keep
    /// back so the turret picks that one.
    fn bot_gallery(&self, id: u32, me: &Seat) -> Input {
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        let rank = ids.iter().position(|&x| x == id).unwrap_or(0);
        if rank == 1 && self.stage == 0 {
            // Still holding the second plate.
            if let Some(p) = self.props.iter().find(|p| p.kind == Pk::Plate && p.id == 1) {
                return self.bot_goto(me, (p.x, p.y), 2, false);
            }
        }
        if rank != 0 {
            // Wait well back, off the turret's line.
            return self.bot_goto(me, (middle(19), middle(2 + (2 * rank as i32) % 9)), 10, false);
        }
        let spot = (middle(29), middle(5));
        let mut i = self.bot_goto(me, spot, 5, false);
        // The crystal is up and to the right: lock it, so a parried bolt
        // flies straight at it.
        let (cx, cy) = self
            .props
            .iter()
            .find(|p| p.kind == Pk::Crystal)
            .map_or((middle(35), middle(5)), |p| (p.x, p.y));
        i.aim_x = sign(cx - me.x, 3 * Q);
        i.aim_y = sign(cy - me.y, 3 * Q);
        if dist(spot.0 - me.x, spot.1 - me.y) < 9 * Q {
            // At the spot: stand, lock the crystal (a tap of the arrow moves
            // the lock off the turret), and meet the bolt with the shield.
            i.ult = true;
            i.move_x = 0;
            i.move_y = 0;
            let on_crystal = me
                .lock
                .is_some_and(|l| matches!(l, Lock::Prop(p) if self.props.get(usize::from(p)).is_some_and(|q| q.kind == Pk::Crystal)));
            let (ax, ay) = if on_crystal {
                (0, 0)
            } else if self.elapsed.is_multiple_of(2) {
                // The crystal lies above the turret on the screen.
                (0, -1)
            } else {
                (0, 0)
            };
            i.aim_x = ax;
            i.aim_y = ay;
            // A bolt close: up with the shield for its last ticks.
            let near = self.bolts.iter().any(|b| {
                b.hostile()
                    && dist(b.x - me.x, b.y - me.y) < 15 * Q
                    && dot(unit(b.vx, b.vy), unit(me.x - b.x, me.y - b.y)) > Q / 2
            });
            let clear = self
                .bolts
                .iter()
                .all(|b| !b.hostile() || dist(b.x - me.x, b.y - me.y) > 40 * Q);
            i.dash = on_crystal && (near || (me.shield > 0 && !clear && me.shield < 18));
        }
        i
    }

    /// After the Beacon: hold E and leave.
    fn bot_finale(&self) -> Input {
        if self.finale() {
            return Input {
                interact: self.elapsed.is_multiple_of(2),
                ..Default::default()
            };
        }
        Input::default()
    }
}
