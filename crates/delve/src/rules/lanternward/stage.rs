//! The adventure's rooms, one after another: what is in each, what each asks
//! of the party (`tick_stage`), the way through, the fallen and the retry.
use super::boss::Warden;
use super::ctx::Hold;
use super::foes::*;
use super::kit::*;
use super::world::*;
use super::*;

impl Session {
    /// Where seat `id` starts in this room: its own mark, wrapping.
    pub(super) fn start_of(&self, id: u32) -> (i32, i32) {
        let marks = plan(usize::from(self.room)).find(b'@');
        let (tx, ty) = marks[(id as usize - 1) % marks.len()];
        (middle(tx), middle(ty))
    }

    /// The party goes into room `room`: its things stand, its foes wait, and
    /// every seat is at its start (the fallen on their feet again, at the
    /// next brazier).
    pub(super) fn enter_room(&mut self, room: usize, r: &Rules) {
        self.room = room as u8;
        self.stage = 0;
        self.stage_t = 0;
        self.props = props::build(room);
        self.foes.clear();
        self.bolts.clear();
        self.drops.clear();
        self.fuses.clear();
        self.exiting = 0;
        self.wipe = 0;
        self.next_foe = 0;
        self.boss = None;
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let at = self.start_of(id);
            let k = self.seats.get_mut(&id).unwrap();
            k.x = at.0;
            k.y = at.1;
            k.safe = at;
            k.cam = at;
            k.z = 0;
            k.lock = None;
            k.act = Act::Free;
            k.t = 0;
            k.shield = 0;
            k.chain = 0;
            k.step = 0;
            k.charge = 0;
            k.push = (0, 0);
            k.pusht = 0;
            k.taper = 0;
            if k.hp == 0 {
                k.hp = REVIVE_HP.min(k.max_hp);
            }
            k.inv = SPAWN_INV;
        }
        self.provision_room(room);
        let _ = r;
    }

    /// The foes (and the Warden) a room opens with.
    fn provision_room(&mut self, room: usize) {
        let seats = self.seats.len().max(1) as u32;
        let p = plan(room);
        match room {
            0 => {
                let n = (1 + (seats - 1) / 3) as usize;
                for (tx, ty) in p.find(b'S').into_iter().take(n) {
                    self.spawn_foe(Fk::Snuffer, middle(tx), middle(ty), false);
                }
            }
            2 => {
                let n = if seats >= 4 { 2 } else { 1 };
                for (tx, ty) in p.find(b'T').into_iter().take(n) {
                    self.spawn_foe(Fk::Turret, middle(tx), middle(ty), false);
                }
            }
            3 => {
                let n = (2 + seats / 2) as usize;
                for (tx, ty) in p.find(b'S').into_iter().take(n) {
                    self.spawn_foe(Fk::Snuffer, middle(tx), middle(ty), true);
                }
            }
            4 => {
                if let Some((tx, ty)) = p.find(b'W').first().copied() {
                    self.boss = Some(Warden::new(middle(tx), middle(ty), seats));
                }
                // The arena's lamps burn until the Gloam falls.
                for q in self.props.iter_mut().filter(|q| q.kind == Pk::Brazier) {
                    q.on = 1;
                }
            }
            _ => {}
        }
    }

    /// The Lamp Hall's wave: the Gloam answers the Lantern.
    fn spawn_wave(&mut self) {
        let seats = self.seats.len().max(1) as u32;
        let n = (2 + (seats - 1) / 2).min(4) as usize;
        for (tx, ty) in plan(1).find(b'S').into_iter().take(n) {
            self.spawn_foe(Fk::Snuffer, middle(tx), middle(ty), false);
        }
    }

    pub(super) fn prop_of(&self, kind: Pk, id: u8) -> Option<usize> {
        self.props.iter().position(|p| p.kind == kind && p.id == id)
    }

    fn all_lit(&self, kind: Pk) -> bool {
        let mut any = false;
        for p in self.props.iter().filter(|p| p.kind == kind) {
            any = true;
            if !p.lit() {
                return false;
            }
        }
        any
    }

    fn foes_down(&self) -> bool {
        self.foes.iter().all(|f| !f.alive())
    }

    /// Whether the way on is open.
    pub fn exit_open(&self) -> bool {
        match self.room {
            0 => self.stage >= 1,
            1 => self.stage >= 4,
            2 => self.stage >= 1,
            3 => self.stage >= 4,
            _ => false,
        }
    }

    /// One tick of the things in the room.
    pub(super) fn tick_props(&mut self, ev: &mut Events) {
        // A plate is pressed by a knight on the ground or a block on it.
        let mut revealed = None;
        let weights: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|k| k.up() && k.z == 0)
            .map(|k| (k.x, k.y))
            .chain(
                self.props
                    .iter()
                    .filter(|p| p.kind == Pk::Block)
                    .map(|p| (p.x, p.y)),
            )
            .collect();
        let flares: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|k| k.flaring())
            .map(|k| (k.x, k.y))
            .collect();
        for i in 0..self.props.len() {
            let p = &mut self.props[i];
            match p.kind {
                Pk::Brazier
                    if p.t > 0 => {
                        p.t -= 1;
                        if p.t == 0 && p.on == 0 {
                            let (x, y) = (p.x, p.y);
                            self.fx(Fxk::Puff, x, y, 0);
                            ev.sound("step_ash");
                        }
                    }
                Pk::Plate => {
                    let was = p.on;
                    let (x, y) = (p.x, p.y);
                    p.on = u8::from(weights.iter().any(|&(wx, wy)| dist(wx - x, wy - y) < PLATE_R * Q));
                    if was != p.on {
                        ev.sound(if p.on == 1 { "wall_up" } else { "step_stone" });
                    }
                }
                Pk::Block => {
                    if p.on > 0 {
                        let (dx, dy) = match p.on {
                            1 => (1, 0),
                            2 => (-1, 0),
                            3 => (0, 1),
                            _ => (0, -1),
                        };
                        p.x += dx * Q;
                        p.y += dy * Q;
                        p.t = p.t.saturating_sub(1);
                        if p.t == 0 {
                            p.x = middle(tile_of(p.x));
                            p.y = middle(tile_of(p.y));
                            p.tx = tile_of(p.x) as u8;
                            p.ty = tile_of(p.y) as u8;
                            p.on = 0;
                            ev.sound("rock_land");
                        }
                    } else {
                        p.t = p.t.saturating_sub(1);
                    }
                }
                Pk::Sigil if p.on == 0 => {
                    let (x, y) = (p.x, p.y);
                    if flares.iter().any(|&(fx, fy)| dist(fx - x, fy - y) < FLARE_R * Q) {
                        p.t += 1;
                        if p.t >= SIGIL_TICKS {
                            p.on = 1;
                            revealed = Some((x, y));
                        }
                    } else {
                        p.t = p.t.saturating_sub(1);
                    }
                }
                Pk::Fissure => p.t = p.t.saturating_sub(1),
                _ => {}
            }
        }
        // A sigil found opens what it was hiding.
        if let Some((x, y)) = revealed {
            self.set_gate(2, true);
            self.solved(x, y, ev);
            ev.sound("door_open");
        }
    }

    /// A gate that closed or a block that slid over a body puts it out on the
    /// nearest open ground: nobody is walled in.
    pub(super) fn unstick(&mut self) {
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let (x, y) = {
                let k = &self.seats[&id];
                (k.x, k.y)
            };
            if !self.clear(x, y, KNIGHT_R / 2) {
                let (nx, ny) = self.nudge(x, y, KNIGHT_R);
                let k = self.seats.get_mut(&id).unwrap();
                (k.x, k.y) = (nx, ny);
            }
        }
    }

    /// What each room asks, and what it gives.
    pub(super) fn tick_stage(&mut self, r: &Rules, ev: &mut Events) {
        self.stage_t = (self.stage_t + 1).min(super::LIMIT);
        match self.room {
            0 => {
                if self.stage == 0 && self.foes_down() {
                    self.stage = 1;
                    self.open_exit(ev);
                }
            }
            1 => self.stage_lamp_hall(ev),
            2 => self.stage_gallery(ev),
            3 => self.stage_vault(ev),
            _ => self.stage_warden(r, ev),
        }
        // The way on: stand in it.
        if self.exiting == 0 && self.exit_open() && self.room < 4 {
            let hit = self.seats.values().any(|k| {
                k.up() && k.z == 0 && plan(usize::from(self.room)).at(tile_of(k.x), tile_of(k.y)) == b'Q'
            });
            if hit {
                self.exiting = EXIT_TICKS;
                ev.sound("door_pass");
            }
        } else if self.exiting > 0 {
            self.exiting -= 1;
            if self.exiting == 0 {
                let next = usize::from(self.room) + 1;
                self.enter_room(next, r);
                ev.sound("descend");
            }
        }
    }

    /// The exit gate swings open.
    fn open_exit(&mut self, ev: &mut Events) {
        if self.set_gate(0, true) {
            let at = self
                .props
                .iter()
                .find(|p| p.kind == Pk::Gate && p.id == 0)
                .map_or((0, 0), |p| (p.x, p.y));
            self.solved(at.0, at.1, ev);
            ev.sound("door_open");
        }
    }

    fn stage_lamp_hall(&mut self, ev: &mut Events) {
        match self.stage {
            // Three braziers, all alight at once, from one ember.
            0
                if self.all_lit(Pk::Brazier) => {
                    self.stage = 1;
                    let at = self.prop_of(Pk::Chest, 0).map_or((0, 0), |i| (self.props[i].x, self.props[i].y));
                    self.set_gate(1, true);
                    self.solved(at.0, at.1, ev);
                    ev.sound("door_open");
                }
            // The niche stands open; the Lantern is in the chest.
            1
                if self.prop_of(Pk::Chest, 0).is_some_and(|i| self.props[i].on == 1) => {
                    self.stage = 2;
                    self.stage_t = 0;
                    self.spawn_wave();
                    ev.sound("wave_gate");
                }
            // The Gloam answers.
            2
                if self.stage_t > 20 && self.foes_down() => {
                    self.stage = 3;
                    ev.sound("play_card");
                }
            // One light left to throw.
            3
                if self.prop_of(Pk::Great, 0).is_some_and(|i| self.props[i].on == 1) => {
                    self.stage = 4;
                    self.open_exit(ev);
                }
            _ => {}
        }
    }

    fn stage_gallery(&mut self, ev: &mut Events) {
        // Two plates for two or more knights: the block (or a knight) on the
        // first, another knight on the second; one plate alone for one.
        let held = |id: u8| self.props.iter().any(|p| p.kind == Pk::Plate && p.id == id && p.on == 1);
        let plate = held(0) && (held(1) || self.seats.len() < 2);
        let crystal = self.props.iter().any(|p| p.kind == Pk::Crystal && p.on == 1);
        // The plate-gate holds open only while it is held, or for good once
        // the crystal has woken.
        let was = self.props.iter().any(|p| p.kind == Pk::Gate && p.id == 1 && p.on == 1);
        let want = plate || crystal;
        if self.set_gate(1, want) && want != was {
            let at = self
                .props
                .iter()
                .find(|p| p.kind == Pk::Gate && p.id == 1)
                .map_or((0, 0), |p| (p.x, p.y));
            ev.sound(if want { "door_open" } else { "door_seal" });
            if want {
                self.fx(Fxk::Dust, at.0, at.1, 0);
            }
        }
        if self.stage == 0 && crystal {
            self.stage = 1;
            self.open_exit(ev);
        }
    }

    fn stage_vault(&mut self, ev: &mut Events) {
        match self.stage {
            0
                if self.foes_down() => {
                    self.stage = 1;
                    if let Some(i) = self.prop_of(Pk::Chest, 1) {
                        self.props[i].on = 0;
                        let (x, y) = (self.props[i].x, self.props[i].y);
                        self.fx(Fxk::Light, x, y, 0);
                        self.solved(x, y, ev);
                    }
                    // The spark the last snuffer gave back lights the
                    // vault's braziers for good.
                    for p in self.props.iter_mut().filter(|p| p.kind == Pk::Brazier) {
                        p.on = 1;
                    }
                }
            1
                if self.prop_of(Pk::Chest, 1).is_some_and(|i| self.props[i].on == 1) => {
                    self.stage = 2;
                }
            2
                if self.props.iter().any(|p| p.kind == Pk::Lock && p.on == 1) => {
                    self.stage = 3;
                }
            3
                if self.prop_of(Pk::Chest, 0).is_some_and(|i| self.props[i].on == 1) => {
                    self.stage = 4;
                    // The way out is the Boss Key door; its carrier opens
                    // it (`Ctx::Unlock`).
                    self.stage = 4;
                }
            _ => {}
        }
        if self.props.iter().any(|p| p.kind == Pk::Boss && p.on == 1) && self.stage >= 4 {
            self.stage = 5;
        }
    }

    /// Drops: hearts mend, gems and bombs are kept, the Boss Key is carried.
    pub(super) fn pickups(&mut self, r: &Rules, ev: &mut Events) {
        // The Boss Key follows its carrier; a carrier who falls or leaves
        // puts it down where they stood.
        if let Hold::Carried(id, _, _) = self.inv.boss {
            match self.seats.get(&u32::from(id)).filter(|k| k.up()) {
                Some(k) => self.inv.boss = Hold::Carried(id, k.x, k.y),
                None => {
                    if let Hold::Carried(_, x, y) = self.inv.boss {
                        self.inv.boss = Hold::Lying(x, y);
                    }
                }
            }
        }
        if let Hold::Lying(x, y) = self.inv.boss
            && let Some((&id, _)) = self
                .seats
                .iter()
                .find(|(_, k)| k.up() && dist(k.x - x, k.y - y) < PICK_R * Q)
        {
            self.inv.boss = Hold::Carried(id as u8, x, y);
            ev.sound("card_pickup");
        }
        let mut i = 0;
        while i < self.drops.len() {
            self.drops[i].age = self.drops[i].age.saturating_add(1);
            let d = self.drops[i].clone();
            let taker = self
                .seats
                .iter()
                .filter(|(_, k)| k.up() && dist(k.x - d.x, k.y - d.y) < PICK_R * Q)
                .map(|(&id, _)| id)
                .next();
            let mut gone = d.age > DROP_LIFE && d.kind != Dk::Container;
            if let Some(id) = taker {
                match d.kind {
                    Dk::Heart => {
                        let k = self.seats.get_mut(&id).unwrap();
                        if k.hp < k.max_hp {
                            k.hp = (k.hp + HEART).min(k.max_hp);
                            ev.sound("potion");
                            gone = true;
                        }
                    }
                    Dk::Gem => {
                        self.seats.get_mut(&id).unwrap().gems += 1;
                        ev.sound("gold");
                        gone = true;
                    }
                    Dk::Bomb => {
                        let k = self.seats.get_mut(&id).unwrap();
                        k.bombs = (k.bombs + 1).min(BOMB_CAP);
                        ev.sound("spoil_pickup");
                        gone = true;
                    }
                    Dk::Container => {
                        if let Some(mut k) = self.seats.remove(&id) {
                            self.grant(id, &mut k, Item::Container, r, ev);
                            self.seats.insert(id, k);
                        }
                        self.stage = self.stage.max(3);
                        gone = true;
                    }
                }
            }
            if gone {
                self.drops.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// The fallen: a friend holding E gets them up; a long wait brings them
    /// to the next brazier on their own.
    pub(super) fn revive_pass(&mut self, ev: &mut Events) {
        let downed: Vec<u32> = self
            .seats
            .iter()
            .filter(|(_, k)| k.act == Act::Down)
            .map(|(&id, _)| id)
            .collect();
        for d in downed {
            let (dx, dy) = {
                let k = &self.seats[&d];
                (k.x, k.y)
            };
            let helpers = self
                .seats
                .iter()
                .filter(|(id, k)| {
                    **id != d
                        && k.up()
                        && k.held & B_CTX != 0
                        && k.act == Act::Free
                        && dist(k.x - dx, k.y - dy) < REVIVE_REACH * Q
                })
                .count() as u32;
            let k = self.seats.get_mut(&d).unwrap();
            k.rev = if helpers > 0 {
                k.rev + helpers.min(2)
            } else {
                k.rev.saturating_sub(2)
            };
            let auto = k.t >= DOWN_AUTO;
            if k.rev >= REVIVE_HOLD || auto {
                let at = if auto { None } else { Some((k.x, k.y)) };
                k.hp = if auto { REVIVE_HP / 2 } else { REVIVE_HP }.min(k.max_hp).max(1);
                k.act = Act::Free;
                k.t = 0;
                k.rev = 0;
                k.inv = SPAWN_INV;
                let (mut x, mut y) = (k.x, k.y);
                if at.is_none() {
                    // Carried to the room's resting brazier.
                    let start = self.start_of(d);
                    (x, y) = start;
                    let k = self.seats.get_mut(&d).unwrap();
                    k.x = x;
                    k.y = y;
                    k.safe = start;
                }
                self.fx(Fxk::Light, x, y, 0);
                ev.sound("potion");
            }
        }
    }

    /// Every knight down: a breath, then the party is back at the room's
    /// start with the world as it was.
    pub(super) fn tick_wipe(&mut self, r: &Rules, ev: &mut Events) {
        let all_down = !self.seats.is_empty() && self.seats.values().all(|k| !k.up());
        if !all_down {
            self.wipe = 0;
            return;
        }
        self.wipe += 1;
        if self.wipe == 1 {
            ev.sound("boss_fall");
        }
        if self.wipe >= WIPE_TICKS {
            self.retry(r, ev);
        }
    }

    /// Back to the room's start: hearts full, foes as the encounter opened,
    /// every solved thing still solved.
    pub(super) fn retry(&mut self, r: &Rules, ev: &mut Events) {
        self.wipe = 0;
        self.tries = self.tries.saturating_add(1);
        self.foes.clear();
        self.bolts.clear();
        self.fuses.clear();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let at = self.start_of(id);
            let k = self.seats.get_mut(&id).unwrap();
            k.x = at.0;
            k.y = at.1;
            k.safe = at;
            k.cam = at;
            k.hp = k.max_hp;
            k.act = Act::Free;
            k.t = 0;
            k.z = 0;
            k.lock = None;
            k.shield = 0;
            k.chain = 0;
            k.step = 0;
            k.charge = 0;
            k.rev = 0;
            k.push = (0, 0);
            k.pusht = 0;
            k.inv = SPAWN_INV;
            k.carry = Carry::Nothing;
            k.flame = if self.inv.lantern { r.flame } else { 0 };
        }
        // Carried keys are put down at the start.
        if let Hold::Carried(id, _, _) = self.inv.boss {
            let at = self.seats.get(&u32::from(id)).map_or((0, 0), |k| (k.x, k.y));
            self.inv.boss = Hold::Carried(id, at.0, at.1);
        }
        match (self.room, self.stage) {
            (0, 0) | (3, 0) | (2, _) => self.provision_room(usize::from(self.room)),
            (1, 2) => self.spawn_wave(),
            (4, _) => self.reset_boss_phase(),
            _ => {}
        }
        ev.sound("play_card");
        self.flash = self.flash.max(8);
    }

    /// The tick's leftovers age out.
    pub(super) fn age(&mut self) {
        for f in &mut self.fx {
            f.t = f.t.saturating_add(1);
        }
        self.fx.retain(|f| f.t < f.kind.life());
        self.shake = self.shake.saturating_sub(1);
        self.flash = self.flash.saturating_sub(1);
        self.cue = match self.cue {
            Some((x, y, t)) if t > 1 => Some((x, y, t - 1)),
            _ => None,
        };
        if let Some(b) = &mut self.banner {
            b.t = b.t.saturating_sub(1);
        }
        if self.banner.is_some_and(|b| b.t == 0) {
            self.banner = None;
        }
    }
}
