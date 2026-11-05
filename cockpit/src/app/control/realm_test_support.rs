//! Test-only forwards to the realm host's controls that the cockpit's tests
//! drive by their old names (they were `impl App` methods before the cut);
//! each runs the control in the cockpit's context like a real call.

use super::*;
use angel_realm_host::control::Lesson;
use angel_realm_host::together_painter::Target;
use angel_realm_host::together_shooter::knights::Part;
use std::time::Instant;

impl App {
    pub(crate) fn joined_actor(&self) -> u32 { self.realm.dungeon.joined_actor() }
    pub(crate) fn joined_knight(&self) -> Option<serde_json::Value> { self.realm.dungeon.joined_knight() }
    pub(crate) fn joined_card_pages(&self) -> usize { self.realm.dungeon.joined_card_pages() }
    pub(crate) fn wish_upgrades(&self, id: u32) -> (Vec<angel_delve::rules::genie::Upgrade>, Option<&'static str>) { self.realm.dungeon.wish_upgrades(id) }
    pub(crate) fn wish_offer_open(&self, id: u32) -> bool { self.realm.dungeon.wish_offer_open(id) }
    pub(crate) fn can_leave_dungeon_activity(&self, id: u32) -> bool { self.realm.dungeon.can_leave_dungeon_activity(id) }
    pub(crate) fn open_lamp_wish(&mut self) { self.with_realm(|r, cx| r.session(cx).open_lamp_wish()); }
    pub(crate) fn mirror_key(&mut self, key: event::KeyEvent) -> Option<bool> { self.with_realm(|r, cx| r.session(cx).mirror_key(key)) }
    pub(crate) fn activity_key(&mut self, key: event::KeyEvent) -> bool { self.with_realm(|r, cx| r.session(cx).activity_key(key)) }
    pub(crate) fn flush_speculative(&mut self) -> bool { self.with_realm(|r, cx| r.session(cx).flush_speculative()) }
    pub(crate) fn sync_wish_session(&mut self) -> bool { self.with_realm(|r, cx| r.session(cx).sync_wish_session()) }
    pub(crate) fn drain_detached_lessons(&mut self) { self.with_realm(|r, cx| r.session(cx).drain_detached_lessons()); }
    pub(crate) fn settle_home(&mut self) -> bool {
        self.with_realm(|r, cx| r.session(cx).settle_home())
    }

    pub(crate) fn settle_realm(&mut self) -> bool {
        self.with_realm(|r, cx| r.session(cx).settle_realm())
    }

    pub(crate) fn settle_off_frame(&mut self) -> bool {
        self.with_realm(|r, cx| r.session(cx).settle_off_frame())
    }

    pub(crate) fn advance_shooter_at(&mut self, now: Instant) {
        self.with_realm(|r, cx| r.session(cx).advance_shooter_at(now));
    }

    pub(crate) fn advance_joined(&mut self) {
        self.with_realm(|r, cx| r.session(cx).advance_joined());
    }

    pub(crate) fn advance_dungeon_guest(&mut self) {
        self.with_realm(|r, cx| r.session(cx).advance_dungeon_guest());
    }

    pub(crate) fn watch_learning(&mut self) {
        self.with_realm(|r, cx| r.session(cx).watch_learning());
    }

    pub(crate) fn teach_asked(&mut self) {
        self.with_realm(|r, cx| r.session(cx).teach_asked());
    }

    pub(crate) fn teach_next(&mut self) {
        self.with_realm(|r, cx| r.session(cx).teach_next());
    }

    pub(crate) fn teach_claim(&mut self, claim: angel_delve::rules::wish_jobs::Claim) {
        self.with_realm(|r, cx| r.session(cx).teach_claim(claim));
    }

    pub(crate) fn teach_spoken(&mut self, open: Vec<(u32, String, u64, angel_realm_host::together_shooter::dead_wishes::Target)>) {
        self.with_realm(|r, cx| r.session(cx).teach_spoken(open));
    }

    pub(crate) fn take_grants(&mut self) {
        self.with_realm(|r, cx| r.session(cx).take_grants());
    }

    pub(crate) fn watch_reforge(&mut self) {
        self.with_realm(|r, cx| r.session(cx).watch_reforge());
    }

    pub(crate) fn watch_brewing(&mut self) {
        self.with_realm(|r, cx| r.session(cx).watch_brewing());
    }

    pub(crate) fn watch_painters(&mut self) {
        self.with_realm(|r, cx| r.session(cx).watch_painters());
    }

    pub(crate) fn watch_drafting(&mut self) {
        self.with_realm(|r, cx| r.session(cx).watch_drafting());
    }

    pub(crate) fn watch_delve_gate(&mut self) {
        self.with_realm(|r, cx| r.session(cx).watch_delve_gate());
    }

    pub(crate) fn reforged_dir(&self) -> PathBuf { self.tools.current_workspace().join(".angel/dungeon/reforged") }

    pub(crate) fn brewed_dir(&mut self) -> PathBuf {
        self.with_realm(|r, cx| r.session(cx).brewed_dir())
    }

    pub(crate) fn request_reforge(&mut self, player: u32, part: Part, words: String) {
        self.with_realm(|r, cx| r.session(cx).request_reforge(player, part, words));
    }

    pub(crate) fn deliver_wish_lesson(&mut self, lesson: &mut Lesson) -> bool {
        self.with_realm(|r, cx| r.session(cx).deliver_wish_lesson(lesson))
    }

    pub(crate) fn offer_from_scroll(&mut self, player: u32, words: &str, known: Option<&str>) {
        self.with_realm(|r, cx| r.session(cx).offer_from_scroll(player, words, known));
    }

    pub(crate) fn read_forged(&mut self, path: &Path) -> Result<String, String> {
        self.with_realm(|r, cx| r.session(cx).read_forged(path))
    }

    pub(crate) fn start_shooter(&mut self, guest: Option<&str>) {
        self.with_realm(|r, cx| r.session(cx).start_shooter(guest));
    }

    pub(crate) fn start_walk(&mut self) {
        self.with_realm(|r, cx| r.session(cx).start_walk());
    }

    pub(crate) fn expand_dungeon(&mut self) {
        self.with_realm(|r, cx| r.session(cx).expand_dungeon());
    }

    pub(crate) fn delve_map_hit(&mut self, x: u16, y: u16) -> bool {
        self.with_realm(|r, cx| r.session(cx).delve_map_hit(x, y))
    }

    pub(crate) fn dungeon_keyboard_active(&self) -> bool {
        self.realm.view().dungeon_keyboard_active(&self.realm_facts())
    }

    pub(crate) fn host_name(&self) -> String {
        angel_realm_host::control::host_name()
    }

    pub(crate) fn economy(&mut self) -> &mut angel_realm_host::together_realm::Realm {
        self.realm.economy()
    }
}

// The tests of the game's controls that need the cockpit (its `App`, its
// keys, its screens). They read the names the controls' modules had in scope
// before the cut.
#[allow(unused_imports)]
use angel_delve::rules::host_commands::{Command as GameCommand, Reply as GameReply};
#[allow(unused_imports)]
use angel_realm_host::control::{Hosting, MAX_FRIENDS, MAX_WATCHED_FRIENDS};
#[allow(unused_imports)]
use angel_realm_host::together_shooter as shooter;
#[allow(unused_imports)]
use std::time::Duration;

#[path = "../../../../tests/cockpit/app/dungeon_controls__tests.rs"]
mod controls_tests;
#[path = "../../../../tests/cockpit/app/dungeon_brew__tests.rs"]
mod brew_tests;
#[path = "../../../../tests/cockpit/app/dungeon_chivalry__tests.rs"]
mod chivalry_tests;
#[path = "../../../../tests/cockpit/app/dungeon_paint__tests.rs"]
mod paint_tests;
#[path = "../../../../tests/cockpit/app/dungeon_reforge_jobs__tests.rs"]
mod reforge_jobs_tests;
#[path = "../../../../tests/cockpit/app/dungeon_wish_jobs__tests.rs"]
mod wish_jobs_tests;
#[path = "../../../../tests/cockpit/app/dungeon_grant_flight__tests.rs"]
mod grant_flight_tests;
#[path = "../../../../tests/cockpit/app/overworld_walk__tests.rs"]
mod walk_tests;

#[allow(unused_imports)]
use angel_delve::rules::{cards, cauldron};
#[allow(unused_imports)]
use angel_realm_host::control::dungeon_brew::brew_brief;
#[allow(unused_imports)]
use angel_realm_host::control::dungeon_reforge::{SPELL_LINES, SUMMONS, INKS};
#[allow(unused_imports)]
use angel_realm_host::control::Pick;
#[allow(unused_imports)]
use angel_delve::rules::wish_jobs::Outcome;
#[allow(unused_imports)]
use angel_realm_host::together_painter as painter;
#[allow(unused_imports)]
use crate::app::WorldButton;

#[path = "../../../../tests/cockpit/app/dungeon_mirror_controls__tests.rs"]
mod mirror_tests;
#[path = "../../../../tests/cockpit/app/dungeon_speculative__tests.rs"]
mod speculative_tests;
#[path = "../../../../tests/cockpit/app/dungeon_squire_settlement__tests.rs"]
mod squire_settlement_tests;

#[allow(unused_imports)]
use angel_delve::rules::Input as ShooterInput;
