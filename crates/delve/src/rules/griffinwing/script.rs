//! The three beats' scripts: what appears, and when, along the rail. A beat's
//! clock runs from its first tick; an event at `at` appears then, ahead of
//! the formation (a prop stands where the rail will bring it to the formation
//! at its own arrival tick, so the author thinks in arrivals). The teaching
//! comes first: aim, then hold, then the roll, then the lamps. More knights
//! at the table means more of each wave (`plus` for every two seats beyond
//! the first), never less.
use super::*;
use std::sync::OnceLock;

/// How many ticks the rail takes to carry a thing from where it appears to
/// the formation.
pub const LEAD_TICKS: u32 = (LEAD / PACE) as u32;

/// One thing the script does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// A flight of snuffers: how many, how many more per two seats, the
    /// shape (0 line, 1 vee, 2 file, 3 crossing, 4 weaving), where, and how
    /// they fly (0 drift, 1 weave, 3 shoot).
    Wisps {
        n: u8,
        plus: u8,
        shape: u8,
        x: i16,
        z: i16,
        mode: u8,
    },
    /// Kites crossing in a file from one side (-1 left, 1 right).
    Convoy {
        n: u8,
        plus: u8,
        from: i8,
        z: i16,
    },
    /// A kite that hunts a wingman and latches onto its tail.
    Hunter {
        side: i8,
    },
    /// A lantern tower on a ledge.
    Turret {
        x: i16,
        z: i16,
    },
    /// A ridge with dive-bombers waiting behind it.
    Ridge {
        x: i16,
        bombers: u8,
        plus: u8,
    },
    /// A bonus gate, a hearth ring, a rock arch, a pair of shutters: where
    /// the rail brings them (`rel` from the beat's first point).
    Gate {
        rel: i32,
        x: i16,
        z: i16,
        bonus: bool,
    },
    Ring {
        rel: i32,
        x: i16,
        z: i16,
    },
    Arch {
        rel: i32,
        x: i16,
        z: i16,
        w: i16,
        h: i16,
    },
    Shutter {
        rel: i32,
        phase: u16,
    },
    /// The wing's lead shouts.
    Say(Line),
    /// The Wyrm rises.
    Beast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ev {
    pub at: u32,
    pub act: Act,
}

fn arrive(t: u32) -> i32 {
    t as i32 * PACE
}

struct Beat(Vec<Ev>);

impl Beat {
    fn at(&mut self, at: u32, act: Act) {
        self.0.push(Ev { at, act });
    }
    /// A prop that arrives at the formation on tick `t`.
    fn prop(&mut self, t: u32, make: impl Fn(i32) -> Act) {
        self.0.push(Ev {
            at: t.saturating_sub(LEAD_TICKS),
            act: make(arrive(t)),
        });
    }
    fn gate(&mut self, t: u32, x: i16, z: i16) {
        self.prop(t, |rel| Act::Gate {
            rel,
            x,
            z,
            bonus: true,
        });
    }
    fn ring(&mut self, t: u32, x: i16, z: i16) {
        self.prop(t, |rel| Act::Ring { rel, x, z });
    }
    fn arch(&mut self, t: u32, x: i16, z: i16, w: i16, h: i16) {
        self.prop(t, |rel| Act::Arch { rel, x, z, w, h });
    }
    fn shutter(&mut self, t: u32, phase: u16) {
        self.prop(t, |rel| Act::Shutter { rel, phase });
    }
    fn say(&mut self, at: u32, line: Line) {
        self.at(at, Act::Say(line));
    }
    fn wisps(&mut self, at: u32, n: u8, plus: u8, shape: u8, x: i16, z: i16, mode: u8) {
        self.at(
            at,
            Act::Wisps {
                n,
                plus,
                shape,
                x,
                z,
                mode,
            },
        );
    }
    fn done(mut self) -> Vec<Ev> {
        self.0.sort_by_key(|e| e.at);
        self.0
    }
}

/// Thunderhead Run: aim, then hold and let go, the first gates, a rock arch,
/// and the first kite that latches on.
fn thunderhead() -> Vec<Ev> {
    let mut b = Beat(Vec::new());
    b.say(30, Line::Hello);
    b.wisps(110, 3, 1, 0, 0, 20, 0);
    b.gate(330, -26, 30);
    b.gate(345, 0, 38);
    b.gate(360, 26, 30);
    b.wisps(330, 5, 2, 1, 0, 25, 0);
    b.say(350, Line::Hold);
    b.ring(700, -30, 40);
    b.ring(700, 30, 40);
    b.say(560, Line::Ring);
    b.arch(860, 0, 20, 24, 15);
    b.say(700, Line::Arch);
    b.wisps(800, 4, 2, 3, 0, 15, 0);
    b.wisps(850, 4, 2, 3, 0, 30, 0);
    b.at(1010, Act::Hunter { side: -1 });
    b.wisps(1180, 6, 2, 4, 0, 20, 1);
    b.wisps(1240, 6, 2, 4, 0, 32, 1);
    b.wisps(1100, 4, 2, 0, 0, 25, 3);
    b.wisps(560, 4, 2, 2, 0, 20, 0);
    b.gate(1500, 0, 36);
    b.gate(1515, 30, 15);
    b.gate(1530, 0, -10);
    b.gate(1545, -30, 15);
    b.gate(1560, 0, 36);
    b.at(1380, Act::Hunter { side: 1 });
    b.wisps(1400, 3, 2, 0, 0, 25, 0);
    b.wisps(1560, 3, 1, 0, 0, 25, 3);
    b.at(1500, Act::Hunter { side: -1 });
    b.ring(1700, 0, 40);
    b.done()
}

/// The Ruined Causeway: turrets teach the roll, kite convoys pay off the
/// lock, dive-bombers come from behind a ridge, shutters close on a clock.
fn causeway() -> Vec<Ev> {
    let mut b = Beat(Vec::new());
    b.say(30, Line::Ruins);
    b.say(80, Line::Turret);
    b.at(90, Act::Turret { x: -95, z: -20 });
    b.say(150, Line::Roll);
    b.at(330, Act::Turret { x: 95, z: -10 });
    b.at(345, Act::Turret { x: -100, z: 0 });
    b.say(500, Line::Convoy);
    b.at(
        520,
        Act::Convoy {
            n: 6,
            plus: 2,
            from: 1,
            z: 25,
        },
    );
    b.gate(800, -26, 25);
    b.gate(815, 0, 36);
    b.gate(830, 26, 25);
    b.arch(900, -10, 0, 22, 14);
    b.say(750, Line::Bombers);
    b.at(
        760,
        Act::Ridge {
            x: -70,
            bombers: 2,
            plus: 2,
        },
    );
    b.say(980, Line::Shutter);
    b.shutter(1190, 0);
    b.shutter(1390, 75);
    b.at(
        1150,
        Act::Convoy {
            n: 5,
            plus: 2,
            from: -1,
            z: 15,
        },
    );
    b.at(1170, Act::Turret { x: 95, z: -20 });
    b.wisps(620, 4, 2, 0, 0, 25, 3);
    b.wisps(1300, 5, 2, 1, 0, 25, 3);
    b.at(1650, Act::Turret { x: -100, z: -10 });
    b.at(1670, Act::Turret { x: 100, z: -15 });
    b.at(
        1400,
        Act::Ridge {
            x: 70,
            bombers: 2,
            plus: 2,
        },
    );
    b.at(
        1450,
        Act::Ridge {
            x: -70,
            bombers: 1,
            plus: 2,
        },
    );
    b.ring(1750, -30, 30);
    b.ring(1750, 30, 30);
    b.at(1780, Act::Hunter { side: 1 });
    b.at(1850, Act::Hunter { side: -1 });
    b.at(1990, Act::Hunter { side: 1 });
    b.at(2050, Act::Hunter { side: -1 });
    b.wisps(1900, 4, 2, 0, 0, 28, 3);
    b.gate(2250, 0, 36);
    b.gate(2265, -30, 18);
    b.gate(2280, 0, 0);
    b.gate(2295, 30, 18);
    b.done()
}

/// The Wyrm's Roost: a short approach, then the Wyrm.
fn roost() -> Vec<Ev> {
    let mut b = Beat(Vec::new());
    b.say(30, Line::Roost);
    b.wisps(110, 4, 2, 0, 0, 25, 3);
    b.at(250, Act::Hunter { side: -1 });
    b.at(280, Act::Hunter { side: 1 });
    b.gate(520, -20, 25);
    b.ring(500, 0, 30);
    b.at(rail::BOSS_AT, Act::Beast);
    b.done()
}

/// The scripts, built once.
pub fn script(stage: u8) -> &'static [Ev] {
    static SCRIPTS: OnceLock<[Vec<Ev>; 3]> = OnceLock::new();
    &SCRIPTS.get_or_init(|| [thunderhead(), causeway(), roost()])[usize::from(stage.min(2))]
}

/// Where a beat's script stands when it comes back to the tick `cp`.
pub fn resume(stage: u8, cp: u32) -> u16 {
    script(stage).iter().take_while(|e| e.at < cp).count() as u16
}

impl Session {
    /// Run the events that are due.
    pub(super) fn run_script(&mut self, _n: &Nums) {
        let script = script(self.stage);
        while let Some(ev) = script.get(usize::from(self.next)) {
            if ev.at > self.t {
                break;
            }
            self.next += 1;
            self.act(ev.act);
        }
    }

    /// A centred spread: the `k`th of `count`, in half steps.
    fn spread(k: i32, count: i32) -> i32 {
        2 * k - (count - 1)
    }

    pub(super) fn act(&mut self, act: Act) {
        let anchor = self.anchor();
        let far = anchor + LEAD;
        let seats = i32::from(self.pressure());
        let origin = rail::ORIGIN[usize::from(self.stage.min(2))];
        match act {
            Act::Wisps {
                n,
                plus,
                shape,
                x,
                z,
                mode,
            } => {
                let count = i32::from(n) + (seats - 1) * i32::from(plus) / 2;
                for k in 0..count {
                    let sp = Session::spread(k, count);
                    let (dx, dz, ds, vx) = match shape {
                        0 => (sp * dm(16), 0, 0, 0),
                        1 => (sp * dm(13), -sp.abs() * dm(3), sp.abs() * dm(10), 0),
                        2 => (0, 0, k * dm(34), 0),
                        3 => {
                            let side = if k % 2 == 0 { 1 } else { -1 };
                            (side * dm(95), (k % 3) * dm(10), k * dm(22), -side * 44)
                        }
                        _ => (sp * dm(15), (k % 2) * dm(12), k * dm(16), 0),
                    };
                    self.spawn_foe(
                        Kind::Wisp,
                        (far + ds, dm(i32::from(x)) + dx, dm(i32::from(z)) + dz),
                        (-15, vx, 0),
                        St::Fly,
                        mode,
                        (k * 7) as u8,
                    );
                }
                if mode == 3 {
                    for f in self.foes.iter_mut().filter(|f| f.mode == 3 && f.ticks == 0) {
                        f.ticks = 60;
                    }
                }
            }
            Act::Convoy { n, plus, from, z } => {
                let count = i32::from(n) + (seats - 1) * i32::from(plus) / 2;
                for k in 0..count {
                    let side = i32::from(from);
                    self.spawn_foe(
                        Kind::Kite,
                        (far + k * dm(26), side * dm(95), dm(i32::from(z))),
                        (-15, -side * 46, 0),
                        St::Fly,
                        0,
                        (k * 5) as u8,
                    );
                }
            }
            Act::Hunter { side } => {
                self.spawn_foe(
                    Kind::Kite,
                    (far - dm(40), i32::from(side) * dm(90), dm(40)),
                    (0, 0, 0),
                    St::Hunt,
                    1,
                    0,
                );
            }
            Act::Turret { x, z } => {
                if let Some(id) = self.spawn_foe(
                    Kind::Turret,
                    (far, dm(i32::from(x)), dm(i32::from(z))),
                    (0, 0, 0),
                    St::Rest,
                    0,
                    (self.next % 5) as u8,
                ) {
                    if let Some(f) = self.foes.iter_mut().find(|f| f.id == id) {
                        f.ticks = 30;
                    }
                }
            }
            Act::Ridge { x, bombers, plus } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Ridge,
                        s: far,
                        x: dm(i32::from(x)),
                        z: dm(-30),
                        w: dm(30),
                        h: dm(55),
                        len: dm(60),
                        taken: 0,
                        phase: 0,
                    });
                }
                let count = i32::from(bombers) + (seats - 1) * i32::from(plus) / 2;
                for k in 0..count {
                    self.spawn_foe(
                        Kind::Bomber,
                        (
                            far + dm(10) + k * dm(8),
                            dm(i32::from(x)) + (k % 3 - 1) * dm(14),
                            dm(-6),
                        ),
                        (0, 0, 0),
                        St::Hide,
                        0,
                        (k * 3) as u8,
                    );
                }
            }
            Act::Gate { rel, x, z, bonus } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Gate,
                        s: origin + rel,
                        x: dm(i32::from(x)),
                        z: dm(i32::from(z)),
                        w: dm(20),
                        h: 0,
                        len: 0,
                        taken: 0,
                        phase: u16::from(!bonus),
                    });
                    if bonus {
                        self.beat.offered += 100 * seats as u32;
                    }
                }
            }
            Act::Ring { rel, x, z } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Ring,
                        s: origin + rel,
                        x: dm(i32::from(x)),
                        z: dm(i32::from(z)),
                        w: dm(22),
                        h: 0,
                        len: 0,
                        taken: 0,
                        phase: 0,
                    });
                }
            }
            Act::Arch { rel, x, z, w, h } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Arch,
                        s: origin + rel,
                        x: dm(i32::from(x)),
                        z: dm(i32::from(z)),
                        w: dm(i32::from(w)),
                        h: dm(i32::from(h)),
                        len: dm(30),
                        taken: 0,
                        phase: 0,
                    });
                }
            }
            Act::Shutter { rel, phase } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Shutter,
                        s: origin + rel,
                        x: 0,
                        z: 0,
                        w: 0,
                        h: 0,
                        len: dm(30),
                        taken: 0,
                        phase,
                    });
                }
            }
            Act::Say(line) => {
                let who = self.voice();
                self.say(who, line);
            }
            Act::Beast => self.rise(),
        }
    }

    /// The warm fly-through after the Wyrm: a procession of gold gates and
    /// hearth rings, lit for the way home. It pays nothing and offers
    /// nothing; it is the party's welcome.
    pub(super) fn victory_gates(&mut self) {
        let origin = self.anchor() + dm(150);
        for k in 0..9 {
            let (x, z) = (
                tri(k as u32 * 40, 120, dm(30)),
                dm(30) + tri(k as u32 * 25, 90, dm(18)),
            );
            if self.props.len() < MAX_PROPS {
                self.props.push(Prop {
                    thing: if k % 3 == 2 { Thing::Ring } else { Thing::Gate },
                    s: origin + k * dm(75),
                    x,
                    z,
                    w: dm(24),
                    h: 0,
                    len: 0,
                    taken: 0,
                    phase: 1,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_beat_has_a_script_sorted_by_its_clock_and_inside_its_beat() {
        for stage in 0..3u8 {
            let s = script(stage);
            assert!(!s.is_empty());
            assert!(s.windows(2).all(|w| w[0].at <= w[1].at), "stage {stage}");
            if stage < 2 {
                let len = rail::LEN[usize::from(stage)];
                assert!(s.iter().all(|e| e.at < len), "stage {stage}");
            }
        }
        assert!(
            script(2)
                .iter()
                .any(|e| e.act == Act::Beast && e.at == rail::BOSS_AT)
        );
    }

    #[test]
    fn a_script_resumes_at_its_checkpoint() {
        assert_eq!(resume(1, 0), 0);
        let n = script(2).len() as u16;
        assert_eq!(
            resume(2, rail::BOSS_AT),
            n - 1,
            "the Wyrm is the last event"
        );
    }
}
