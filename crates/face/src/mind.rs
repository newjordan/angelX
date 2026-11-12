//! What she does, moment to moment: where she looks, how she holds her
//! head, her blinks and small expressions, and how present she is. A
//! function of time and of what happens in the cockpit ([`Cue`]), with her
//! own small randomness. She reads you as you type, sometimes; she looks
//! away to think while the agent works; she speaks from an Audio2Face track
//! when there is one.

use crate::model::Goddess;
use crate::pose::HeadPose;

/// Something that happened, that she may answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// She is called in.
    Arrive,
    /// She is sent away.
    Depart,
    /// A key went into the composer.
    Typing,
    /// The composer was sent.
    Submitted,
    /// The agent is at work (thinking, streaming, calling tools).
    Working,
    /// The agent's answer has landed; `ok` false for an error.
    Finished { ok: bool },
    /// Her spectacle: her eyes heat, she fires them, then a rainbow pours
    /// out of her mouth ([`SPECTACLE`]).
    Spectacle,
}

/// Her spectacle's beats, seconds from its start: her eyes heat until
/// `FIRE_AT`, burn until `FIRE_END`, a breath, then the rainbow pours from
/// `POUR_AT` to `POUR_END`, and she settles by `SPECTACLE`.
pub const FIRE_AT: f64 = 0.9;
pub const FIRE_END: f64 = 2.5;
pub const POUR_AT: f64 = 2.9;
pub const POUR_END: f64 = 5.6;
pub const SPECTACLE: f64 = 6.6;

/// One moment of her, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Moment {
    /// A weight per channel of the goddess.
    pub weights: Vec<f32>,
    pub head: HeadPose,
    /// How here she is, 0 (gone) to 1: what the drawing fades and forms by.
    pub presence: f32,
    /// How molten her chrome is, 0..1.
    pub liquid: f32,
    /// How bright her eyes and halo burn, about 0..1.5.
    pub glow: f32,
    /// How loud she is right now, 0..1, from the speech track.
    pub voice: f32,
    /// How fast her halo turns, 1 at rest.
    pub spin: f32,
    /// Her spectacle: how hot her eyes are (0..1, rising before she
    /// fires), how hard they fire (0..1), how hard the rainbow pours (0..1),
    /// and how far into it she is (seconds), when one is on.
    pub heat: f32,
    pub laser: f32,
    pub pour: f32,
    pub spectacle: Option<f32>,
}

/// Where she looks: yaw toward her left, pitch down, radians, relative to
/// straight ahead of her body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Gaze {
    yaw: f32,
    pitch: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Focus {
    /// At you, through the screen.
    Viewer,
    /// Down at what you are typing.
    Input,
    /// Somewhere else, idly.
    Away(Gaze),
    /// Inward and up, thinking.
    Musing(Gaze),
}

/// The channels she moves by name, looked up once.
#[derive(Clone, Debug, Default)]
struct Channels {
    blink: [Option<usize>; 2],
    look_up: [Option<usize>; 2],
    look_down: [Option<usize>; 2],
    look_in: [Option<usize>; 2],
    look_out: [Option<usize>; 2],
    squint: [Option<usize>; 2],
    wide: [Option<usize>; 2],
    smile: [Option<usize>; 2],
    press: [Option<usize>; 2],
    brow_inner: Option<usize>,
    brow_outer: [Option<usize>; 2],
    brow_down: [Option<usize>; 2],
    jaw_open: Option<usize>,
    funnel: Option<usize>,
    roll_lower: Option<usize>,
    mouth_left: Option<usize>,
    cheek_squint: [Option<usize>; 2],
}

impl Channels {
    fn of(g: &Goddess) -> Self {
        let c = |n: &str| g.channel(n);
        let pair = |a: &str| [c(&format!("{a}Left")), c(&format!("{a}Right"))];
        Self {
            blink: pair("eyeBlink"),
            look_up: pair("eyeLookUp"),
            look_down: pair("eyeLookDown"),
            look_in: pair("eyeLookIn"),
            look_out: pair("eyeLookOut"),
            squint: pair("eyeSquint"),
            wide: pair("eyeWide"),
            smile: pair("mouthSmile"),
            press: pair("mouthPress"),
            brow_inner: c("browInnerUp"),
            brow_outer: pair("browOuterUp"),
            brow_down: pair("browDown"),
            jaw_open: c("jawOpen"),
            funnel: c("mouthFunnel"),
            roll_lower: c("mouthRollLower"),
            mouth_left: c("mouthLeft"),
            cheek_squint: pair("cheekSquint"),
        }
    }
}

/// How long she takes to arrive and to leave, seconds.
pub const ARRIVAL: f64 = 3.6;
pub const DEPARTURE: f64 = 1.8;

/// How far her eyes turn at a shape's full weight, radians (ARKit's span).
const EYE_SIDE: f32 = 0.52;
const EYE_UP: f32 = 0.42;
const EYE_DOWN: f32 = 0.50;

/// Her mind.
#[derive(Clone, Debug)]
pub struct Mind {
    ch: Channels,
    channels: usize,
    rng: u64,
    /// Where you are, and where the composer is, from her eyes.
    viewer: Gaze,
    input: Gaze,
    // Attention.
    focus: Focus,
    focus_until: f64,
    gaze: Gaze,
    next_saccade: f64,
    micro: Gaze,
    // Reading what you type.
    typed_at: f64,
    reading: bool,
    reading_since: f64,
    tilt: f32,
    glance_until: f64,
    next_glance: f64,
    declined_until: f64,
    // The agent.
    working: bool,
    worked_at: f64,
    // Feelings that fade: pleasure, concern, a nod.
    joy: f32,
    joy_at: f64,
    concern: f32,
    concern_at: f64,
    nod_at: f64,
    // Blinks.
    next_blink: f64,
    blink_at: f64,
    double: bool,
    // Rare: she closes her eyes a while.
    next_commune: f64,
    commune_at: f64,
    // Being here.
    arrived_at: Option<f64>,
    departed_at: Option<f64>,
    // Her spectacle, when one is on.
    spectacle_at: Option<f64>,
    // Smoothed.
    head: HeadPose,
    expression: Vec<f32>,
    voice: f32,
    last: f64,
}

impl Mind {
    /// A mind for `goddess`. `viewer` is where you are from her eyes (yaw
    /// toward her left, pitch down, radians), `seed` her randomness.
    pub fn new(goddess: &Goddess, viewer: [f32; 2], seed: u64) -> Self {
        let mut m = Self {
            ch: Channels::of(goddess),
            channels: goddess.channels.len(),
            rng: seed | 1,
            viewer: Gaze {
                yaw: viewer[0],
                pitch: viewer[1],
            },
            input: Gaze {
                yaw: -0.35,
                pitch: 0.55,
            },
            focus: Focus::Viewer,
            focus_until: 0.0,
            gaze: Gaze {
                yaw: viewer[0],
                pitch: viewer[1],
            },
            next_saccade: 0.0,
            micro: Gaze::default(),
            typed_at: f64::NEG_INFINITY,
            reading: false,
            reading_since: 0.0,
            tilt: 0.0,
            glance_until: 0.0,
            next_glance: 0.0,
            declined_until: 0.0,
            working: false,
            worked_at: f64::NEG_INFINITY,
            joy: 0.0,
            joy_at: f64::NEG_INFINITY,
            concern: 0.0,
            concern_at: f64::NEG_INFINITY,
            nod_at: f64::NEG_INFINITY,
            next_blink: 1.5,
            blink_at: f64::NEG_INFINITY,
            double: false,
            next_commune: 45.0,
            commune_at: f64::NEG_INFINITY,
            arrived_at: None,
            departed_at: None,
            spectacle_at: None,
            head: HeadPose::default(),
            expression: vec![0.0; goddess.channels.len()],
            voice: 0.0,
            last: 0.0,
        };
        m.next_commune = 40.0 + m.uniform() as f64 * 50.0;
        m
    }

    /// Where the composer lies from her eyes (yaw toward her left, pitch
    /// down, radians): the host knows where it drew her.
    pub fn set_input(&mut self, yaw: f32, pitch: f32) {
        self.input = Gaze { yaw, pitch };
    }

    /// Something caught her eye at `now` (the pointer moving, say): yaw
    /// toward her left and pitch down, radians. She glances at it for
    /// `seconds` if she is idle; reading, thinking and speaking come first.
    pub fn glance(&mut self, yaw: f32, pitch: f32, now: f64, seconds: f64) -> bool {
        let busy = self.reading || self.working || self.voice > 0.05 || !self.here();
        if busy || matches!(self.focus, Focus::Away(_)) && now < self.focus_until {
            return false;
        }
        self.focus = Focus::Away(Gaze {
            yaw: yaw.clamp(-0.8, 0.8),
            pitch: pitch.clamp(-0.5, 0.7),
        });
        self.focus_until = now + seconds;
        true
    }

    /// Whether her spectacle is on at `now`.
    pub fn in_spectacle(&self, now: f64) -> bool {
        self.spectacle_at
            .is_some_and(|at| (0.0..SPECTACLE).contains(&(now - at)))
    }

    /// Whether she is reading what you type right now.
    pub fn reading(&self) -> bool {
        self.reading
    }

    /// Whether she has arrived and not left.
    pub fn here(&self) -> bool {
        self.arrived_at.is_some() && self.departed_at.is_none()
    }

    /// Whether she has finished leaving at `now`.
    pub fn gone(&self, now: f64) -> bool {
        self.arrived_at.is_none() || self.departed_at.is_some_and(|d| now - d >= DEPARTURE)
    }

    /// Something happened at `now` (seconds on her clock).
    pub fn cue(&mut self, cue: Cue, now: f64) {
        match cue {
            Cue::Arrive => {
                if !self.here() {
                    self.arrived_at = Some(now);
                    self.departed_at = None;
                    self.next_blink = now + ARRIVAL + 1.2;
                    self.focus = Focus::Viewer;
                }
            }
            Cue::Depart => {
                if self.here() {
                    self.departed_at = Some(now);
                }
            }
            Cue::Typing => {
                self.typed_at = now;
                if !self.reading && now >= self.declined_until {
                    // Curiosity, not duty: she looks down sometimes.
                    if self.uniform() < 0.62 {
                        self.reading = true;
                        self.reading_since = now + 0.25 + self.uniform() as f64 * 0.5;
                        self.tilt = if self.uniform() < 0.5 { -1.0 } else { 1.0 }
                            * (0.08 + self.uniform() * 0.07);
                        self.next_glance = self.reading_since + 2.5 + self.uniform() as f64 * 3.5;
                    } else {
                        self.declined_until = now + 6.0 + self.uniform() as f64 * 6.0;
                    }
                }
            }
            Cue::Submitted => {
                self.reading = false;
                self.declined_until = now + 1.0;
                self.nod_at = now;
                self.focus = Focus::Viewer;
                self.next_saccade = now + 0.6;
            }
            Cue::Working => {
                if !self.working {
                    self.working = true;
                    self.focus_until = now;
                }
                self.worked_at = now;
            }
            Cue::Spectacle => {
                if self.here() && self.spectacle_at.is_none_or(|at| now - at >= SPECTACLE) {
                    self.spectacle_at = Some(now);
                }
            }
            Cue::Finished { ok } => {
                self.working = false;
                self.focus = Focus::Viewer;
                self.next_saccade = now + 0.8;
                self.nod_at = now;
                if ok {
                    self.joy_at = now;
                } else {
                    self.concern_at = now;
                }
            }
        }
    }

    /// Her at `now`, speaking `speech` (a weight per channel from a track,
    /// and how loud it is) or not.
    pub fn moment(&mut self, now: f64, speech: Option<(&[f32], f32)>) -> Moment {
        let dt = ((now - self.last).clamp(0.0, 0.1)) as f32;
        self.last = now;
        let speaking = speech.is_some();
        let loud = speech.map_or(0.0, |(_, v)| v);
        self.voice += (loud - self.voice) * rate(dt, if loud > self.voice { 30.0 } else { 9.0 });
        if self.working && now - self.worked_at > 4.0 {
            // No word from the agent in a while: it has stopped.
            self.working = false;
        }
        if self.reading && now - self.typed_at > 1.8 {
            self.reading = false;
            self.focus = Focus::Viewer;
            self.next_saccade = now + 0.3;
        }

        let presence = self.presence(now);
        let arriving = self
            .arrived_at
            .map_or(1.0, |a| ((now - a) / ARRIVAL).clamp(0.0, 1.0) as f32);

        // ---- where she looks ----
        let reading_now = self.reading && now >= self.reading_since && !speaking;
        if reading_now && now >= self.next_glance {
            // A glance up at you, then back to the words.
            self.glance_until = now + 0.5 + self.uniform() as f64 * 0.8;
            self.next_glance = self.glance_until + 2.5 + self.uniform() as f64 * 4.0;
        }
        let focus = if arriving < 0.8 {
            Focus::Viewer
        } else if speaking {
            Focus::Viewer
        } else if reading_now {
            if now < self.glance_until {
                Focus::Viewer
            } else {
                Focus::Input
            }
        } else if self.working {
            if now >= self.focus_until || !matches!(self.focus, Focus::Musing(_)) {
                let side = if self.uniform() < 0.5 { -1.0 } else { 1.0 };
                self.focus_until = now + 1.8 + self.uniform() as f64 * 2.8;
                self.focus = Focus::Musing(Gaze {
                    yaw: side * (0.18 + self.uniform() * 0.25),
                    pitch: -0.12 - self.uniform() * 0.16,
                });
            }
            self.focus
        } else {
            if matches!(self.focus, Focus::Away(_) | Focus::Musing(_)) && now >= self.focus_until {
                self.focus = Focus::Viewer;
                self.focus_until = now + 5.0 + self.uniform() as f64 * 9.0;
            } else if matches!(self.focus, Focus::Viewer | Focus::Input) && now >= self.focus_until
            {
                if self.uniform() < 0.45 {
                    let g = Gaze {
                        yaw: (self.uniform() - 0.5) * 0.9,
                        pitch: (self.uniform() - 0.6) * 0.35,
                    };
                    self.focus = Focus::Away(g);
                    self.focus_until = now + 1.4 + self.uniform() as f64 * 2.6;
                } else {
                    self.focus = Focus::Viewer;
                    self.focus_until = now + 4.0 + self.uniform() as f64 * 8.0;
                }
            }
            self.focus
        };
        let target = match focus {
            Focus::Viewer => self.viewer,
            Focus::Input => self.input,
            Focus::Away(g) | Focus::Musing(g) => g,
        };
        // Small saccades about the target: on your eyes, along the line.
        if now >= self.next_saccade {
            let spread = if reading_now { 0.07 } else { 0.025 };
            self.micro = Gaze {
                yaw: (self.uniform() - 0.5) * spread * 2.0,
                pitch: (self.uniform() - 0.5) * spread * 0.6,
            };
            self.next_saccade = now
                + if reading_now {
                    0.18 + self.uniform() as f64 * 0.35
                } else {
                    0.4 + self.uniform() as f64 * 2.0
                };
        }
        let want = Gaze {
            yaw: target.yaw + self.micro.yaw,
            pitch: target.pitch + self.micro.pitch,
        };
        let before = self.gaze;
        let k = rate(dt, 22.0);
        self.gaze.yaw += (want.yaw - self.gaze.yaw) * k;
        self.gaze.pitch += (want.pitch - self.gaze.pitch) * k;
        let jump = (want.yaw - before.yaw).abs() + (want.pitch - before.pitch).abs();
        if jump > 0.35 && now - self.blink_at > 0.6 && self.uniform() < 0.4 {
            // Big turns of the eyes come with a blink.
            self.blink_at = now;
        }

        // ---- the head ----
        let t = now as f32;
        let drift = |a: f32, b: f32, c: f32| {
            (t * a).sin() * 0.6 + (t * b + 1.3).sin() * 0.3 + (t * c + 2.1).sin() * 0.1
        };
        let mut head = HeadPose {
            yaw: self.gaze.yaw * 0.45 + 0.035 * drift(0.21, 0.53, 1.37),
            pitch: (self.gaze.pitch - self.viewer.pitch) * 0.5 + 0.025 * drift(0.17, 0.41, 1.11),
            roll: 0.02 * drift(0.13, 0.37, 0.91),
            rise: 0.0025 * (t * 1.15).sin(),
            sway: [0.0; 2],
        };
        if reading_now && focus == Focus::Input {
            head.pitch += 0.08;
            head.roll += self.tilt;
        }
        if self.working {
            head.roll += 0.04 * (t * 0.3).sin();
        }
        if speaking {
            head.pitch += 0.05 * self.voice * (t * 3.1).sin();
            head.yaw += 0.03 * self.voice * (t * 1.7).sin();
        }
        let since_nod = (now - self.nod_at) as f32;
        if (0.0..0.7).contains(&since_nod) {
            head.pitch += 0.09 * (since_nod / 0.7 * std::f32::consts::PI).sin();
        }
        // Waking: her face rises to you as she forms.
        head.pitch += 0.35 * (1.0 - ease(arriving));
        // Her spectacle: where she is in it, and how it moves her.
        let show = self
            .spectacle_at
            .map(|at| now - at)
            .filter(|s| (0.0..SPECTACLE).contains(s));
        if show.is_none() {
            self.spectacle_at = None;
        }
        let beat = |from: f64, to: f64| {
            show.map_or(0.0, |s| ((s - from) / (to - from)).clamp(0.0, 1.0) as f32)
        };
        let heat = beat(0.0, FIRE_AT) * (1.0 - beat(FIRE_END, FIRE_END + 0.5));
        let laser = if show.is_some_and(|s| (FIRE_AT..FIRE_END).contains(&s)) {
            1.0 - 0.25 * beat(FIRE_END - 0.4, FIRE_END)
        } else {
            0.0
        };
        let pour =
            (beat(POUR_AT, POUR_AT + 0.35) * (1.0 - beat(POUR_END - 0.5, POUR_END))).max(0.0);
        let settle = beat(POUR_END, SPECTACLE);
        if show.is_some() {
            // She rears as she fires; she bows her head to pour.
            head.pitch += -0.06 * laser + 0.16 * pour * (1.0 - settle);
            head.yaw += 0.02 * laser * (t * 23.0).sin();
        }
        let hk = rate(dt, if speaking { 6.0 } else { 3.2 });
        self.head.yaw += (head.yaw - self.head.yaw) * hk;
        self.head.pitch += (head.pitch - self.head.pitch) * hk;
        self.head.roll += (head.roll - self.head.roll) * hk;
        self.head.rise = head.rise;
        // Her hair hangs behind her head's turns and drifts on its own.
        let swing = [
            0.05 * drift(0.31, 0.77, 1.9) - 0.6 * (head.roll - self.head.roll) * 4.0,
            0.04 * drift(0.23, 0.61, 1.4) + 0.6 * (head.pitch - self.head.pitch) * 4.0,
        ];
        let sk = rate(dt, 2.0);
        self.head.sway[0] += (swing[0] - self.head.sway[0]) * sk;
        self.head.sway[1] += (swing[1] - self.head.sway[1]) * sk;

        // ---- her face ----
        let mut want = vec![0.0f32; self.channels];
        let put = |c: Option<usize>, v: f32, want: &mut Vec<f32>| {
            if let Some(c) = c {
                want[c] += v;
            }
        };
        // At rest: serene, a trace of a smile.
        for i in 0..2 {
            put(self.ch.smile[i], 0.07, &mut want);
            put(self.ch.squint[i], 0.06, &mut want);
        }
        if reading_now {
            put(self.ch.brow_inner, 0.28, &mut want);
            for i in 0..2 {
                put(self.ch.brow_outer[i], 0.12, &mut want);
                put(self.ch.smile[i], 0.08, &mut want);
                put(self.ch.wide[i], 0.06, &mut want);
            }
        }
        if self.working {
            for i in 0..2 {
                put(self.ch.squint[i], 0.18, &mut want);
                put(self.ch.brow_down[i], 0.12, &mut want);
                put(self.ch.press[i], 0.18, &mut want);
            }
            put(self.ch.roll_lower, 0.06, &mut want);
            put(self.ch.mouth_left, 0.08, &mut want);
        }
        // Her spectacle's face: a scowl as her eyes heat, wide and fixed as
        // they fire, her jaw dropped wide as the rainbow pours.
        for i in 0..2 {
            put(self.ch.squint[i], 0.55 * heat * (1.0 - laser), &mut want);
            put(self.ch.brow_down[i], 0.55 * heat, &mut want);
            put(self.ch.wide[i], 0.6 * laser, &mut want);
            put(self.ch.squint[i], 0.35 * pour, &mut want);
            put(self.ch.brow_outer[i], 0.3 * pour, &mut want);
        }
        put(self.ch.jaw_open, 0.06 * laser + 0.78 * pour, &mut want);
        put(self.ch.funnel, 0.3 * pour, &mut want);
        put(self.ch.brow_inner, 0.25 * pour, &mut want);
        self.joy = feeling(now - self.joy_at, 2.5);
        self.concern = feeling(now - self.concern_at, 4.0);
        for i in 0..2 {
            put(self.ch.smile[i], 0.35 * self.joy, &mut want);
            put(self.ch.cheek_squint[i], 0.2 * self.joy, &mut want);
            put(self.ch.brow_down[i], 0.12 * self.concern, &mut want);
        }
        put(self.ch.brow_inner, 0.3 * self.concern, &mut want);
        let ek = rate(dt, if show.is_some() { 12.0 } else { 5.0 });
        for (e, w) in self.expression.iter_mut().zip(&want) {
            *e += (w - *e) * ek;
        }
        let mut weights = self.expression.clone();

        // Speech: the track's shapes over her expression.
        if let Some((track, _)) = speech {
            for (w, s) in weights.iter_mut().zip(track) {
                *w += s;
            }
        }

        // Her eyes within her head.
        let eye_yaw = self.gaze.yaw - self.head.yaw;
        let eye_pitch = self.gaze.pitch - self.head.pitch;
        let side = (eye_yaw / EYE_SIDE).clamp(-1.0, 1.0);
        let vertical = if eye_pitch >= 0.0 {
            (eye_pitch / EYE_DOWN).min(1.0)
        } else {
            (eye_pitch / EYE_UP).max(-1.0)
        };
        let (l, r) = (0usize, 1usize);
        let mut set = |c: Option<usize>, v: f32| {
            if let Some(c) = c {
                weights[c] = weights[c].max(v);
            }
        };
        if side > 0.0 {
            set(self.ch.look_out[l], side);
            set(self.ch.look_in[r], side);
        } else {
            set(self.ch.look_in[l], -side);
            set(self.ch.look_out[r], -side);
        }
        for i in [l, r] {
            if vertical > 0.0 {
                set(self.ch.look_down[i], vertical);
            } else {
                set(self.ch.look_up[i], -vertical);
            }
        }

        // Blinks.
        if now >= self.next_blink {
            self.blink_at = now;
            self.double = self.uniform() < 0.14;
            let mean = if reading_now {
                5.0
            } else if speaking {
                2.6
            } else {
                3.6
            };
            self.next_blink = now + 0.8 + (-(self.uniform().max(1e-4)).ln() * mean) as f64;
        }
        let mut blink = lid((now - self.blink_at) as f32);
        if self.double {
            blink = blink.max(lid((now - self.blink_at) as f32 - 0.28));
        }
        // Rarely, she closes her eyes and listens to something far off.
        if now >= self.next_commune && !speaking && !reading_now && !self.working && arriving >= 1.0
        {
            self.commune_at = now;
            self.next_commune = now + 50.0 + self.uniform() as f64 * 70.0;
        }
        let commune = {
            let s = (now - self.commune_at) as f32;
            if (0.0..3.2).contains(&s) {
                (s / 0.6).min(1.0).min((3.2 - s) / 0.8) * 0.92
            } else {
                0.0
            }
        };
        // Waking: her eyes stay closed until she has formed.
        let waking = 1.0 - ((arriving - 0.62) / 0.3).clamp(0.0, 1.0);
        // Her eyes do not blink while they burn.
        let closed = (blink.max(commune) * (1.0 - laser)).max(ease(waking));
        for i in [l, r] {
            if let Some(c) = self.ch.blink[i] {
                weights[c] = weights[c].max(closed);
            }
        }
        for w in &mut weights {
            *w = w.clamp(-0.2, 1.0);
        }

        // Her light and her substance.
        let molten = 1.0 - ease(((arriving - 0.15) / 0.7).clamp(0.0, 1.0));
        let leaving = self
            .departed_at
            .map_or(0.0, |d| ((now - d) / DEPARTURE).clamp(0.0, 1.0) as f32);
        let liquid = (0.06 + 0.06 * self.working as u8 as f32 + 0.05 * self.voice)
            .max(molten)
            .max(ease(leaving));
        let glow = (0.75 + 0.45 * self.voice + 0.2 * self.joy + 0.15 * self.working as u8 as f32)
            * (1.0 - closed * 0.6);
        let spin = 1.0 + 1.8 * self.working as u8 as f32 + 1.2 * self.voice;
        Moment {
            weights,
            head: self.head,
            presence,
            liquid,
            glow,
            voice: self.voice,
            spin,
            heat,
            laser,
            pour,
            spectacle: show.map(|s| s as f32),
        }
    }

    /// How here she is at `now`, 0..1.
    pub fn presence(&self, now: f64) -> f32 {
        let Some(a) = self.arrived_at else { return 0.0 };
        let coming = ease(((now - a) / (ARRIVAL * 0.55)).clamp(0.0, 1.0) as f32);
        let going =
            self.departed_at.map_or(
                0.0,
                |d| ease(((now - d) / DEPARTURE).clamp(0.0, 1.0) as f32),
            );
        coming * (1.0 - going)
    }

    /// 0..1, xorshift64*.
    fn uniform(&mut self) -> f32 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// An eyelid through one blink, `s` seconds in: down fast, up slower.
fn lid(s: f32) -> f32 {
    if !(0.0..0.32).contains(&s) {
        0.0
    } else if s < 0.075 {
        s / 0.075
    } else if s < 0.11 {
        1.0
    } else {
        1.0 - (s - 0.11) / 0.21
    }
}

/// The share of the way a value covers in `dt` chasing at `per_second`.
fn rate(dt: f32, per_second: f32) -> f32 {
    1.0 - (-dt * per_second).exp()
}

/// What remains of a feeling `age` seconds after it came: it swells over a
/// quarter second, then halves every `half` seconds, gone after six halvings.
fn feeling(age: f64, half: f64) -> f32 {
    if !(0.0..half * 6.0).contains(&age) {
        return 0.0;
    }
    let swell = (age / 0.25).min(1.0);
    (swell * 0.5f64.powf(age / half)) as f32
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
