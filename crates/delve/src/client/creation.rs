//! An opt-in local Creator. The wire gets neutral hands throughout.
use crate::render::creation::Floor;
use crate::rules::creation::{Action, DriveInput, Ground, Session, Step};
use crate::rules::rite::{self, Made};
use angel_net::input::{Hands, button};

/// The rite's invitation to the Creator on a newcomer's own screen: a slip for the first seconds
/// after they arrive, then a corner hint for as long as the rite is owed. It never holds a key
/// but C while N is held, never blocks the knight and is the client's alone.
#[derive(Default)]
pub(super) struct Invite {
    owed: bool,
    since: u64,
    /// The Creator has been opened from it: the slip does not come back this session.
    done: bool,
}

/// How long the slip shows before it puts itself off, in ticks.
const SLIP_TICKS: u64 = 10 * crate::rules::HZ as u64;

impl Invite {
    /// The run says whether the rite is owed now; a rite newly owed starts the slip.
    pub fn update(&mut self, owed: bool, tick: u64) {
        if owed && !self.owed {
            self.since = tick;
            self.done = false;
        }
        self.owed = owed;
    }

    /// Whether N held and C opens the Creator for the rite now.
    pub fn up(&self) -> bool {
        self.owed
    }

    pub fn opened(&mut self) {
        self.done = true;
    }

    /// What to draw at `tick`: none, the slip (with its age), or the corner hint.
    pub fn shown(&self, tick: u64) -> Option<(u64, bool)> {
        let age = tick.saturating_sub(self.since);
        self.owed.then_some((age, !self.done && age < SLIP_TICKS))
    }
}

#[derive(Default)]
pub(super) struct Editor {
    pub session: Session,
    /// The Test drive's fixture floor, stood up the first time it is walked.
    pub floor: Option<Floor>,
    pub invite: Invite,
    /// A thing brought to life, on its way to the host: `(kind, body)`.
    pub outbox: Vec<(u32, Vec<u8>)>,
    /// The grant tick of the piece last sent, so one grant is one action.
    sent: Option<u64>,
    repeat_at: u64,
}

impl Editor {
    pub fn hands(&mut self, hands: &Hands, tick: u64) {
        // Only a dev build can stand a fixture floor up for the Test drive.
        self.session.can_drive = cfg!(any(test, feature = "testing"));
        let on = |bits| hands.pressed & bits != 0;
        if self.session.step == Step::Drive {
            self.drive(hands, tick);
            return;
        }
        let direction = || {
            if hands.y < -51 || hands.aim_y < -51 {
                Some(Action::Up)
            } else if hands.y > 51 || hands.aim_y > 51 {
                Some(Action::Down)
            } else if hands.x < -51 || hands.aim_x < -51 {
                Some(Action::Less)
            } else if hands.x > 51 || hands.aim_x > 51 {
                Some(Action::More)
            } else {
                None
            }
        };
        let action = if on(button::MENU) {
            Some(Action::Close)
        } else if on(button::ALT) {
            Some(Action::Back)
        } else if on(button::OK | button::USE) {
            Some(Action::Confirm)
        } else if on(button::UP | button::AIM_UP) {
            Some(Action::Up)
        } else if on(button::DOWN | button::AIM_DOWN) {
            Some(Action::Down)
        } else if on(button::LEFT | button::AIM_LEFT) {
            Some(Action::Less)
        } else if on(button::RIGHT | button::AIM_RIGHT) {
            Some(Action::More)
        } else if let Some(row) = [button::SLOT1, button::SLOT2, button::SLOT3, button::SLOT4]
            .iter()
            .position(|&b| on(b))
        {
            Some(Action::Row(row))
        } else if tick >= self.repeat_at {
            direction()
        } else {
            None
        };
        if let Some(action) = action {
            self.session.action(action, tick);
            self.repeat_at = tick.saturating_add(5);
        } else if direction().is_none() {
            self.repeat_at = tick;
        }
        self.bring_to_life();
    }

    /// Whatever the grant just sealed is the realm's: sent to the host once, as the Creator drew it
    /// (the host checks it again, credits the maker and takes it into the world).
    fn bring_to_life(&mut self) {
        let Some(granted) = self.session.granted else {
            return;
        };
        if self.sent == Some(granted) {
            return;
        }
        let made = if self.session.sealed_weapon.is_some() {
            Made::Weapon(self.session.weapon.clone())
        } else if self.session.sealed.is_some() {
            Made::Creature(self.session.creation.clone())
        } else {
            return;
        };
        if let Ok(body) = serde_json::to_vec(&made) {
            self.sent = Some(granted);
            self.outbox.push((rite::ACTION, body));
        }
    }

    /// The Test drive: the arrows and WASD walk (Shift runs), F strikes, R casts (the game's ultimate key),
    /// X falls and gets up again (the bomb key), E meets, 1 to 3 flip the looks, Q goes back, Esc closes.
    /// R and X do something only for art that has a cast or a death clip of its own.
    fn drive(&mut self, hands: &Hands, tick: u64) {
        let on = |bits| hands.pressed & bits != 0;
        if on(button::MENU) {
            self.session.action(Action::Close, tick);
            return;
        }
        if on(button::ALT) {
            self.session.action(Action::Back, tick);
            return;
        }
        // The number keys flip the creature's looks while it walks (a role with no such look ignores them).
        if let Some(n) = [button::SLOT1, button::SLOT2, button::SLOT3, button::SLOT4].iter().position(|&b| on(b)) {
            self.session.action(Action::Look(n), tick);
        }
        let held = |bits| hands.held & bits != 0;
        let axis = |neg: bool, pos: bool| i8::from(pos) - i8::from(neg);
        let dx = axis(hands.x < -51 || hands.aim_x < -51 || held(button::LEFT | button::AIM_LEFT), hands.x > 51 || hands.aim_x > 51 || held(button::RIGHT | button::AIM_RIGHT));
        let dy = axis(hands.y < -51 || hands.aim_y < -51 || held(button::UP | button::AIM_UP), hands.y > 51 || hands.aim_y > 51 || held(button::DOWN | button::AIM_DOWN));
        let input = DriveInput {
            dx,
            dy,
            attack: on(button::FIRE),
            interact: on(button::USE),
            run: held(button::SHIFT),
            cast: on(button::X4),
            fall: on(button::X1),
        };
        self.drive_input(input, tick);
    }

    /// One tick of scripted or real input on the fixture floor.
    pub fn drive_input(&mut self, input: DriveInput, tick: u64) {
        #[cfg(any(test, feature = "testing"))]
        {
            let floor = self.floor.get_or_insert_with(|| Floor::new("Ada"));
            // The art on show says how it times its clips; a look flipped mid-walk changes it between blows.
            self.session.timing = crate::render::creation::timing(&self.session.creation);
            let ground = Ground {
                spawn: floor.spawn,
                walkable: &|x, y| floor.walkable(x, y),
                foes: &floor.foes,
                friend: Some(floor.friend),
            };
            self.session.drive_step(input, tick, &ground);
            if let Some(d) = &self.session.drive {
                floor.prepare(d, tick);
            }
        }
        #[cfg(not(any(test, feature = "testing")))]
        let _ = (input, tick, Ground { spawn: (0.0, 0.0), walkable: &|_, _| false, foes: &[], friend: None });
    }
}
