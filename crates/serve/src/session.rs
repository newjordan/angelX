//! Who is in: the join key, the seats and their tokens, the watchers, and
//! the host's lock. No IO; the server calls in with the time.

use crate::link::{normalize_key, same_key};
use angel_net::proto::{MAX_NAME, Reject};
use std::time::{Duration, Instant};

/// A dropped connection's seat waits this long for its token to come back.
pub const RESUME_WINDOW: Duration = Duration::from_secs(120);
/// Seats at most, whatever the game asks for.
pub const MAX_PLAYERS: u32 = 256;
/// Watchers beyond the seats.
pub const MAX_SPECTATORS: u32 = 32;
/// Tokens of kicked players remembered, so a reload does not walk back in.
const KICKED_KEPT: usize = 256;
/// Characters of a name kept.
const NAME_CHARS: usize = 24;

/// A taken seat.
#[derive(Clone, Debug)]
struct Taken {
    token: String,
    name: String,
    /// The connection playing it, or `None` while it waits for a resume.
    conn: Option<u64>,
    dropped_at: Option<Instant>,
}

/// What a hello was let in as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admit {
    Seat {
        seat: u32,
        token: String,
        name: String,
        /// The seat was this client's before: same knight, same everything.
        resumed: bool,
        /// A connection still holding the seat (a reload before the old
        /// socket died): it is closed.
        displaced: Option<u64>,
    },
    Watch,
}

/// A seat as the host sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub seat: u32,
    pub name: String,
    pub connected: bool,
}

pub struct Sessions {
    key: String,
    locked: bool,
    seats: Vec<Option<Taken>>,
    watchers: u32,
    kicked: Vec<String>,
    /// Seats kept for their players by name ([`Sessions::keep`]).
    kept: Vec<Option<String>>,
}

impl Sessions {
    pub fn new(key: &str, seats: u32) -> Sessions {
        Sessions {
            key: normalize_key(key),
            locked: false,
            seats: vec![None; seats.min(MAX_PLAYERS) as usize],
            watchers: 0,
            kicked: Vec::new(),
            kept: vec![None; seats.min(MAX_PLAYERS) as usize],
        }
    }

    /// Seats kept for the players named: a restored run's knights, whose
    /// tokens went with the old server. A player who joins by that name
    /// takes their own seat back (so their own knight), and newcomers take
    /// the seats nobody keeps first (the operator, 2026-10-18: a restart
    /// and a reload swapped two friends' knights).
    pub fn keep(&mut self, names: &[(u32, String)]) {
        for (seat, name) in names {
            if let Some(kept) = self.kept.get_mut(*seat as usize) {
                *kept = Some(name.trim().to_lowercase());
            }
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    /// A new join key: the old link stops letting newcomers in. Seated
    /// players keep their seats and their tokens.
    pub fn set_key(&mut self, key: &str) {
        self.key = normalize_key(key);
    }

    pub fn lock(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    /// A client's hello. A live seat token takes its seat back (even when
    /// locked, even after a new key); otherwise the join key lets it in, to
    /// a free seat or, past capacity or when it only watches, as a watcher.
    /// The `host`'s own connections (in process) need no key and are let in
    /// while locked: the server made their pipe itself, so whoever holds it
    /// is the host.
    #[allow(clippy::too_many_arguments)]
    pub fn hello(
        &mut self,
        conn: u64,
        join: &str,
        resume: &str,
        name: &str,
        watch: bool,
        host: bool,
        now: Instant,
        new_token: impl FnOnce() -> String,
    ) -> Result<Admit, Reject> {
        if !resume.is_empty() {
            if self.kicked.iter().any(|t| same_key(t, resume)) {
                return Err(Reject::Kicked);
            }
            let found = self.seats.iter().position(|s| {
                s.as_ref().is_some_and(|t| {
                    same_key(&t.token, resume) && t.dropped_at.is_none_or(|at| now.duration_since(at) <= RESUME_WINDOW)
                })
            });
            if let Some(seat) = found {
                let taken = self.seats[seat].as_mut().expect("found a taken seat");
                let displaced = taken.conn.replace(conn);
                taken.dropped_at = None;
                return Ok(Admit::Seat {
                    seat: seat as u32,
                    token: taken.token.clone(),
                    name: taken.name.clone(),
                    resumed: true,
                    displaced,
                });
            }
        }
        if !host && !same_key(&self.key, join) {
            return Err(Reject::Key);
        }
        if !host && self.locked {
            return Err(Reject::Locked);
        }
        let free = if watch {
            None
        } else {
            let wanted = clean_name(name, 0).to_lowercase();
            let free = |seat: usize| self.seats[seat].is_none();
            (0..self.seats.len())
                .find(|&s| free(s) && self.kept[s].as_deref() == Some(wanted.as_str()))
                .or_else(|| (0..self.seats.len()).find(|&s| free(s) && self.kept[s].is_none()))
                .or_else(|| (0..self.seats.len()).find(|&s| free(s)))
        };
        match free {
            Some(seat) => {
                self.kept[seat] = None;
                let token = new_token();
                let name = clean_name(name, seat as u32);
                self.seats[seat] =
                    Some(Taken { token: token.clone(), name: name.clone(), conn: Some(conn), dropped_at: None });
                Ok(Admit::Seat { seat: seat as u32, token, name, resumed: false, displaced: None })
            }
            None if self.watchers < MAX_SPECTATORS => {
                self.watchers += 1;
                Ok(Admit::Watch)
            }
            None => Err(Reject::Full),
        }
    }

    /// A seated connection ended without a goodbye: the seat waits for its
    /// token. A connection that was displaced by a resume changes nothing.
    pub fn dropped(&mut self, conn: u64, seat: u32, now: Instant) {
        if let Some(Some(t)) = self.seats.get_mut(seat as usize)
            && t.conn == Some(conn)
        {
            t.conn = None;
            t.dropped_at = Some(now);
        }
    }

    /// A seated player said goodbye: the seat is free now. Returns whether
    /// it was this connection's to free.
    pub fn bye(&mut self, conn: u64, seat: u32) -> bool {
        match self.seats.get_mut(seat as usize) {
            Some(slot) if slot.as_ref().is_some_and(|t| t.conn == Some(conn)) => {
                *slot = None;
                true
            }
            _ => false,
        }
    }

    /// A watcher's connection ended.
    pub fn watcher_left(&mut self) {
        self.watchers = self.watchers.saturating_sub(1);
    }

    /// Seats whose resume window ran out, freed.
    pub fn expire(&mut self, now: Instant) -> Vec<u32> {
        let mut freed = Vec::new();
        for (seat, slot) in self.seats.iter_mut().enumerate() {
            if slot.as_ref().and_then(|t| t.dropped_at).is_some_and(|at| now.duration_since(at) > RESUME_WINDOW) {
                *slot = None;
                freed.push(seat as u32);
            }
        }
        freed
    }

    /// The host removes a player: the seat is freed, its token stops
    /// working, and the connection playing it (if any) is returned to close.
    pub fn kick(&mut self, seat: u32) -> Option<Option<u64>> {
        let taken = self.seats.get_mut(seat as usize)?.take()?;
        if self.kicked.len() == KICKED_KEPT {
            self.kicked.remove(0);
        }
        self.kicked.push(taken.token);
        Some(taken.conn)
    }

    pub fn players(&self) -> Vec<Player> {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(seat, s)| {
                s.as_ref().map(|t| Player { seat: seat as u32, name: t.name.clone(), connected: t.conn.is_some() })
            })
            .collect()
    }

    /// Seats taken, played or waiting for a resume.
    pub fn seated(&self) -> u32 {
        self.seats.iter().filter(|s| s.is_some()).count() as u32
    }

    pub fn watchers(&self) -> u32 {
        self.watchers
    }
}

/// A name as shown: no control characters, trimmed, at most `NAME_CHARS`;
/// a seat's number when nothing is left.
fn clean_name(name: &str, seat: u32) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(NAME_CHARS).collect();
    let name = name.trim();
    debug_assert!(name.len() <= MAX_NAME * 4);
    if name.is_empty() { format!("Knight {}", seat + 1) } else { name.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    #[test]
    fn after_a_restart_each_player_takes_their_own_seat_back_by_name() {
        let now = Instant::now();
        let mut s = Sessions::new(KEY, 4);
        s.keep(&[(1, "Frank".into()), (2, "Jordan".into())]);
        let seat = |a: Admit| match a {
            Admit::Seat { seat, .. } => seat,
            Admit::Watch => panic!("seated"),
        };
        // The host first, then the friends in the other order, with stale tokens.
        assert_eq!(seat(s.hello(1, "", "", "host", false, true, now, || "t1".into()).unwrap()), 0);
        assert_eq!(seat(s.hello(2, KEY, "old-2", "jordan", false, false, now, || "t2".into()).unwrap()), 2);
        assert_eq!(seat(s.hello(3, KEY, "", "Frank ", false, false, now, || "t3".into()).unwrap()), 1);
        // A newcomer takes a seat nobody keeps.
        assert_eq!(seat(s.hello(4, KEY, "", "Ada", false, false, now, || "t4".into()).unwrap()), 3);
    }

    #[test]
    fn a_newcomer_takes_a_kept_seat_only_when_no_other_is_free() {
        let now = Instant::now();
        let mut s = Sessions::new(KEY, 2);
        s.keep(&[(0, "Frank".into())]);
        let seat = |a: Admit| match a {
            Admit::Seat { seat, .. } => seat,
            Admit::Watch => panic!("seated"),
        };
        assert_eq!(seat(s.hello(1, KEY, "", "Ada", false, false, now, || "t1".into()).unwrap()), 1);
        assert_eq!(seat(s.hello(2, KEY, "", "Bo", false, false, now, || "t2".into()).unwrap()), 0);
    }

    fn tokens() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("TOKEN{n}")
        }
    }

    fn join(s: &mut Sessions, conn: u64, now: Instant, token: &mut impl FnMut() -> String) -> Result<Admit, Reject> {
        s.hello(conn, KEY, "", "Gwen", false, false, now, token)
    }

    #[test]
    fn the_key_lets_players_in_to_free_seats() {
        let mut s = Sessions::new(KEY, 2);
        let now = Instant::now();
        let mut t = tokens();
        assert_eq!(
            join(&mut s, 1, now, &mut t),
            Ok(Admit::Seat { seat: 0, token: "TOKEN1".into(), name: "Gwen".into(), resumed: false, displaced: None })
        );
        // Any case.
        assert!(matches!(s.hello(2, &KEY.to_lowercase(), "", "x", false, false, now, &mut t), Ok(Admit::Seat { seat: 1, .. })));
        assert_eq!(s.hello(3, "WRONG", "", "x", false, false, now, &mut t), Err(Reject::Key));
        assert_eq!(s.seated(), 2);
    }

    #[test]
    fn past_capacity_and_watchers_only_watch_then_it_is_full() {
        let mut s = Sessions::new(KEY, 1);
        let now = Instant::now();
        let mut t = tokens();
        join(&mut s, 1, now, &mut t).unwrap();
        // A watcher asking for no seat, then the overflow.
        assert_eq!(s.hello(2, KEY, "", "eye", true, false, now, &mut t), Ok(Admit::Watch));
        for conn in 3..(2 + MAX_SPECTATORS as u64) {
            assert_eq!(join(&mut s, conn, now, &mut t), Ok(Admit::Watch));
        }
        assert_eq!(join(&mut s, 99, now, &mut t), Err(Reject::Full));
        s.watcher_left();
        assert_eq!(join(&mut s, 100, now, &mut t), Ok(Admit::Watch));
    }

    #[test]
    fn a_dropped_seat_resumes_within_its_window() {
        let mut s = Sessions::new(KEY, 4);
        let now = Instant::now();
        let mut t = tokens();
        let Ok(Admit::Seat { seat, token, .. }) = join(&mut s, 1, now, &mut t) else { panic!() };
        s.dropped(1, seat, now);
        assert_eq!(s.players(), vec![Player { seat, name: "Gwen".into(), connected: false }]);
        let later = now + RESUME_WINDOW - Duration::from_secs(1);
        assert_eq!(s.expire(later), Vec::<u32>::new());
        // The token works without the key (it may have changed) and while locked.
        s.set_key("NEWKEY");
        s.lock(true);
        assert_eq!(
            s.hello(2, "", &token, "", false, false, later, &mut t),
            Ok(Admit::Seat { seat, token: token.clone(), name: "Gwen".into(), resumed: true, displaced: None })
        );
    }

    #[test]
    fn a_resume_after_the_window_gets_a_new_seat() {
        let mut s = Sessions::new(KEY, 4);
        let now = Instant::now();
        let mut t = tokens();
        let Ok(Admit::Seat { seat, token, .. }) = join(&mut s, 1, now, &mut t) else { panic!() };
        s.dropped(1, seat, now);
        // Someone else sits down meanwhile.
        join(&mut s, 2, now, &mut t).unwrap();
        let late = now + RESUME_WINDOW + Duration::from_secs(1);
        assert_eq!(s.expire(late), vec![seat]);
        let again = s.hello(3, KEY, &token, "Gwen", false, false, late, &mut t).unwrap();
        assert!(matches!(again, Admit::Seat { resumed: false, .. }));
        let Admit::Seat { token: new_token, .. } = again else { panic!() };
        assert_ne!(new_token, token);
        // Even unexpired-but-late tokens (expire not yet run) are not taken back.
        let mut s = Sessions::new(KEY, 4);
        let Ok(Admit::Seat { seat, token, .. }) = join(&mut s, 1, now, &mut t) else { panic!() };
        s.dropped(1, seat, now);
        assert!(matches!(s.hello(2, KEY, &token, "", false, false, late, &mut t), Ok(Admit::Seat { resumed: false, .. })));
    }

    #[test]
    fn a_reload_takes_the_seat_from_the_old_connection() {
        let mut s = Sessions::new(KEY, 4);
        let now = Instant::now();
        let mut t = tokens();
        let Ok(Admit::Seat { seat, token, .. }) = join(&mut s, 1, now, &mut t) else { panic!() };
        let again = s.hello(2, KEY, &token, "", false, false, now, &mut t).unwrap();
        assert!(matches!(again, Admit::Seat { resumed: true, displaced: Some(1), .. }));
        // The old connection ending later does not drop the seat.
        s.dropped(1, seat, now);
        assert!(s.players()[0].connected);
        assert!(!s.bye(1, seat));
        assert!(s.bye(2, seat));
        assert_eq!(s.seated(), 0);
    }

    #[test]
    fn a_locked_session_lets_no_newcomer_in() {
        let mut s = Sessions::new(KEY, 4);
        let now = Instant::now();
        let mut t = tokens();
        s.lock(true);
        assert_eq!(join(&mut s, 1, now, &mut t), Err(Reject::Locked));
        s.lock(false);
        assert!(join(&mut s, 1, now, &mut t).is_ok());
    }

    #[test]
    fn the_host_needs_no_key_and_is_let_in_while_locked_but_not_past_capacity() {
        let mut s = Sessions::new(KEY, 1);
        let now = Instant::now();
        let mut t = tokens();
        s.lock(true);
        s.set_key("NEWKEY");
        assert!(matches!(s.hello(1, "", "", "Host", false, true, now, &mut t), Ok(Admit::Seat { seat: 0, .. })));
        // Past the seats, the host's next connection watches; a kicked token stays kicked.
        assert_eq!(s.hello(2, "", "", "Couch", false, true, now, &mut t), Ok(Admit::Watch));
        s.kick(0);
        assert_eq!(s.hello(3, "", "TOKEN1", "Host", false, true, now, &mut t), Err(Reject::Kicked));
    }

    #[test]
    fn a_kicked_player_loses_the_seat_and_the_token() {
        let mut s = Sessions::new(KEY, 4);
        let now = Instant::now();
        let mut t = tokens();
        let Ok(Admit::Seat { seat, token, .. }) = join(&mut s, 7, now, &mut t) else { panic!() };
        assert_eq!(s.kick(seat), Some(Some(7)));
        assert_eq!(s.kick(seat), None);
        assert_eq!(s.seated(), 0);
        assert_eq!(s.hello(8, KEY, &token, "", false, false, now, &mut t), Err(Reject::Kicked));
        // A new key keeps the old link out.
        s.set_key("FRESH");
        assert_eq!(join(&mut s, 9, now, &mut t), Err(Reject::Key));
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Gwen\u{7}  ", 0), "Gwen");
        assert_eq!(clean_name("", 4), "Knight 5");
        assert_eq!(clean_name(&"x".repeat(40), 0).len(), NAME_CHARS);
    }
}
