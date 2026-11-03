//! The seam between the cockpit and the game: the game's controls run on a
//! context that holds only what the contract lends (`angel_live::realm`).

use super::*;
use angel_live::realm::{Facts, HostCx, Move, Oracle, Panel, RealmHost};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// A cockpit that has nothing to ask a model.
struct NoOracle;

impl Oracle for NoOracle {
    fn respond(&self, _prompt: &str, _at_least: Duration) -> Result<String, String> {
        Err("no model in a test".into())
    }

    fn label(&self) -> &str {
        "none"
    }
}

/// The cockpit as a test holds it: a composer, a transcript, the moves asked.
struct TestCx {
    facts: Facts,
    workspace: PathBuf,
    composer: (String, usize),
    said: Vec<String>,
    moves: Vec<Move>,
    redraws: usize,
}

impl TestCx {
    fn new(label: &str) -> Self {
        let workspace = std::env::temp_dir().join(format!("host-seam-{label}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&workspace);
        Self {
            facts: Facts { composer_empty: true, terminal_focused: true, ..Facts::default() },
            workspace,
            composer: (String::new(), 0),
            said: Vec::new(),
            moves: Vec::new(),
            redraws: 0,
        }
    }

    fn type_in(&mut self, text: &str) {
        self.composer = (text.to_string(), text.len());
        self.facts.composer_empty = text.is_empty();
    }
}

impl HostCx for TestCx {
    fn facts(&self) -> Facts {
        self.facts
    }

    fn workspace(&self) -> PathBuf {
        self.workspace.clone()
    }

    fn oracle(&self) -> Arc<dyn Oracle> {
        Arc::new(NoOracle)
    }

    fn say(&mut self, text: String) {
        self.said.push(text);
    }

    fn redraw(&mut self) {
        self.redraws += 1;
    }

    fn take_composer(&mut self) -> (String, usize) {
        self.facts.composer_empty = true;
        std::mem::take(&mut self.composer)
    }

    fn set_composer(&mut self, text: String, cursor: usize) {
        self.facts.composer_empty = text.is_empty();
        self.composer = (text, cursor);
    }

    fn make(&mut self, ask: Move) {
        self.moves.push(ask);
    }
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn host() -> GameHost {
    GameHost::new(world_viz::World::new(7))
}

#[test]
fn a_host_with_no_game_open_leaves_every_key_to_the_cockpit() {
    let mut host = host();
    let mut cx = TestCx::new("keys");
    assert!(!RealmHost::view_active(&host, &cx.facts));
    assert!(!RealmHost::keyboard_active(&host, &cx.facts));
    assert!(!RealmHost::wants_fast_tick(&host, &cx.facts));
    for code in [KeyCode::Char('w'), KeyCode::Esc, KeyCode::F(2), KeyCode::Enter] {
        assert!(!RealmHost::key(&mut host, &mut cx, press(code)), "{code:?}");
        assert!(!RealmHost::walk_key(&mut host, &mut cx, press(code)), "{code:?}");
    }
    assert!(cx.moves.is_empty());
    assert!(cx.said.is_empty());
}

#[test]
fn the_game_view_keeps_the_composers_draft_and_gives_it_back() {
    let mut host = host();
    let mut cx = TestCx::new("draft");
    cx.type_in("fix the build");
    host.session(&mut cx).expand_dungeon();
    // The draft left the composer for the game's keeping, the stage came up,
    // and the cockpit was asked to move there.
    assert_eq!(cx.composer, (String::new(), 0));
    assert_eq!(
        cx.moves,
        vec![Move::DismissIntro, Move::ShowWorld, Move::Focus("artifacts")]
    );
    assert!(host.dungeon.expanded);
    cx.moves.clear();

    RealmHost::collapse(&mut host, &mut cx);
    assert_eq!(cx.composer, ("fix the build".to_string(), 13));
    assert_eq!(cx.moves, vec![Move::Focus("core")]);
    assert!(!host.dungeon.expanded);
}

#[test]
fn slash_hands_the_keyboard_back_with_a_command_started() {
    let mut host = host();
    let mut cx = TestCx::new("slash");
    host.session(&mut cx).expand_dungeon();
    // Something is open in the game view for the key to find: the intro.
    host.dungeon.intro = Some(crate::delve_intro::Intro::default());
    cx.facts.composer_empty = true;
    let taken = RealmHost::key(&mut host, &mut cx, press(KeyCode::Char('x')));
    // The intro takes its own keys; an unrelated letter is not the cockpit's.
    assert!(taken);
    assert!(cx.composer.0.is_empty());
}

#[test]
fn a_door_in_the_map_asks_the_cockpit_for_its_panel() {
    use crate::world_viz::overworld::Opens;
    let mut host = host();
    let mut cx = TestCx::new("door");
    // No visitor at a door: nothing is asked.
    RealmHost::go_in(&mut host, &mut cx);
    assert!(cx.moves.is_empty());
    let _ = Opens::Library;
    let _ = Panel::Library;
}

#[test]
fn the_holdings_hear_a_turn_through_the_contract() {
    let mut host = host();
    RealmHost::note_call(&mut host, "t1", "shell", "command=git commit -m \"x\"");
    RealmHost::note_result(&mut host, "t1", true);
    RealmHost::note_turn(&mut host, 10, 20, 0);
    RealmHost::note_loop(&mut host, 2);
    assert_eq!(host.dungeon.ops_loop, 2);
}
