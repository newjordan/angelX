use super::*;
use crate::tests::{TestEnvGuard, env_lock};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};

type Term = Terminal<TestBackend>;
type Scene = fn(&mut Term);

const SIZES: [(u16, u16); 3] = [(100, 30), (144, 44), (200, 60)];

const SCENES: &[(&str, Scene)] = &[
    ("idle", idle),
    ("tools", tools),
    ("walk", walk),
    ("delve_menu", delve_menu),
    ("delve_run", delve_run),
    ("delve_run_minimap", delve_run_minimap),
    ("delve_run_ticks", delve_run_ticks),
    ("delve_joined", delve_joined),
    ("loop_card", loop_card),
    ("loop_quintain", loop_quintain),
    ("loop_crawl", loop_crawl),
    ("stage_observatory", stage_observatory),
    ("stage_vault", stage_vault),
    ("stage_smithy", stage_smithy),
];

fn scratch(leaf: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("angelx-golden-{leaf}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn bind(app: &mut App, workspace: &Path) {
    app.tools = std::sync::Arc::new(crate::agent::harness::ToolRegistry::with_team(
        workspace.to_path_buf(),
        Vec::new(),
    ));
    app.realm.world = crate::delve::world_viz::World::for_workspace(workspace);
    app.realm.dungeon.realm = None;
    app.realm.dungeon.save_path = Some(workspace.join(".angel/dungeon/session.json"));
}

fn bound(mut app: App, leaf: &str) -> App {
    let workspace = scratch(leaf);
    bind(&mut app, &workspace);
    app
}

fn preview() -> App {
    bound(seed_preview_app(), "workspace")
}

fn delver() -> App {
    bound(
        App::preview(crate::ui::viewer::Viewer::static_preview()),
        "workspace",
    )
}

fn paint(term: &mut Term, app: &mut App) {
    term.draw(|frame| ui(frame, app)).unwrap();
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn artifacts_on(app: &mut App, route: crate::ui::scryglass::StageRoute) {
    app.focus_module("artifacts");
    app.scryglass.navigate(route);
}

fn idle(term: &mut Term) {
    let mut app = preview();
    paint(term, &mut app);
}

fn tools(term: &mut Term) {
    use crate::ui::transcript::{Message, Role};
    let mut app = preview();
    let done = crate::agent::harness::ToolEventId("golden-shell".into());
    app.realm
        .world
        .note_tool_call_event(done.clone(), "shell", "cargo test --no-default-features");
    app.realm.world.note_tool_result_event(
        &done,
        "shell",
        "verify · 412 passed, tree unchanged",
        crate::agent::harness::ToolOutcome {
            execution: crate::agent::harness::ExecutionOutcome::Succeeded,
            verification: crate::agent::harness::VerificationOutcome::Passed,
        },
    );
    let editing = crate::agent::harness::ToolEventId("golden-edit".into());
    app.realm
        .world
        .note_tool_call_event(editing, "edit", "hashline · cockpit.rs");
    app.messages.push(Message::new(
        Role::User,
        "plate the site with captures out of the real renderers",
    ));
    app.messages.push(Message::new(
        Role::Angel,
        "Five plates from the compositor, the chart renderer and the ride camera.",
    ));
    app.messages.push(Message::new(
        Role::Activity,
        "shell · cargo test --no-default-features → 412 passed, tree unchanged",
    ));
    app.messages.push(Message::new(
        Role::Activity,
        "edit · hashline · cockpit.rs → running",
    ));
    app.settle_transcript_spawns();
    paint(term, &mut app);
}

fn walk(term: &mut Term) {
    let mut app = preview();
    paint(term, &mut app);
    app.walk_from_command();
    paint(term, &mut app);
}

fn delve_menu(term: &mut Term) {
    let mut app = delver();
    paint(term, &mut app);
    app.open_intro();
    paint(term, &mut app);
}

fn started() -> App {
    let mut app = delver();
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app
}

fn delve_run(term: &mut Term) {
    let mut app = started();
    paint(term, &mut app);
}

fn delve_run_minimap(term: &mut Term) {
    let mut app = started();
    paint(term, &mut app);
    press(&mut app, KeyCode::Esc);
    paint(term, &mut app);
}

fn delve_run_ticks(term: &mut Term) {
    let mut app = started();
    paint(term, &mut app);
    let now = app
        .realm
        .dungeon
        .shooter
        .clock()
        .unwrap_or_else(std::time::Instant::now);
    app.advance_shooter_at(now);
    for tick in 1..=90u32 {
        app.advance_shooter_at(now + std::time::Duration::from_nanos(33_333_333) * tick);
    }
    paint(term, &mut app);
}

fn delve_joined(term: &mut Term) {
    let _public = TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = delver();
    host.terminal_focused = true;
    let message = host.dungeon_command(Some("host --1"));
    let link = message
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http://"))
        .unwrap()
        .to_string();
    let mut friend = bound(
        App::preview(crate::ui::viewer::Viewer::static_preview()),
        "friend",
    );
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {link}")));
    let base = host
        .realm
        .dungeon
        .shooter
        .clock()
        .unwrap_or_else(std::time::Instant::now);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    let mut tick = 0u32;
    loop {
        tick += 1;
        host.advance_shooter_at(base + std::time::Duration::from_nanos(33_333_333) * tick);
        friend.advance_joined();
        if friend.joined_knight().is_some() {
            paint(term, &mut friend);
            if test_backend_text(term.backend()).contains("the doors stand open") {
                break;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the friend never saw the doors stand open"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn loop_card(term: &mut Term) {
    let mut app = preview();
    app.loop_dialog = Some(crate::drive::loop_dialog::LoopLaunchDialog::new(
        "chart plate",
        0,
        false,
    ));
    paint(term, &mut app);
}

fn loop_running(app: &mut App) {
    use crate::drive::loop_ctl::LoopStatus;
    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.iteration = 1;
    app.realm
        .world
        .note_adventure(crate::delve::world_viz::AdventureEvent::LoopStarted {
            kind: crate::delve::world_viz::LoopKind::Coding,
            task: "an actual coding loop".to_string(),
        });
    app.realm
        .world
        .note_adventure(crate::delve::world_viz::AdventureEvent::Iteration { n: 1 });
}

fn loop_quintain(term: &mut Term) {
    let mut app = preview();
    artifacts_on(&mut app, crate::ui::scryglass::StageRoute::Loop);
    loop_running(&mut app);
    paint(term, &mut app);
}

fn loop_crawl(term: &mut Term) {
    let mut app = preview();
    artifacts_on(&mut app, crate::ui::scryglass::StageRoute::Realm);
    loop_running(&mut app);
    paint(term, &mut app);
}

fn stage_route(term: &mut Term, route: crate::ui::scryglass::StageRoute) {
    let mut app = preview();
    artifacts_on(&mut app, route);
    paint(term, &mut app);
}

fn stage_observatory(term: &mut Term) {
    stage_route(term, crate::ui::scryglass::StageRoute::Observatory);
}

fn stage_vault(term: &mut Term) {
    stage_route(term, crate::ui::scryglass::StageRoute::Vault);
}

fn stage_smithy(term: &mut Term) {
    stage_route(
        term,
        crate::ui::scryglass::StageRoute::Explore(crate::delve::world_viz::Building::Smithy.into()),
    );
}

type Style = [String; 3];

#[derive(PartialEq)]
struct Shot {
    width: usize,
    height: usize,
    cells: Vec<(String, Style)>,
}

fn mask_ports(cells: &mut [(String, Style)], width: usize) {
    let lead: Vec<String> = "127.0.0.1:".chars().map(String::from).collect();
    for row in cells.chunks_mut(width) {
        let mut at = 0;
        while at + lead.len() < row.len() {
            if (0..lead.len()).all(|i| row[at + i].0 == lead[i]) {
                let mut digit = at + lead.len();
                while digit < row.len()
                    && row[digit].0.len() == 1
                    && row[digit].0.as_bytes()[0].is_ascii_digit()
                {
                    row[digit].0 = "#".into();
                    digit += 1;
                }
                at = digit;
            } else {
                at += 1;
            }
        }
    }
}

fn shot(term: &Term) -> Shot {
    let buffer = term.backend().buffer();
    let width = buffer.area().width as usize;
    let height = buffer.area().height as usize;
    let mut cells: Vec<(String, Style)> = buffer
        .content()
        .iter()
        .map(|cell| {
            (
                cell.symbol().to_string(),
                [
                    format!("{:?}", cell.fg),
                    format!("{:?}", cell.bg),
                    format!("{:?}", cell.modifier),
                ],
            )
        })
        .collect();
    mask_ports(&mut cells, width);
    Shot {
        width,
        height,
        cells,
    }
}

impl Shot {
    fn text(&self) -> String {
        let mut out = String::new();
        for row in self.cells.chunks(self.width) {
            let visible = row
                .iter()
                .rposition(|cell| cell.0 != " ")
                .map(|idx| idx + 1)
                .unwrap_or(0);
            for cell in &row[..visible] {
                out.push_str(&cell.0);
            }
            out.push('\n');
        }
        out
    }

    fn sheet(&self) -> Vec<u8> {
        let mut palette: Vec<Style> = Vec::new();
        let mut cells: Vec<(&str, usize)> = Vec::new();
        for (symbol, style) in &self.cells {
            let index = palette.iter().position(|s| s == style).unwrap_or_else(|| {
                palette.push(style.clone());
                palette.len() - 1
            });
            cells.push((symbol, index));
        }
        let value = serde_json::json!({
            "width": self.width,
            "height": self.height,
            "palette": palette,
            "cells": cells,
        });
        serde_json::to_vec(&value).unwrap()
    }

    fn differences(&self, other: &Shot) -> Vec<String> {
        self.cells
            .iter()
            .zip(&other.cells)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(5)
            .map(|(at, (a, b))| {
                format!(
                    "cell ({},{}) first {:?} second {:?}",
                    at % self.width,
                    at / self.width,
                    a,
                    b
                )
            })
            .collect()
    }
}

fn render(scene: Scene, width: u16, height: u16) -> Shot {
    scratch("home");
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    scene(&mut term);
    shot(&term)
}

#[test]
fn golden_frames() {
    let Some(dir) = std::env::var_os("ANGELX_GOLDEN_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let _lock = env_lock();
    let _env = [
        TestEnvGuard::set("HOME", scratch("home").to_str().unwrap()),
        TestEnvGuard::unset("ANGEL_COMP_MODE"),
        TestEnvGuard::unset("ANGEL_TURBO"),
        TestEnvGuard::unset("ANGEL_BACKDROP"),
        TestEnvGuard::unset("ANGEL_LOOP_VIEW"),
        TestEnvGuard::unset("ANGEL_WORLD_MAP"),
        TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks"),
    ];
    crate::drive::comp_mode::invalidate_cache();
    crate::ui::surfaces::invalidate_backdrop_cache();
    let mut unstable = Vec::new();
    for (name, scene) in SCENES {
        for (width, height) in SIZES {
            let first = render(*scene, width, height);
            let second = render(*scene, width, height);
            let label = format!("{name}_{width}x{height}");
            if first != second {
                eprintln!("{label} {:?}", first.differences(&second));
                unstable.push(label.clone());
            }
            std::fs::write(dir.join(format!("{label}.txt")), first.text()).unwrap();
            std::fs::write(dir.join(format!("{label}.json")), first.sheet()).unwrap();
        }
    }
    assert!(
        unstable.is_empty(),
        "frames that differ between two renders: {unstable:?}"
    );
}
