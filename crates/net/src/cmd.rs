//! Commands, and the server's queue of them for each seat.
//!
//! A client sends one command per client tick, numbered from 1. The server
//! takes one per seat per tick, in order: seq `n` is played on one tick and
//! only one, so a key held for ten client ticks walks ten server ticks
//! however the packets bunched on the way, and a client that replays its
//! unacknowledged commands on the server's word draws its own knight where
//! the server will have it. No leases, no back-dating, no renewals.
//!
//! The link is never even, so the queue plays commands out a little behind
//! their arrival:
//!
//! - **The cushion.** A seat's first command is taken once `CUSHION` more
//!   are queued behind it (or once it has waited `CUSHION` ticks, if no
//!   more come). From then on each tick takes the next seq, and a command
//!   up to `CUSHION` ticks late still arrives before its tick. A seat played
//!   in process (the host's own, a couch partner's) has no link to cushion
//!   against: its server sets its cushion to 0 (`SeatInputs::set_cushion`).
//! - **A guess.** A tick whose seq has not arrived repeats the seat's held
//!   keys (never its presses) and counts as that seq: the ack moves on. When
//!   the late command comes it is not played again; its presses go into the
//!   next tick, and its held keys are what the next guess repeats. A guess
//!   is right whenever the keys held did not change on that tick, so a held
//!   walk stays exact and a release lands at most a tick late.
//! - **A stall.** After `GUESS` guesses in a row the link has stopped, not
//!   jittered: the seat repeats its held keys without counting seqs, and
//!   fills its cushion again before taking more, from the newest commands
//!   (what piled up behind the stall folds into them, presses kept).
//! - **A drain.** A client whose clock runs a little fast piles commands
//!   up. Once the queue has sat past the cushion for `DRAIN_AFTER` ticks,
//!   one still command (nothing held, nothing pressed) is dropped: a knight
//!   standing still loses nothing by it.
//! - **The cap.** Past `DEPTH` queued, the oldest folds into the next,
//!   presses kept.
//!
//! The numbers come from a 30 Hz stream timed at the far end of the links
//! the Delve is played over (`docs/ENGINE.md`, "Ticks and input commands").

use std::collections::VecDeque;

pub trait Command: Clone + PartialEq {
    fn neutral() -> Self;
    fn held_only(&self) -> Self;
    fn absorb(&mut self, older: &Self);
}

pub const CUSHION: usize = 2;

pub fn sample_allowance(
    round_trip: std::time::Duration,
    tick_hz: u32,
    snapshot_ticks: u32,
    local: bool,
) -> u64 {
    let transit = (round_trip.min(std::time::Duration::from_millis(250)).as_nanos()
        * u128::from(tick_hz))
    .div_ceil(1_000_000_000) as u64;
    transit + u64::from(snapshot_ticks) + if local { 0 } else { CUSHION as u64 }
}

pub const GUESS: u32 = 4;
pub const DEPTH: usize = 8;
pub const DRAIN_AFTER: u32 = 30;
pub const NEUTRAL_AFTER: u32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub taken: u64,
    pub guessed: u64,
    pub wrong: u64,
    pub late: u64,
    pub waited: u64,
    pub drained: u64,
    pub folded: u64,
}

impl Counts {
    pub fn add(&mut self, other: &Counts) {
        self.taken += other.taken;
        self.guessed += other.guessed;
        self.wrong += other.wrong;
        self.late += other.late;
        self.waited += other.waited;
        self.drained += other.drained;
        self.folded += other.folded;
    }
}

pub struct SeatInputs<C: Command> {
    queue: VecDeque<(u32, C)>,
    cushion: usize,
    newest: u32,
    acked: u32,
    last: C,
    carry: Option<C>,
    guesses: VecDeque<(u32, C)>,
    primed: bool,
    filling: u32,
    starved: u32,
    guessed: u32,
    above: u32,
    counts: Counts,
}

impl<C: Command> Default for SeatInputs<C> {
    fn default() -> Self {
        SeatInputs {
            queue: VecDeque::with_capacity(DEPTH + 1),
            cushion: CUSHION,
            newest: 0,
            acked: 0,
            last: C::neutral(),
            carry: None,
            guesses: VecDeque::with_capacity(GUESS as usize),
            primed: false,
            filling: 0,
            starved: 0,
            guessed: 0,
            above: 0,
            counts: Counts::default(),
        }
    }
}

impl<C: Command> SeatInputs<C> {
    pub fn push(&mut self, seq: u32, cmd: C) {
        if seq <= self.newest {
            return;
        }
        self.newest = seq;
        if seq <= self.acked {
            self.counts.late += 1;
            while self.guesses.front().is_some_and(|&(s, _)| s < seq) {
                self.guesses.pop_front();
            }
            if let Some((_, played)) = self.guesses.pop_front().filter(|&(s, _)| s == seq)
                && played != cmd.held_only()
            {
                self.counts.wrong += 1;
            }
            self.last = cmd.held_only();
            let mut carry = cmd;
            if let Some(older) = self.carry.take() {
                carry.absorb(&older);
            }
            self.carry = Some(carry);
            return;
        }
        self.queue.push_back((seq, cmd));
        while self.queue.len() > DEPTH {
            self.fold_oldest();
        }
    }

    pub fn take(&mut self) -> (C, u32) {
        let cushion = self.cushion;
        if !self.primed && !self.queue.is_empty() {
            self.filling += 1;
            if self.queue.len() > cushion || self.filling > cushion as u32 {
                while self.queue.len() > cushion + 1 {
                    self.fold_oldest();
                }
                self.primed = true;
                self.filling = 0;
            }
        }
        if self.primed
            && self.above >= DRAIN_AFTER
            && self.queue.len() > cushion + 1
            && self.queue.front().is_some_and(|(_, c)| *c == C::neutral())
        {
            let (seq, _) = self.queue.pop_front().expect("the queue has a front");
            self.acked = seq;
            self.counts.drained += 1;
            self.above = 0;
        }
        let taken = if self.primed { self.queue.pop_front() } else { None };
        let cmd = match taken {
            Some((seq, mut cmd)) => {
                if let Some(carry) = self.carry.take() {
                    cmd.absorb(&carry);
                }
                self.counts.taken += 1;
                self.starved = 0;
                self.guessed = 0;
                self.acked = seq;
                self.last = cmd.held_only();
                cmd
            }
            None => self.starve(),
        };
        self.above = if self.queue.len() > cushion { self.above.saturating_add(1) } else { 0 };
        (cmd, self.acked)
    }

    fn starve(&mut self) -> C {
        self.starved = self.starved.saturating_add(1);
        if self.starved >= NEUTRAL_AFTER {
            self.last = C::neutral();
        }
        let mut cmd = self.last.held_only();
        if self.primed && self.guessed < GUESS {
            self.guessed += 1;
            self.acked = self.acked.wrapping_add(1).max(1);
            self.counts.guessed += 1;
            if self.guesses.len() == GUESS as usize {
                self.guesses.pop_front();
            }
            self.guesses.push_back((self.acked, cmd.clone()));
        } else {
            self.primed = false;
            self.counts.waited += 1;
        }
        if let Some(carry) = self.carry.take() {
            cmd.absorb(&carry);
        }
        cmd
    }

    fn fold_oldest(&mut self) {
        let (_, oldest) = self.queue.pop_front().expect("folding a queue of two or more");
        let (_, next) = self.queue.front_mut().expect("folding a queue of two or more");
        next.absorb(&oldest);
        self.counts.folded += 1;
    }

    pub fn set_cushion(&mut self, cushion: usize) {
        self.cushion = cushion.min(DEPTH - 1);
    }

    pub fn acked(&self) -> u32 {
        self.acked
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    pub fn take_counts(&mut self) -> Counts {
        std::mem::take(&mut self.counts)
    }

    pub fn restart(&mut self) {
        self.queue.clear();
        self.newest = 0;
        self.acked = 0;
        self.last = self.last.held_only();
        self.guesses.clear();
        self.primed = false;
        self.filling = 0;
        self.starved = 0;
        self.guessed = 0;
        self.above = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Keys {
        held: u32,
        taps: u32,
    }

    impl Command for Keys {
        fn neutral() -> Self {
            Keys { held: 0, taps: 0 }
        }
        fn held_only(&self) -> Self {
            Keys { held: self.held, taps: 0 }
        }
        fn absorb(&mut self, older: &Self) {
            self.taps += older.taps;
        }
    }

    fn keys(held: u32, taps: u32) -> Keys {
        Keys { held, taps }
    }

    fn primed(held: u32) -> SeatInputs<Keys> {
        let mut seat = SeatInputs::default();
        for seq in 1..=CUSHION as u32 + 1 {
            seat.push(seq, keys(held, 0));
        }
        assert_eq!(seat.take(), (keys(held, 0), 1));
        seat
    }

    #[test]
    fn the_first_command_waits_for_the_cushion_behind_it() {
        let mut seat = SeatInputs::default();
        seat.push(1, keys(1, 1));
        for _ in 0..CUSHION {
            assert_eq!(seat.take(), (keys(0, 0), 0), "waiting, standing");
            let next = seat.newest + 1;
            seat.push(next, keys(1, 0));
        }
        assert_eq!(seat.take(), (keys(1, 1), 1));
        assert_eq!(seat.queued(), CUSHION);
    }

    #[test]
    fn a_lone_command_is_taken_once_it_has_waited_the_cushion() {
        let mut seat = SeatInputs::default();
        seat.push(1, keys(0, 1));
        for _ in 0..CUSHION {
            assert_eq!(seat.take(), (keys(0, 0), 0));
        }
        assert_eq!(seat.take(), (keys(0, 1), 1));
    }

    #[test]
    fn an_in_process_seat_takes_each_command_on_the_next_tick() {
        let mut seat = SeatInputs::default();
        seat.set_cushion(0);
        seat.push(1, keys(1, 1));
        assert_eq!(seat.take(), (keys(1, 1), 1));
        seat.push(2, keys(0, 0));
        assert_eq!(seat.take(), (keys(0, 0), 2), "let go: stopped on the next tick");
        assert_eq!(seat.take(), (keys(0, 0), 3), "nothing came: guessed");
        seat.push(3, keys(0, 1));
        seat.push(4, keys(0, 0));
        assert_eq!(seat.take(), (keys(0, 1), 4), "3 came late: its tap rides on 4");
    }

    #[test]
    fn one_command_is_taken_a_tick_in_order() {
        let mut seat = primed(0);
        seat.push(4, keys(1, 1));
        seat.push(5, keys(1, 0));
        assert_eq!(seat.take(), (keys(0, 0), 2));
        assert_eq!(seat.take(), (keys(0, 0), 3));
        assert_eq!(seat.take(), (keys(1, 1), 4));
        assert_eq!(seat.take(), (keys(1, 0), 5));
    }

    #[test]
    fn a_starved_tick_guesses_the_held_keys_as_the_next_seq() {
        let mut seat = primed(3);
        seat.take();
        seat.take();
        assert_eq!(seat.take(), (keys(3, 0), 4));
        assert_eq!(seat.take(), (keys(3, 0), 5));
        let counts = seat.take_counts();
        assert_eq!((counts.taken, counts.guessed, counts.waited), (3, 2, 0));
    }

    #[test]
    fn a_late_command_is_never_played_twice_and_its_taps_come_next() {
        let mut seat = primed(3);
        seat.take();
        seat.take();
        assert_eq!(seat.take(), (keys(3, 0), 4), "guessed");
        seat.push(4, keys(3, 1));
        seat.push(5, keys(3, 0));
        assert_eq!(seat.take(), (keys(3, 1), 5), "4's tap rides on 5, 4 is not walked again");
        assert_eq!(seat.queued(), 0);
        let counts = seat.take_counts();
        assert_eq!((counts.late, counts.wrong), (1, 0));
    }

    #[test]
    fn a_guess_that_missed_a_release_is_counted_and_the_release_holds_from_then() {
        let mut seat = primed(1);
        seat.take();
        seat.take();
        assert_eq!(seat.take(), (keys(1, 0), 4), "walking, guessed");
        seat.push(4, keys(0, 0));
        assert_eq!(seat.take(), (keys(0, 0), 5), "the next guess stands");
        assert_eq!(seat.take_counts().wrong, 1);
    }

    #[test]
    fn a_stall_waits_then_fills_the_cushion_from_the_newest() {
        let mut seat = primed(1);
        seat.take();
        seat.take();
        for seq in 4..4 + GUESS {
            assert_eq!(seat.take(), (keys(1, 0), seq));
        }
        let acked = seat.acked();
        assert_eq!(seat.take(), (keys(1, 0), acked));
        assert_eq!(seat.take(), (keys(1, 0), acked));
        for seq in 4..=20 {
            seat.push(seq, keys(1, 1));
        }
        let (cmd, ack) = seat.take();
        assert_eq!(ack, 20 - CUSHION as u32);
        let mut taps = cmd.taps;
        while seat.queued() > 0 {
            taps += seat.take().0.taps;
        }
        assert_eq!(taps, 17);
        let counts = seat.take_counts();
        assert_eq!(counts.late, u64::from(GUESS));
        assert!(counts.folded > 0 && counts.waited == 2);
    }

    #[test]
    fn a_silent_seat_lets_go_after_its_limit() {
        let mut seat = primed(3);
        seat.take();
        seat.take();
        for _ in 1..NEUTRAL_AFTER {
            assert_eq!(seat.take().0, keys(3, 0));
        }
        assert_eq!(seat.take().0, keys(0, 0));
    }

    #[test]
    fn a_pile_up_folds_the_oldest_and_keeps_every_tap() {
        let mut seat = primed(0);
        for seq in 4..4 + DEPTH as u32 {
            seat.push(seq, keys(1, 1));
        }
        assert_eq!(seat.queued(), DEPTH);
        let mut taps = 0;
        while seat.queued() > 0 {
            taps += seat.take().0.taps;
        }
        assert_eq!(taps, DEPTH as u32);
    }

    #[test]
    fn a_queue_past_its_cushion_drops_a_still_command_after_a_while() {
        let mut seat = primed(0);
        let mut seq = 3;
        for tick in 0..DRAIN_AFTER * 10 {
            for _ in 0..1 + u32::from(tick % 40 == 39) {
                seq += 1;
                seat.push(seq, keys(0, 0));
            }
            seat.take();
        }
        let counts = seat.take_counts();
        assert!(counts.drained > 0, "{counts:?}, {} queued", seat.queued());
        assert!(seat.queued() <= CUSHION + 2, "{}", seat.queued());
    }

    #[test]
    fn a_walking_seat_past_its_cushion_drops_nothing() {
        let mut seat = primed(1);
        let mut seq = 3;
        for tick in 0..DRAIN_AFTER * 10 {
            for _ in 0..1 + u32::from(tick % 40 == 39) {
                seq += 1;
                seat.push(seq, keys(1, 0));
            }
            seat.take();
        }
        let counts = seat.take_counts();
        assert_eq!(counts.drained, 0);
        assert!(seat.queued() <= DEPTH);
    }

    #[test]
    fn a_resent_or_older_command_is_ignored() {
        let mut seat = primed(0);
        seat.push(5, keys(1, 0));
        seat.push(5, keys(2, 0));
        seat.push(4, keys(3, 0));
        assert_eq!(seat.queued(), 3);
        seat.take();
        seat.take();
        assert_eq!(seat.take(), (keys(1, 0), 5));
    }

    #[test]
    fn a_resumed_seat_counts_from_one_again_after_its_cushion() {
        let mut seat = primed(1);
        seat.push(900, keys(1, 0));
        seat.restart();
        assert_eq!(seat.take().0, keys(1, 0), "the held keys go on while the cushion fills");
        for seq in 1..=CUSHION as u32 + 1 {
            seat.push(seq, keys(2, 0));
        }
        assert_eq!(seat.take(), (keys(2, 0), 1));
    }

    struct Played {
        walked: u32,
        walk_cmds: u32,
        taps_sent: u32,
        taps_played: u32,
        releases: u32,
        worst_overrun: u32,
        worst_wait: f64,
        counts: Counts,
    }

    const STEP_MS: f64 = 1000.0 / 30.0;

    fn play(commands: u32, clock: f64, arrive: impl Fn(u32, f64) -> f64) -> Played {
        let cmd = |n: u32| keys(u32::from((n / 45) % 2 == 1), u32::from(n % 13 == 0));
        let mut arrivals: Vec<(f64, u32)> = (1..=commands).map(|n| (arrive(n, f64::from(n) * STEP_MS * clock), n)).collect();
        for i in 1..arrivals.len() {
            arrivals[i].0 = arrivals[i].0.max(arrivals[i - 1].0);
        }
        let mut seat = SeatInputs::default();
        let mut next = 0;
        let mut out = Played {
            walked: 0,
            walk_cmds: (1..=commands).filter(|&n| cmd(n).held == 1).count() as u32,
            taps_sent: (1..=commands).map(|n| cmd(n).taps).sum(),
            taps_played: 0,
            releases: 0,
            worst_overrun: 0,
            worst_wait: 0.0,
            counts: Counts::default(),
        };
        let mut overrun = 0;
        let last_arrival = arrivals.last().map_or(0.0, |a| a.0);
        let mut tick = 0;
        loop {
            let now = 7.0 + f64::from(tick) * STEP_MS;
            tick += 1;
            while next < arrivals.len() && arrivals[next].0 <= now {
                seat.push(arrivals[next].1, cmd(arrivals[next].1));
                next += 1;
            }
            let before = seat.counts.taken;
            let (played, ack) = seat.take();
            if seat.counts.taken > before {
                let arrived = arrivals.iter().find(|a| a.1 == ack).map_or(now, |a| a.0);
                out.worst_wait = out.worst_wait.max((now - arrived) / STEP_MS);
            }
            out.walked += played.held;
            out.taps_played += played.taps;
            if played.held == 1 && ack > 0 && cmd(ack).held == 0 {
                overrun += 1;
                out.worst_overrun = out.worst_overrun.max(overrun);
            } else {
                overrun = 0;
            }
            if (next == arrivals.len() && seat.queued() == 0 && ack >= commands) || now > last_arrival + 40.0 * STEP_MS {
                break;
            }
        }
        out.releases = (1..commands).filter(|&n| cmd(n).held == 1 && cmd(n + 1).held == 0).count() as u32;
        out.counts = seat.take_counts();
        out
    }

    fn noise(n: u32) -> f64 {
        let x = n.wrapping_mul(2_654_435_761).rotate_left(13).wrapping_mul(0x9E37_79B1);
        f64::from(x) / f64::from(u32::MAX)
    }

    #[test]
    fn over_an_even_or_jittery_link_every_command_is_played_once() {
        for (name, link, spread) in [
            ("even", Box::new(|_: u32, sent: f64| sent + 30.0) as Box<dyn Fn(u32, f64) -> f64>, 0.0),
            ("jitter", Box::new(|n, sent| sent + 30.0 + 50.0 * noise(n)), 50.0 / STEP_MS),
            ("bunched", Box::new(|_, sent| 30.0 + (sent / 55.0).ceil() * 55.0), 55.0 / STEP_MS),
        ] {
            let p = play(900, 1.0, link);
            assert_eq!(p.walked, p.walk_cmds, "{name}: walked {} for {}", p.walked, p.walk_cmds);
            assert_eq!(p.taps_played, p.taps_sent, "{name}");
            assert_eq!((p.counts.guessed, p.counts.folded, p.counts.drained), (0, 0, 0), "{name}: {:?}", p.counts);
            assert_eq!(p.worst_overrun, 0, "{name}");
            assert!(p.worst_wait <= CUSHION as f64 + 1.0 + spread, "{name}: waited {:.1} ticks", p.worst_wait);
        }
    }

    #[test]
    fn a_link_late_past_the_cushion_guesses_and_only_a_change_on_a_guess_is_off() {
        let p = play(900, 1.0, |_, sent| 30.0 + (sent / 166.0).ceil() * 166.0);
        assert!(p.counts.guessed > 0);
        assert_eq!(p.taps_played, p.taps_sent);
        let past = 5 - CUSHION as u32;
        eprintln!("bunched 166 ms: {:?}, walked {} for {}, overrun {}", p.counts, p.walked, p.walk_cmds, p.worst_overrun);
        assert!(p.counts.wrong <= u64::from(2 * past * p.releases), "{:?} for {} walks", p.counts, p.releases);
        assert!(p.walked.abs_diff(p.walk_cmds) as u64 <= p.counts.wrong, "walked {} for {}", p.walked, p.walk_cmds);
        assert!(p.worst_overrun <= past);
        assert!(p.counts.waited <= 10, "a stall or two at most: {:?}", p.counts);
    }

    #[test]
    fn a_stalled_link_folds_its_backlog_and_keeps_every_tap() {
        let p = play(900, 1.0, |n, sent| if (300..330).contains(&n) { 30.0 + 330.0 * STEP_MS } else { sent + 30.0 });
        assert_eq!(p.taps_played, p.taps_sent);
        assert_eq!(p.counts.guessed, u64::from(GUESS));
        assert!(p.counts.folded > 0 && p.counts.waited > u64::from(CUSHION as u32));
        assert!(p.walked.abs_diff(p.walk_cmds) <= 30 + GUESS, "walked {} for {}", p.walked, p.walk_cmds);
        assert!(p.worst_wait <= CUSHION as f64 + 1.0, "waited {:.1}", p.worst_wait);
    }

    #[test]
    fn a_slow_or_fast_client_clock_stays_close_and_bounded() {
        for (name, clock) in [("slow", 1.005), ("fast", 0.995)] {
            let p = play(1800, clock, |n, sent| sent + 30.0 + 20.0 * noise(n));
            assert_eq!(p.taps_played, p.taps_sent, "{name}");
            assert!(p.worst_wait <= DEPTH as f64 + 1.0, "{name}: waited {:.1}", p.worst_wait);
            let off = p.walked.abs_diff(p.walk_cmds);
            assert!(off <= 12, "{name}: walked {} for {} {:?}", p.walked, p.walk_cmds, p.counts);
            assert!(p.worst_overrun <= 1, "{name}: {:?}", p.counts);
        }
    }
}
