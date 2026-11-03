//! Opt-in, file-only game-night bridge. The worker is the only thread doing
//! filesystem access and Run serialization. A request is published only after
//! its identity and the entire Run have reached the atomic checkpoint.
use angel_delve::rules::{genie::Asked, phrasebook, Run, HZ};
use serde_json::{Value, json};
use std::{fs, io::Write, path::{Path, PathBuf}, sync::mpsc::{self, Receiver, SyncSender, TrySendError}, thread, time::Duration};

const MAX_FILE: u64 = 512 * 1024;
const CHECKPOINT: &str = "run.json";

#[derive(Clone)]
struct Pending { nonce: u64, asked: Asked, name: String }

struct State { run: Run, pending: Vec<Pending>, next: u64 }

enum Update { Book(String), Result(u64, Value) }

pub struct Night {
    to_worker: SyncSender<State>,
    from_worker: Receiver<Update>,
    pending: Vec<Pending>,
    next: u64,
    recovered: bool,
    last_sent: u64,
    dirty: bool,
}

impl Night {
    pub fn recovered(&self) -> bool { self.recovered }

    pub fn open(dir: &Path) -> Result<(Self, Option<Run>), String> {
        fs::create_dir_all(dir).map_err(|e| format!("night directory: {e}"))?;
        fs::create_dir_all(dir.join("results")).map_err(|e| format!("night results: {e}"))?;
        let saved = match fs::read(dir.join(CHECKPOINT)) {
            Ok(bytes) => {
                if bytes.len() > 32 * 1024 * 1024 { return Err("night checkpoint too large".into()); }
                let v: Value = serde_json::from_slice(&bytes).map_err(|e| format!("night checkpoint: {e}"))?;
                let run = serde_json::from_value(v.get("run").cloned().ok_or("night checkpoint missing Run")?)
                    .map_err(|e| format!("night Run: {e}"))?;
                let pending = parse_pending(v.get("pending").ok_or("night checkpoint missing pending")?)?;
                let next = v["next_nonce"].as_u64().ok_or("night checkpoint missing nonce")?;
                if pending.iter().any(|p| p.nonce >= next) { return Err("night checkpoint invalid nonce".into()); }
                Some((run, pending, next))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("night checkpoint: {e}")),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let (reply, updates) = mpsc::sync_channel(32);
        let dir = dir.to_path_buf();
        let durable_pending = saved.as_ref().map(|(_, pending, _)| pending.clone()).unwrap_or_default();
        thread::Builder::new().name("delve-night-files".into()).spawn(move || worker(dir, rx, reply, durable_pending))
            .map_err(|e| format!("night worker: {e}"))?;
        let recovered = saved.is_some();
        let (run, pending, next) = match saved { Some((r, p, n)) => (Some(r), p, n), None => (None, Vec::new(), 1) };
        Ok((Self { to_worker: tx, from_worker: updates, pending, next, recovered, last_sent: 0, dirty: true }, run))
    }

    pub fn tick(&mut self, run: &mut Run) {
        // Never remove an Asked until a verified result (or explicit failure)
        // settles it. Keeping it in Run also keeps it in external snapshots.
        for asked in &run.asked.clone() {
            if !self.pending.iter().any(|p| p.asked == *asked) {
                let Some(next) = self.next.checked_add(1) else { eprintln!("night nonce exhausted"); break };
                let name = run.players.get(&asked.knight).map(|h| h.name.clone()).unwrap_or_default();
                self.pending.push(Pending { nonce: self.next, asked: asked.clone(), name });
                self.next = next;
                if !asked.cosmetic { run.lamp_sent(asked.knight, &asked.words); }
                self.dirty = true;
            }
        }
        // Worker sends only bounded files. No disk operations on this thread.
        for _ in 0..32 {
            match self.from_worker.try_recv() {
                Ok(Update::Book(text)) => { phrasebook::install(text, Vec::new()); }
                Ok(Update::Result(nonce, result)) => {
                    if let Some(pos) = self.pending.iter().position(|p| p.nonce == nonce)
                        && settle(run, &self.pending[pos], &result) {
                        self.pending.remove(pos);
                        self.dirty = true;
                    }
                }
                Err(_) => break,
            }
        }
        if self.dirty || run.tick.saturating_sub(self.last_sent) >= u64::from(HZ) * 3 {
            let state = State { run: run.clone(), pending: self.pending.clone(), next: self.next };
            match self.to_worker.try_send(state) {
                Ok(()) => { self.last_sent = run.tick; self.dirty = false; }
                Err(TrySendError::Full(_)) => {},
                Err(TrySendError::Disconnected(_)) => eprintln!("night writer stopped; checkpoint not saved"),
            }
        }
    }
}

fn parse_pending(v: &Value) -> Result<Vec<Pending>, String> {
    let arr = v.as_array().ok_or("night pending is not an array")?;
    if arr.len() > 256 { return Err("night pending too large".into()); }
    arr.iter().map(|v| {
        Ok(Pending {
            nonce: v["nonce"].as_u64().ok_or("night pending nonce")?,
            asked: serde_json::from_value(v["asked"].clone()).map_err(|e| format!("night asked: {e}"))?,
            name: v["player"].as_str().ok_or("night player")?.to_owned(),
        })
    }).collect()
}

// Result schema: {"nonce": N, "asked": <the exact asked object>,
// "wish_id": "learned-id"} or {"nonce": N, "asked": ..., "fail": "reason"}.
// A named wish MUST be in the learned declarative book; never substitute a
// nearby stock phrase. Failures refund by the same rules as the cockpit.
fn settle(run: &mut Run, p: &Pending, v: &Value) -> bool {
    if v["nonce"].as_u64() != Some(p.nonce) ||
        serde_json::from_value::<Asked>(v["asked"].clone()).ok().as_ref() != Some(&p.asked) ||
        !run.asked.contains(&p.asked) { return false; }
    let fail = v.get("fail").and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 512);
    let wish = v.get("wish_id").and_then(Value::as_str).and_then(phrasebook::get);
    if (fail.is_some() as u8 + wish.is_some() as u8) != 1 { return false; }
    let asked = &p.asked;
    if asked.cosmetic {
        if let Some(w) = wish {
            if !w.learned || phrasebook::find(&asked.words).as_ref().map(|x| x.id.as_str()) != Some(w.id.as_str()) { return false; }
            let Some(mut card) = (0..w.tiers.len()).rev().find_map(|tier| w.card(tier).ok().filter(|c| c.looks.worn.is_some())) else { return false };
            if asked.cosmetic_name.is_empty() {
                if run.wear_mirror_card(asked.knight, &asked.words, &card).is_err() { return false; }
            } else {
                card.id = angel_delve::rules::cosmetic_wish::key(&asked.cosmetic_name);
                card.text = asked.words.clone();
                run.update_mirror_paint(&card);
            }
        }
        run.asked.remove(run.asked.iter().position(|a| a == asked).unwrap());
        return true;
    }
    if let Some(why) = fail {
        run.lamp_dropped(asked.knight, &asked.words);
        run.refund_told(asked.knight, asked.great, why);
    } else if let Some(w) = wish {
        if !w.learned { return false; }
        // Learned entries must actually match the player's words. The file
        // cannot use a different ID to grant an unrelated card.
        let deck = run.players.get(&asked.knight).map(|h| h.deck.clone()).unwrap_or_default();
        if phrasebook::find_for(&asked.words, &deck).as_ref().map(|x| x.id.as_str()) != Some(w.id.as_str()) { return false; }
        let was = run.aim_wish(asked.knight, &w, asked.target);
        match run.grant(asked.knight, &w) {
            Ok(mut name) => {
                for _ in 1..asked.steps {
                    if let Some(more) = run.grant_more(asked.knight, &w) { name = more; } else { break; }
                }
                run.sealed(asked.knight, &w, asked.great, asked.fills);
                run.lamp_landed(asked.knight, &asked.words, &name);
            }
            Err(why) => {
                run.aim_wish(asked.knight, &w, was);
                run.lamp_dropped(asked.knight, &asked.words);
                run.refund_told(asked.knight, asked.great, &why);
            }
        }
    }
    run.asked.remove(run.asked.iter().position(|a| a == asked).unwrap());
    true
}

fn atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(tmp, path)?;
    fs::File::open(path.parent().unwrap())?.sync_all()
}

fn bounded(path: &Path) -> Option<Vec<u8>> {
    let f = fs::File::open(path).ok()?;
    if f.metadata().ok()?.len() > MAX_FILE { return None; }
    use std::io::Read;
    let mut bytes = Vec::new();
    f.take(MAX_FILE + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_FILE).then_some(bytes)
}

fn worker(dir: PathBuf, rx: Receiver<State>, tx: SyncSender<Update>, mut durable_pending: Vec<Pending>) {
    let mut state: Option<State> = None;
    let mut published = std::collections::HashSet::new();
    let mut last_book = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(s) => state = Some(s),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {},
        }
        if let Some(s) = state.take() {
            let pending: Vec<Value> = s.pending.iter().map(|p| json!({"nonce":p.nonce,"asked":p.asked,"player":p.name})).collect();
            let v = json!({"version":1,"run":s.run,"pending":pending,"next_nonce":s.next});
            match serde_json::to_vec(&v).map_err(|e| e.to_string())
                .and_then(|bytes| atomic(&dir.join(CHECKPOINT), &bytes).map_err(|e| e.to_string())) {
                Ok(()) => {
                    durable_pending = s.pending.clone();
                    // Rebuild the outbox from checkpoint on restart; publishing
                    // duplicates is safe because nonce is the request key.
                    for p in &s.pending {
                        if published.insert(p.nonce) {
                            let line = json!({"nonce":p.nonce,"asked":p.asked,"player":p.name});
                            let result = fs::OpenOptions::new().create(true).append(true).open(dir.join("wishes.jsonl"))
                                .and_then(|mut f| writeln!(f, "{line}").and_then(|_| f.sync_all()));
                            if let Err(e) = result { published.remove(&p.nonce); eprintln!("night outbox: {e}"); }
                        }
                    }
                }
                Err(e) => { eprintln!("night checkpoint: {e}"); state = Some(s); }
            }
        }
        if let Some(bytes) = bounded(&dir.join("phrasebook.txt")) && bytes != last_book {
            if let Ok(text) = String::from_utf8(bytes.clone()) {
                let (_, errors) = phrasebook::read_checked(&text);
                if errors.is_empty() && tx.try_send(Update::Book(text)).is_ok() { last_book = bytes; }
            }
        }
        // A result is eligible only after its request is durably checkpointed.
        for p in &durable_pending {
            if let Some(bytes) = bounded(&dir.join("results").join(format!("{}.json", p.nonce)))
                && let Ok(v) = serde_json::from_slice(&bytes) {
                let _ = tx.try_send(Update::Result(p.nonce, v));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_cosmetic_failure_does_not_refund_gameplay_wishes() {
        let mut run = Run::new(7, 1, None);
        let asked = Asked {
            cosmetic: true, cosmetic_name: String::new(), knight: 1,
            words: "a look".into(), steps: 1, great: false, fills: false,
            fairy: false, star: false, making: None, target: Default::default(),
        };
        run.asked.push(asked.clone());
        let before = serde_json::to_value(&run.players[&1]).unwrap();
        let pending = Pending { nonce: 9, asked: asked.clone(), name: "knight".into() };
        assert!(settle(&mut run, &pending, &json!({"nonce":9,"asked":asked,"fail":"no"})));
        assert!(run.asked.is_empty());
        assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
    }

    #[test]
    fn night_worker_polls_recovered_durable_requests() {
        let dir = PathBuf::from("target").join(format!("night-worker-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("results")).unwrap();
        let asked = Asked {
            cosmetic: false, cosmetic_name: String::new(), knight: 1,
            words: "a wish".into(), steps: 1, great: false, fills: false,
            fairy: false, star: false, making: None, target: Default::default(),
        };
        let pending = Pending { nonce: 7, asked: asked.clone(), name: "knight".into() };
        fs::write(dir.join("results/7.json"), json!({"nonce":7,"asked":asked,"fail":"no"}).to_string()).unwrap();
        let (send, recv) = mpsc::sync_channel(1);
        let (reply, updates) = mpsc::sync_channel(32);
        let worker_dir = dir.clone();
        let handle = thread::spawn(move || worker(worker_dir, recv, reply, vec![pending]));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            Update::Result(7, value) => assert_eq!(value["fail"], "no"),
            _ => panic!("expected the recovered wish result"),
        }
        drop(send);
        handle.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
