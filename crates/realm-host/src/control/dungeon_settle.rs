//! The realm's settlement: what the party banked and bought crosses into
//! the treasury in claims, and each step of a claim is saved (the game, then
//! the realm) before the next may spend it. While the server runs on its own
//! threads, the whole cycle runs on a settler thread with the realm handed
//! to it, so no claim, policy call or disk sync ever holds the host's frame;
//! the realm comes back on a later frame with what the cycle has to say.
//! Where the sim is stepped by hand, the same cycle runs in place.
use super::dungeon::DungeonView;
use crate::together_realm::Realm;
use crate::together_shooter as shooter;
use crate::together_sim::Link;
use angel_delve::realm::Spoils;
use angel_delve::rules::home::Home;
use angel_delve::rules::home_settlement::{
    HomeClaim, HomeCompletion, HomeContext, HomePolicyRequest, HomeResult,
};
use angel_delve::rules::host_commands::RealmClaim;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the realm waits after a home claim that changed nothing
/// before it claims again.
const HOME_QUIET: Duration = Duration::from_secs(2);

/// Claims settled in one cycle at most: a producer that keeps banking
/// cannot hold the settlement forever.
const CLAIMS_PER_CYCLE: usize = 16;

/// The game's side of a claim: the hand-stepped sim's own, or the link the
/// settler thread holds to the server's shared state.
pub(super) trait Ledger {
    fn claim_realm(&mut self) -> Result<Option<RealmClaim>, String>;
    fn acknowledge_realm(&mut self, claim: &RealmClaim) -> Result<(), String>;
    fn sync_home(&mut self, home: Home, treasury: Spoils) -> Result<(), String>;
    fn claim_home(&mut self, context: &HomeContext) -> Result<Option<HomeClaim>, String>;
    fn home_completion(&mut self, claim: &HomeClaim) -> Result<Option<HomeCompletion>, String>;
    fn evaluate_home(&mut self, request: &HomePolicyRequest) -> Result<HomeResult, String>;
    fn complete_home(&mut self, claim: &HomeClaim, result: HomeResult) -> Result<(), String>;
    fn acknowledge_home(
        &mut self,
        claim: &HomeClaim,
        completed: &HomeCompletion,
    ) -> Result<(), String>;
    /// Whether the run holds new facts for a home claim.
    fn home_facts(&mut self) -> bool;
    /// A forced checkpoint of the game.
    fn checkpoint(&mut self) -> Result<(), String>;
    fn saves(&self) -> bool;
}

/// The cycle in place, on the view's own sim.
pub(super) struct InPlace<'a>(pub &'a mut DungeonView);

impl Ledger for InPlace<'_> {
    fn claim_realm(&mut self) -> Result<Option<RealmClaim>, String> {
        self.0.shooter.claim_realm()
    }
    fn acknowledge_realm(&mut self, claim: &RealmClaim) -> Result<(), String> {
        self.0.shooter.acknowledge_realm(claim)
    }
    fn sync_home(&mut self, home: Home, treasury: Spoils) -> Result<(), String> {
        self.0.shooter.sync_home(home, treasury)
    }
    fn claim_home(&mut self, context: &HomeContext) -> Result<Option<HomeClaim>, String> {
        self.0.shooter.claim_home_with_context(context)
    }
    fn home_completion(&mut self, claim: &HomeClaim) -> Result<Option<HomeCompletion>, String> {
        self.0.shooter.home_completion(claim)
    }
    fn evaluate_home(&mut self, request: &HomePolicyRequest) -> Result<HomeResult, String> {
        self.0.shooter.evaluate_home(request)
    }
    fn complete_home(&mut self, claim: &HomeClaim, result: HomeResult) -> Result<(), String> {
        self.0.shooter.complete_home(claim, result)
    }
    fn acknowledge_home(
        &mut self,
        claim: &HomeClaim,
        completed: &HomeCompletion,
    ) -> Result<(), String> {
        self.0.shooter.acknowledge_home(claim, completed)
    }
    fn home_facts(&mut self) -> bool {
        self.0.shooter.home_facts_pending()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.0.checkpoint(Instant::now(), true)
    }
    fn saves(&self) -> bool {
        self.0.save_path.is_some()
    }
}

/// The cycle on the settler thread, through the server's shared state.
pub(super) struct Linked {
    pub link: Link,
    pub save_path: Option<PathBuf>,
    pub raid_serial: u64,
    pub saved: bool,
}

impl Ledger for Linked {
    fn claim_realm(&mut self) -> Result<Option<RealmClaim>, String> {
        self.link.claim_realm()
    }
    fn acknowledge_realm(&mut self, claim: &RealmClaim) -> Result<(), String> {
        self.link.acknowledge_realm(claim)
    }
    fn sync_home(&mut self, home: Home, treasury: Spoils) -> Result<(), String> {
        self.link.sync_home(home, treasury)
    }
    fn claim_home(&mut self, context: &HomeContext) -> Result<Option<HomeClaim>, String> {
        self.link.claim_home_with_context(context)
    }
    fn home_completion(&mut self, claim: &HomeClaim) -> Result<Option<HomeCompletion>, String> {
        self.link.home_completion(claim)
    }
    fn evaluate_home(&mut self, request: &HomePolicyRequest) -> Result<HomeResult, String> {
        self.link.evaluate_home(request)
    }
    fn complete_home(&mut self, claim: &HomeClaim, result: HomeResult) -> Result<(), String> {
        self.link.complete_home(claim, result)
    }
    fn acknowledge_home(
        &mut self,
        claim: &HomeClaim,
        completed: &HomeCompletion,
    ) -> Result<(), String> {
        self.link.acknowledge_home(claim, completed)
    }
    fn home_facts(&mut self) -> bool {
        self.link.home_facts_pending()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        let Some(path) = &self.save_path else {
            return Ok(());
        };
        self.link.checkpoint(path, self.raid_serial)?;
        self.saved = true;
        Ok(())
    }
    fn saves(&self) -> bool {
        self.save_path.is_some()
    }
}

/// What a cycle reads from the view, and what it has to say back.
pub(super) struct Io {
    pub host: String,
    pub bounties_unsaved: bool,
    pub ready_heard: usize,
    pub quiet_until: Option<Instant>,
    pub cues: Vec<String>,
    pub notice: Option<String>,
    pub saved: bool,
}

impl Io {
    pub(super) fn of(view: &DungeonView) -> Io {
        Io {
            host: super::host_name(),
            bounties_unsaved: view.bounties_unsaved,
            ready_heard: view.ready_heard,
            quiet_until: view.home_quiet_until,
            cues: Vec::new(),
            notice: None,
            saved: false,
        }
    }

    /// Tell the view what the cycle did.
    pub(super) fn apply(self, view: &mut DungeonView) {
        let now = Instant::now();
        for cue in &self.cues {
            view.chorus.cue(cue, now);
        }
        if let Some(notice) = self.notice {
            view.notice = notice;
        }
        view.ready_heard = self.ready_heard;
        view.bounties_unsaved &= self.bounties_unsaved;
        view.home_quiet_until = self.quiet_until;
        if self.saved {
            view.last_saved = Some(now);
        }
    }
}

/// Move what the run banked into the treasury, under each knight's name.
pub(super) fn realm_cycle(realm: &mut Realm, game: &mut impl Ledger, io: &mut Io) -> bool {
    // A claim remains in the game journal until both stores agree on its
    // identity. New spoils can accumulate while this exact claim retries.
    for _ in 0..CLAIMS_PER_CYCLE {
        let settled = (|| {
            let Some(claim) = game.claim_realm()? else {
                return Ok(None);
            };
            if realm.persistent() && !game.saves() {
                return Err("the game checkpoint path is unavailable".into());
            }
            // Save the emptied queues AND retained receipt in one core
            // capture before the destination is allowed to spend it.
            game.checkpoint()?;
            let result = realm.settle_claim(&claim, &io.host)?;
            game.acknowledge_realm(&claim)?;
            game.checkpoint()?;
            Ok::<_, String>(Some((claim, result)))
        })();
        let (claim, result) = match settled {
            Ok(Some(settled)) => settled,
            Ok(None) => return true,
            Err(error) => {
                io.notice = Some(format!("Could not save the realm: {error}"));
                return false;
            }
        };
        if result.applied && !claim.transfer.hauls.is_empty() {
            io.cues.push("banked".into());
        }
        if result.tribute_paid {
            io.cues.push("tribute".into());
        }
        if result.applied && result.affordable.len() > io.ready_heard {
            io.cues.push("treasury_ready".into());
        }
        io.ready_heard = result.affordable.len();
        if !result.lines.is_empty() {
            let mut notice = result.lines.join(" · ");
            if !result.affordable.is_empty() {
                notice.push_str(&format!(
                    " · the treasury can raise: {} (Tab → realm)",
                    result.affordable.join(", ")
                ));
            }
            io.notice = Some(notice);
        }
    }
    // Bound the work if a producer keeps banking; a restart/close may not
    // discard a backlog simply because one bounded transfer completed.
    io.notice = Some("The realm is still banking the party's spoils".into());
    false
}

/// Pay for what the party asked for at the Undercroft's plates out of the
/// treasury, and tell the run what now stands and what it can spend.
pub(super) fn home_cycle(realm: &mut Realm, game: &mut impl Ledger, io: &mut Io) -> bool {
    // A claim that changed nothing is followed by another at once: the
    // realm rests a moment before it asks again, unless the run has new
    // facts for it (an order, a sale, a deed, a mark), which settle now.
    if io.quiet_until.is_some_and(|until| Instant::now() < until) && !game.home_facts() {
        return true;
    }
    for _ in 0..CLAIMS_PER_CYCLE {
        let settled = (|| {
            let context = realm.home_context();
            game.sync_home(context.home.clone(), context.treasury.clone())?;
            let Some(claim) = game.claim_home(&context)? else {
                return Ok(None);
            };
            if realm.persistent() && !game.saves() {
                return Err("the game checkpoint path is unavailable".into());
            }
            // Persist reservations/facts before the destination is allowed
            // to pay. A replay always uses this same request and payload.
            game.checkpoint()?;
            if game.home_completion(&claim)?.is_none() {
                let result =
                    realm.prepare_home_claim(&claim, &io.host, io.bounties_unsaved, |request| {
                        game.evaluate_home(request)
                    })?;
                game.complete_home(&claim, result)?;
            }
            let completed = game
                .home_completion(&claim)?
                .ok_or("home delivery did not commit")?;
            // Only after world+delivery is durable may a refund commit and
            // its destination discard the large prepared result.
            game.checkpoint()?;
            realm.finish_home_claim(&claim, &completed)?;
            game.sync_home(realm.home.clone(), realm.treasury.clone())?;
            game.acknowledge_home(&claim, &completed)?;
            game.checkpoint()?;
            Ok::<_, String>(Some(completed))
        })();
        let completed = match settled {
            Ok(Some(completed)) => completed,
            Ok(None) => return true,
            Err(error) => {
                io.notice = Some(format!("Could not save the realm: {error}"));
                return false;
            }
        };
        io.bounties_unsaved = false;
        if !completed.result.changed
            && completed.result.said.is_empty()
            && completed.result.announced.is_empty()
            && completed.delivered.refund.is_empty()
        {
            io.quiet_until = Some(Instant::now() + HOME_QUIET);
            return true;
        }
        let built_or_opened = !completed.result.said.is_empty();
        for (cue, notice) in completed.result.said {
            io.cues.push(cue);
            io.notice = Some(notice);
        }
        if let Some(feat) = completed
            .result
            .announced
            .last()
            .and_then(|id| shooter::feats::feat(id))
        {
            let said = io.notice.clone().unwrap_or_default();
            io.notice = Some(if built_or_opened {
                format!("{said} · Deed: {} ({})", feat.name, feat.tier.name())
            } else {
                format!(
                    "Deed recorded: {}. {} ({} in the Herald's coffer, Fortune's hall)",
                    feat.name,
                    feat.says,
                    feat.tier.name()
                )
            });
        }
        if !completed.delivered.refund.is_empty() {
            io.notice = Some(format!(
                "Purchase could not be delivered; {} returned to the treasury",
                completed.delivered.refund.label()
            ));
        }
    }
    io.notice = Some("The realm is still settling the party's purchases".into());
    false
}

/// What a settler thread hands back.
pub(super) struct Settled {
    pub realm: Realm,
    pub io: Io,
}

/// One settlement cycle running off the frame, holding the realm.
pub struct Settling {
    done: mpsc::Receiver<Settled>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Settling {
    /// Settle on a thread of its own: the realm's claims, then the home's.
    pub(super) fn start(realm: Realm, game: Linked, io: Io) -> Settling {
        let (tx, done) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("realm-settle".into())
            .spawn(move || {
                let (mut realm, mut game, mut io) = (realm, game, io);
                if realm_cycle(&mut realm, &mut game, &mut io) {
                    home_cycle(&mut realm, &mut game, &mut io);
                }
                io.saved |= game.saved;
                let _ = tx.send(Settled { realm, io });
            })
            .expect("the realm's settler thread starts");
        Settling {
            done,
            thread: Some(thread),
        }
    }

    /// The cycle's result once it is done (waiting for it if asked). `Err`
    /// when the thread ended without handing the realm back.
    pub(super) fn poll(&mut self, wait: bool) -> Option<Result<Settled, ()>> {
        let got = if wait {
            self.done
                .recv()
                .map_err(|_| mpsc::TryRecvError::Disconnected)
        } else {
            self.done.try_recv()
        };
        match got {
            Ok(settled) => {
                self.join();
                Some(Ok(settled))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.join();
                Some(Err(()))
            }
        }
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Settling {
    fn drop(&mut self) {
        self.join();
    }
}

impl DungeonView {
    /// The realm as the screen shows it: the realm itself, or while it is
    /// away settling, the copy taken as it left.
    pub fn realm_shown(&self) -> Option<&Realm> {
        self.realm.as_ref().or(self.realm_copy.as_ref())
    }

    /// Whether a settlement cycle off the frame holds the realm now.
    pub fn settling_off_frame(&self) -> bool {
        self.settling.is_some()
    }

    /// Take back the realm from a settlement off the frame that is done, or
    /// (`wait`) once it is. A settler that ended without it leaves the realm
    /// to load again from its last save.
    pub fn collect_settled(&mut self, wait: bool) {
        let Some(settling) = self.settling.as_mut() else {
            return;
        };
        let Some(done) = settling.poll(wait) else {
            return;
        };
        self.settling = None;
        self.realm_copy = None;
        self.shooter.latch();
        match done {
            Ok(settled) => {
                self.realm = Some(settled.realm);
                settled.io.apply(self);
            }
            Err(()) => {
                self.realm = None;
                self.notice =
                    "The realm's settlement stopped; the realm loads again from its last save"
                        .into();
            }
        }
    }

    /// Start a settlement cycle off the frame with the realm handed to it.
    /// False where the server is stepped by hand: the cycle runs in place.
    pub(super) fn settle_off_frame(&mut self) -> bool {
        if self.settling.is_some() {
            return true;
        }
        let Some(link) = self.shooter.link() else {
            return false;
        };
        let Some(taken) = self.realm.take() else {
            return false;
        };
        let io = Io::of(self);
        self.realm_copy = Some(taken.clone());
        let game = Linked {
            link,
            save_path: self.save_path.clone(),
            raid_serial: self.raid_serial,
            saved: false,
        };
        self.settling = Some(Settling::start(taken, game, io));
        true
    }
}
