//! The Round Table session: the board ([`angel_table`]) on the
//! engine's server. The host's seat (0) is the arbiter; every other
//! seat is a friend's Angel, joined by the usual link. Board ops ride
//! one-off `action`s; the board itself replicates as a document tree
//! (`DocRep`), so a guest's cockpit renders what the host publishes
//! and never a local guess. The board's JSONL log is rewritten on
//! every accepted op — the table's checkpoint and export in one.
//!
//! The host cockpit applies its own ops to the same shared board the
//! server publishes (like `Sim::as_mut` edits between ticks): one
//! board, one lock, no second truth.

use angel_serve::client::{LinkOptions, LinkStatus, NativeLink};
use angel_serve::{
    DocRep, DocTick, Events, Everything, GameServer, Presence, Rates, ServeConfig, Server,
};
use angel_table::client::TableClient;
use angel_table::{ARBITER, Board, Denied, Op, TableCmd, act};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

const SEATS: u32 = 8;
const TICK_HZ: u32 = 2;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct Shared {
    board: Mutex<Board>,
    tick: AtomicU64,
    log_path: Option<PathBuf>,
}

impl Shared {
    fn load(log_path: Option<PathBuf>) -> Shared {
        let board = log_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| Board::import_jsonl(&text).ok())
            .unwrap_or_default();
        Shared { board: Mutex::new(board), tick: AtomicU64::new(0), log_path }
    }

    pub(crate) fn apply(&self, seat: u32, op: Op) -> Result<u64, Denied> {
        let tick = self.tick.load(Ordering::Relaxed);
        let mut board = lock(&self.board);
        let seq = board.apply(seat, tick, op)?;
        if let Some(path) = &self.log_path {
            let text = board.export_jsonl();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = crate::platform::workspace_store::write_private_atomic_exact(
                path,
                text.as_bytes(),
            );
        }
        Ok(seq)
    }

    pub(crate) fn board(&self) -> Board {
        lock(&self.board).clone()
    }

    pub(crate) fn bundle(&self, id: u64) -> Option<Vec<u8>> {
        lock(&self.board).bundle(id).map(<[u8]>::to_vec)
    }
}

struct TableServer {
    shared: Arc<Shared>,
    uploads: std::collections::BTreeMap<u32, act::Assembler>,
}

impl GameServer for TableServer {
    type Cmd = TableCmd;
    type Rep = DocRep<Everything>;
    const GAME: &'static str = angel_table::GAME;

    fn rates(&self) -> Rates {
        Rates { tick_hz: TICK_HZ, snapshot_hz: TICK_HZ }
    }

    fn seats(&self) -> u32 {
        SEATS
    }

    fn params(&self) -> Vec<u8> {
        Vec::new()
    }

    fn replicator(&self) -> Self::Rep {
        DocRep(Everything)
    }

    fn decode_cmd(bytes: &[u8]) -> Option<TableCmd> {
        match bytes {
            [] | [0] => Some(TableCmd),
            _ => None,
        }
    }

    fn join(&mut self, seat: u32, name: &str, _events: &mut Events) {
        lock(&self.shared.board).join(seat, name);
    }

    fn leave(&mut self, seat: u32, _events: &mut Events) {
        lock(&self.shared.board).leave(seat);
    }

    fn action(&mut self, seat: Option<u32>, kind: u32, body: &[u8], _events: &mut Events) {
        let Some(seat) = seat else { return };
        let Some((kind, body)) = self.uploads.entry(seat).or_default().feed(kind, body) else {
            return;
        };
        if let Some(op) = act::decode(kind, &body) {
            let _ = self.shared.apply(seat, op);
        }
    }

    fn presence(&mut self, tick: u32, _seats: &[Presence]) {
        self.shared.tick.store(u64::from(tick), Ordering::Relaxed);
    }

    fn step(&mut self, tick: u32, _cmds: &[Option<TableCmd>], _events: &mut Events) {
        lock(&self.shared.board).expire(u64::from(tick));
    }

    fn publish(&mut self, _tick: u32) -> DocTick {
        DocTick { tree: lock(&self.shared.board).to_value() }
    }
}

pub(crate) struct TableHost {
    pub(crate) shared: Arc<Shared>,
    server: Server,
    anchor: NativeLink<TableClient>,
    epoch: Instant,
}

fn anchor(server: &Server, host_name: &str) -> Result<NativeLink<TableClient>, String> {
    let conn = server
        .connect_local()
        .map_err(|e| format!("the arbiter could not take seat 0: {e}"))?;
    Ok(NativeLink::local(conn, host_name, LinkOptions::default()))
}

impl TableHost {
    pub(crate) fn start(workspace: &Path, host_name: &str) -> Result<(TableHost, Vec<String>), String> {
        let log_path = workspace.join(".angel").join("round-table.jsonl");
        let shared = Arc::new(Shared::load(Some(log_path)));
        lock(&shared.board).join(ARBITER, host_name);
        let bind: SocketAddr = angel_games::door::bind(&angel_games::door::default_bind(8797))?;
        let server = Server::start(
            TableServer { shared: Arc::clone(&shared), uploads: Default::default() },
            ServeConfig::new(bind),
        )
        .map_err(|e| format!("the table's server did not start: {e}"))?;
        let anchor = anchor(&server, host_name)?;
        let links = server.links();
        Ok((TableHost { shared, server, anchor, epoch: Instant::now() }, links))
    }

    pub(crate) fn poll(&mut self) {
        let now_ms = self.epoch.elapsed().as_secs_f64() * 1000.0;
        self.anchor.tick(now_ms);
    }

    pub(crate) fn links(&self) -> Vec<String> {
        self.server.links()
    }

    pub(crate) fn stop(mut self) {
        self.anchor.bye();
        self.server.stop();
    }
}

pub(crate) struct TableGuest {
    target: String,
    link: NativeLink<TableClient>,
    epoch: Instant,
    outbox: VecDeque<(u32, Vec<u8>)>,
}

static GUEST_TOKENS: std::sync::LazyLock<Mutex<std::collections::HashMap<String, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(load_tokens_from_disk()));

fn guest_tokens_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("ANGEL_ROUND_TABLE_TOKENS") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    if let Ok(angel_home) = std::env::var("ANGELX_HOME") {
        let trimmed = angel_home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("round-table-tokens.json"));
        }
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".angelX").join("round-table-tokens.json"))
}

fn load_tokens_from_disk() -> std::collections::HashMap<String, String> {
    guest_tokens_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_tokens_to_disk(tokens: &std::collections::HashMap<String, String>) {
    let Some(path) = guest_tokens_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(tokens) {
        let _ = crate::platform::workspace_store::write_private_atomic_exact(
            &path,
            text.as_bytes(),
        );
    }
}

fn lookup_guest_token(target: &str) -> String {
    let mut tokens = lock(&GUEST_TOKENS);
    if let Some(tok) = tokens.get(target) {
        return tok.clone();
    }
    let disk = load_tokens_from_disk();
    if let Some(tok) = disk.get(target).cloned() {
        tokens.insert(target.to_string(), tok.clone());
        return tok;
    }
    String::new()
}

impl TableGuest {
    pub(crate) fn connect(link: &str, name: &str) -> Result<TableGuest, String> {
        let target = link.trim().to_string();
        let resume = lookup_guest_token(&target);
        let link = NativeLink::connect(
            link,
            name,
            LinkOptions { resume, ..LinkOptions::default() },
        )?;
        Ok(TableGuest { target, link, epoch: Instant::now(), outbox: VecDeque::new() })
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn poll(&mut self) {
        let now_ms = self.epoch.elapsed().as_secs_f64() * 1000.0;
        self.link.poll(now_ms);
        let token = self.link.seat_token();
        if !token.is_empty() {
            let mut tokens = lock(&GUEST_TOKENS);
            if tokens.get(&self.target).map(String::as_str) != Some(token) {
                tokens.insert(self.target.clone(), token.to_string());
                save_tokens_to_disk(&tokens);
            }
        }
        if matches!(self.link.status(), LinkStatus::Playing) {
            while let Some((kind, body)) = self.outbox.pop_front() {
                self.link.action(kind, &body);
            }
        }
    }

    pub(crate) fn cast(&mut self, op: &Op) {
        let (kind, body) = act::encode(op);
        for (kind, body) in act::pieces(kind, &body) {
            if matches!(self.link.status(), LinkStatus::Playing) && self.outbox.is_empty() {
                self.link.action(kind, &body);
            } else {
                self.outbox.push_back((kind, body));
            }
        }
    }

    pub(crate) fn board(&mut self) -> Option<Board> {
        self.poll();
        self.link.runtime().game().and_then(TableClient::board)
    }

    pub(crate) fn seat(&self) -> Option<u32> {
        self.link.runtime().seat()
    }

    pub(crate) fn status_line(&self) -> String {
        match self.link.status() {
            LinkStatus::Connecting => "connecting".into(),
            LinkStatus::Playing => match self.seat() {
                Some(seat) => format!("seated (seat {seat})"),
                None => "watching (no seat)".into(),
            },
            LinkStatus::Reconnecting => "reconnecting".into(),
            LinkStatus::Rejected(_, why) => format!("rejected: {why}"),
            LinkStatus::Replaced => "replaced by another connection".into(),
            LinkStatus::Closed => "closed".into(),
        }
    }

    pub(crate) fn bye(&mut self) {
        let mut tokens = lock(&GUEST_TOKENS);
        if tokens.remove(&self.target).is_some() {
            save_tokens_to_disk(&tokens);
        }
        drop(tokens);
        self.link.bye();
    }

    #[cfg(test)]
    pub(crate) fn close_without_bye(&mut self) {
        self.link.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_table::{Op, State};
    use std::time::Duration;

    fn until(
        anchor: &mut NativeLink<TableClient>,
        epoch: Instant,
        guest: &mut TableGuest,
        what: &str,
        pred: impl Fn(&Board) -> bool,
    ) -> Board {
        let until = Instant::now() + Duration::from_secs(8);
        loop {
            anchor.tick(epoch.elapsed().as_secs_f64() * 1000.0);
            if let Some(board) = guest.board() {
                if pred(&board) {
                    return board;
                }
            }
            assert!(Instant::now() < until, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    #[test]
    fn two_seats_share_a_board_and_only_the_arbiter_decides() {
        let shared = Arc::new(Shared::load(None));
        lock(&shared.board).join(ARBITER, "Host");
        let server = Server::start(
            TableServer { shared: Arc::clone(&shared), uploads: Default::default() },
            ServeConfig::new("127.0.0.1:0".parse().unwrap()),
        )
        .expect("the table server starts");
        let epoch = Instant::now();
        let mut anchor = anchor(&server, "Host").expect("the arbiter takes seat 0");
        let link = server
            .links()
            .into_iter()
            .find(|l| l.contains("127.0.0.1"))
            .expect("a loopback link");
        let mut guest = TableGuest::connect(&link, "Percival").expect("a knight connects");
        guest.cast(&Op::NewIssue { title: "Oil the gate".into(), body: "It squeaks.".into() });
        until(&mut anchor, epoch, &mut guest, "both seats", |b| b.seats.len() >= 2);
        assert_eq!(anchor.runtime().seat(), Some(ARBITER), "the anchor holds seat 0");
        assert_eq!(guest.seat(), Some(1), "the first guest is seat 1, never the arbiter");

        let board = until(&mut anchor, epoch, &mut guest, "the proposal", |b| !b.issues.is_empty());
        let id = *board.issues.keys().next().unwrap();

        guest.cast(&Op::Accept { id });
        std::thread::sleep(Duration::from_millis(600));
        anchor.tick(epoch.elapsed().as_secs_f64() * 1000.0);
        let board = guest.board().expect("a board");
        assert_eq!(board.issues[&id].state, State::Proposed, "guests cannot accept");

        shared.apply(ARBITER, Op::Accept { id }).expect("the arbiter accepts");
        until(&mut anchor, epoch, &mut guest, "acceptance", |b| b.issues[&id].state == State::Accepted);

        guest.cast(&Op::Claim { id });
        until(&mut anchor, epoch, &mut guest, "the claim", |b| b.issues[&id].state == State::InProgress);

        let bundle: Vec<u8> = (0..3000u32).map(|i| (i % 253) as u8).collect();
        guest.cast(&Op::Submit {
            id,
            summary: "oiled".into(),
            evidence: "tests green".into(),
            bundle: bundle.clone(),
        });
        until(&mut anchor, epoch, &mut guest, "the submission", |b| {
            b.issues[&id].state == State::Submitted
        });
        assert_eq!(shared.bundle(id).as_deref(), Some(bundle.as_slice()), "arbiter holds the bytes");

        shared
            .apply(ARBITER, Op::Merge { id, commit: "abc123".into() })
            .expect("the arbiter records the merge");
        let board = until(&mut anchor, epoch, &mut guest, "the merge", |b| {
            b.issues[&id].state == State::Merged
        });
        assert_eq!(board.issues[&id].merged_commit.as_deref(), Some("abc123"));

        guest.bye();
        anchor.bye();
        server.stop();
    }

    #[test]
    fn adversarial_guests_cannot_forge_any_arbiter_decision_and_reconnect_reclaims_seat() {
        let shared = Arc::new(Shared::load(None));
        lock(&shared.board).join(ARBITER, "Host");
        let server = Server::start(
            TableServer { shared: Arc::clone(&shared), uploads: Default::default() },
            ServeConfig::new("127.0.0.1:0".parse().unwrap()),
        )
        .expect("the table server starts");
        let epoch = Instant::now();
        let mut anchor = anchor(&server, "Host").expect("the arbiter takes seat 0");
        let link = server
            .links()
            .into_iter()
            .find(|l| l.contains("127.0.0.1"))
            .expect("a loopback link");
        let mut guest = TableGuest::connect(&link, "Mordred").expect("guest connects");
        guest.cast(&Op::NewIssue { title: "Guard the gate".into(), body: "Check locks.".into() });
        let board = until(&mut anchor, epoch, &mut guest, "the proposal", |b| !b.issues.is_empty());
        let id = *board.issues.keys().next().unwrap();
        assert_eq!(guest.seat(), Some(1));

        for op in [
            Op::Accept { id },
            Op::Reject { id },
            Op::Merge { id, commit: "deadbeef".into() },
            Op::Close { id },
        ] {
            guest.cast(&op);
        }
        guest.cast(&Op::Comment { id, text: "probe-proposed".into() });
        let board = until(&mut anchor, epoch, &mut guest, "proposed fence", |b| {
            b.issues[&id].comments.iter().any(|c| c.text == "probe-proposed")
        });
        assert_eq!(board.issues[&id].state, State::Proposed, "guest forged a Proposed transition");
        assert!(board.issues[&id].merged_commit.is_none());

        shared.apply(ARBITER, Op::Accept { id }).unwrap();
        guest.cast(&Op::Claim { id });
        guest.cast(&Op::Submit {
            id,
            summary: "done".into(),
            evidence: "ok".into(),
            bundle: b"patch".to_vec(),
        });
        until(&mut anchor, epoch, &mut guest, "submitted", |b| {
            b.issues[&id].state == State::Submitted
        });
        for op in [
            Op::Merge { id, commit: "forged-sha".into() },
            Op::Reject { id },
            Op::Close { id },
        ] {
            guest.cast(&op);
        }
        guest.cast(&Op::Comment { id, text: "probe-submitted".into() });
        let board = until(&mut anchor, epoch, &mut guest, "submitted fence", |b| {
            b.issues[&id].comments.iter().any(|c| c.text == "probe-submitted")
        });
        assert_eq!(board.issues[&id].state, State::Submitted, "guest forged a Submitted decision");
        assert!(board.issues[&id].merged_commit.is_none());

        guest.close_without_bye();
        assert!(
            load_tokens_from_disk().contains_key(link.trim()),
            "seat token must be persisted on disk while seated"
        );
        lock(&GUEST_TOKENS).remove(link.trim());
        let mut resumed = TableGuest::connect(&link, "Mordred").expect("guest reconnects");
        until(&mut anchor, epoch, &mut resumed, "resumed board", |b| !b.issues.is_empty());
        assert_eq!(resumed.seat(), Some(1), "reconnected guest must reclaim seat 1 via disk seat_token");

        resumed.bye();
        assert!(
            !load_tokens_from_disk().contains_key(link.trim()),
            "bye() must remove the seat token from disk"
        );
        anchor.bye();
        server.stop();
    }
}
