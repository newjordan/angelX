//! The body-maker: a wished look drawn whole, queued next to the worn
//! painter, kept in the looks folder and composed for the window.

use super::*;
use crate::control::DungeonView;
use crate::control::dungeon_reforge::BodyAsk;
use angel_delve::render::overworld::arena::hd_pack::{HdArt, Pack};
use angel_delve::render::overworld::arena::{NATIVE_H, NATIVE_W, scene_for, testing};
use angel_delve::rules::cosmetic_wish::key;
use angel_delve::rules::host_commands::Command as Game;
use angel_delve::rules::{Run, looks};
use std::time::{Duration, Instant};

const WORDS: &str = "a frog knight with a lily-pad shield and a reed spear";

/// A fresh workspace.
fn workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("wish-looker-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A picture of the frog: 64 by 80, one green.
fn frog(root: &Path) -> PathBuf {
    let path = root.join("fixture-frog.png");
    let rgba: Vec<u8> = (0..64 * 80).flat_map(|_| [40u8, 160, 60, 255]).collect();
    image::save_buffer(&path, &rgba, 64, 80, image::ColorType::Rgba8).unwrap();
    path
}

/// A body-maker that runs `body` in the shell.
fn script(root: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = root.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// A fake body-maker: what the real one leaves after its 15 minutes, for the seat that asked
/// (read from stdin as the real one reads it): the idle clip of the frog, both hands, in
/// `pack/`, keyed to that seat's design and colours.
fn fake_maker(root: &Path, picture: &Path) -> PathBuf {
    script(
        root,
        "wish-char-fake",
        &format!(
            r#"input=$(cat)
seat=$(echo "$input" | sed -n 's/.*"seat":\([0-9]*\).*/\1/p')
design=$(echo "$input" | sed -n 's/.*"design":"\([a-z-]*\)".*/\1/p')
mkdir -p pack
cp "{}" pack/frog.png
first=1
{{
  printf '{{"scale":4,"art":[\n'
  for k in 0 1 2 3 4 5; do for side in r l; do
    if [ $first = 1 ]; then first=0; else printf ',\n'; fi
    printf '{{"key":"knight/%s/idle/%s/seat%s/%s","rung":64,"pivot":[32,79],"file":"frog.png"}}' "$design" "$k" "$seat" "$side"
  done; done
  printf '\n]}}\n'
}} > pack/pack.json
echo "drawn for seat $seat" "$design"
"#,
            picture.display()
        ),
    )
}

/// Tick until the body-makers have ended: every job that finished.
fn settle(lookers: &mut Lookers, ws: &Path, bodies: &[(String, u32)]) -> Vec<Finished> {
    let mut done = Vec::new();
    let start = Instant::now();
    while lookers.running() + lookers.queued() > 0 && start.elapsed() < Duration::from_secs(20) {
        done.extend(lookers.tick(ws, bodies).finished);
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(50));
    done.extend(lookers.tick(ws, bodies).finished);
    done
}

fn keys_of(pack_json: &Path) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack_json).unwrap()).unwrap();
    value["art"].as_array().unwrap().iter().map(|e| e["key"].as_str().unwrap().to_string()).collect()
}

/// A boss room with a second seat, and seat 2's mirror wish said and chosen (the ask is the host's to take off the queue).
fn wished_room() -> Run {
    let mut run = testing::boss("cinderjaw");
    run.join_seat(2, "Goose Inspector");
    run.begin_mirror_wish(2).unwrap();
    run.offer_wish(2, WORDS, None).unwrap();
    run.choose(2, 0).unwrap();
    run
}

fn view(ws: &Path, run: Run) -> DungeonView {
    let mut view = DungeonView::default();
    view.restore(ws).unwrap();
    view.shooter.set(Some(run));
    view
}

/// What the host does with the mirror's ask: the card, the ask off the queue, the body-maker started.
fn ask(view: &mut DungeonView, ws: &Path, command: Option<PathBuf>) -> (BodyAsk, angel_delve::rules::cards::Card) {
    let run = view.shooter.as_ref().unwrap();
    let asked = run.asked[0].clone();
    assert!(asked.cosmetic && asked.knight == 2);
    let card = run.mirror_paint_card_for(&asked.cosmetic_name, &asked.words).unwrap();
    view.shooter.command(Game::DropMirrorLook { player: asked.knight, words: asked.words.clone() }).unwrap();
    (view.start_body_look(command, ws, &card, &asked), card)
}

/// Tick the host until its body-makers are done.
fn watch(view: &mut DungeonView, ws: &Path) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        view.watch_looks(ws);
        if view.lookers.running() + view.lookers.queued() == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(50));
    view.watch_looks(ws);
}

/// Every named figure seat `n` is in a room, asked of a pack as a window asks.
fn drawn_by(run: &Run, pack: &Pack, n: u32) -> Vec<(String, Option<(u32, u32)>)> {
    let scene = scene_for(run, NATIVE_W, NATIVE_H, Some(1));
    let tag = format!("/seat{n}/");
    scene
        .figures
        .iter()
        .flat_map(|f| f.bodies.iter())
        .filter(|b| b.named.as_ref().is_some_and(|named| named.key.contains(&tag)))
        .map(|b| {
            let mut sent = Vec::new();
            let placed = HdArt::default().picture_from(pack, b, &mut sent);
            (b.named.as_ref().unwrap().key.to_string(), placed.map(|_| (sent[0].width, sent[0].height)))
        })
        .collect()
}

#[test]
fn seat_twos_job_lands_composes_its_keys_and_only_seat_twos_frames_come_from_them() {
    let ws = workspace("land");
    let maker = fake_maker(&ws, &frog(&ws));
    let mut view = view(&ws, wished_room());
    let before = view.shooter.as_ref().unwrap().home.wardrobe.clone();
    let (queued, card) = ask(&mut view, &ws, Some(maker));
    assert_eq!(queued, BodyAsk::Queued);
    assert!(view.shooter.as_ref().unwrap().asked.is_empty(), "the ask is off the queue before any job: nothing spent, nothing to refund");
    assert!(!view.shooter.as_ref().unwrap().has_mirror_body(2), "the knight is not changed until the job lands");

    // The job's own folder is the card's, and says who asked.
    let job_dir = ws.join(".angel/dungeon/paint-jobs").join(&card.id).join("look");
    let input: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(job_dir.join("job.json")).unwrap()).unwrap();
    assert_eq!(
        (input["seat"].as_u64(), input["design"].as_str(), input["colours"].as_str(), input["rung"].as_u64(), input["wish"].as_str()),
        (Some(2), Some("sugarloaf-crusader"), Some("seat2"), Some(64), Some(WORDS))
    );
    assert_eq!(input["card"], card.id.as_str());

    watch(&mut view, &ws);
    let run = view.shooter.as_ref().unwrap();
    assert!(run.has_mirror_body(2) && !run.has_mirror_body(1), "seat 2's knight is the look, seat 1's is not");
    assert!(view.notice.contains("finished your look") && view.notice.contains("no wish spent"), "{}", view.notice);
    assert_ne!(run.home.wardrobe, before, "the look keeps the body in the wardrobe");
    let log = std::fs::read_to_string(job_dir.join("char.log")).unwrap();
    assert!(log.contains("drawn for seat 2 sugarloaf-crusader"), "its talk is in its folder: {log}");
    let looks = looks_dir(&ws);
    assert!(looks.join(&card.id).join("pack.json").is_file() && looks.join(&card.id).join("frog.png").is_file());

    // The next look composes the window's pack from the roster: seat 2's design and colours, seat 2 only.
    view.watch_looks(&ws);
    let keys = keys_of(&looks.join("pack.json"));
    assert_eq!(keys.len(), 12);
    assert!(keys.iter().all(|k| k.starts_with("knight/sugarloaf-crusader/idle/") && k.contains("/seat2/")), "{keys:?}");
    let pack = Pack::open(&looks).expect("the composed pack opens");
    assert!(pack.warnings().is_empty(), "{:?}", pack.warnings());
    assert_eq!(pack.named_len(), 12);

    // Frames: seat 2's come from the new entries (64 by 80, on the 64 rung whatever the floor); seat 1's do not.
    let run = view.shooter.as_ref().unwrap();
    let mine = drawn_by(run, &pack, 2);
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0].1, Some((64, 80)), "{mine:?}");
    let theirs = drawn_by(run, &pack, 1);
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0].1, None, "seat 1 keeps its own: {theirs:?}");
    // Without the pack, the same figure is drawn by nothing.
    assert!(drawn_by(run, &Pack::of(4, []), 2).iter().all(|(_, drawn)| drawn.is_none()));
}

#[test]
fn the_composed_pack_follows_the_knight_to_its_seat_and_nothing_is_rewritten_for_nothing() {
    let ws = workspace("seats");
    let maker = fake_maker(&ws, &frog(&ws));
    let mut view = view(&ws, wished_room());
    let (_, card) = ask(&mut view, &ws, Some(maker));
    watch(&mut view, &ws);
    let looks = looks_dir(&ws);
    // Composed here for any seat the knight sits in: the four designs have the same clips and frames.
    assert_eq!(compose(&looks, &[(card.id.clone(), 1)]).unwrap(), Composed { changed: true, missing: vec![] });
    assert!(keys_of(&looks.join("pack.json")).iter().all(|k| k.starts_with("knight/great-helm/idle/") && k.contains("/seat1/")));
    assert_eq!(compose(&looks, &[(card.id.clone(), 1)]).unwrap(), Composed { changed: false, missing: vec![] }, "the same roster rewrites nothing");
    let both = [(card.id.clone(), 1), (card.id.clone(), 2)];
    assert!(compose(&looks, &both).unwrap().changed);
    let keys = keys_of(&looks.join("pack.json"));
    assert_eq!(keys.len(), 24);
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "sorted: the file only differs when the looks do");
    // A knight with no set (deleted) is said, not composed.
    let strange = [("mirror-look-0000000000000001".to_string(), 3)];
    assert_eq!(compose(&looks, &strange).unwrap(), Composed { changed: true, missing: vec![strange[0].0.clone()] }, "the file goes: nobody is drawn whole");
    assert!(!looks.join("pack.json").exists());
    assert_eq!(compose(&looks, &[]).unwrap(), Composed::default());
}

#[test]
fn a_failed_job_changes_nothing_and_the_worn_ink_stays() {
    let table: [(&str, &str, &str); 6] = [
        ("exits-one", "echo trouble; exit 1", "ended with"),
        ("no-pack", "exit 0", "no pack/pack.json"),
        ("not-json", "mkdir pack; echo nope > pack/pack.json", "not JSON"),
        ("empty", "mkdir pack; echo '{\"art\":[]}' > pack/pack.json", "no drawings"),
        (
            "wrong-seat",
            "mkdir pack; echo '{\"art\":[{\"key\":\"knight/great-helm/idle/0/seat1/r\",\"rung\":64,\"file\":\"x.png\"}]}' > pack/pack.json",
            "the seat that asked",
        ),
        (
            "missing-file",
            "mkdir pack; echo '{\"art\":[{\"key\":\"knight/sugarloaf-crusader/idle/0/seat2/r\",\"rung\":64,\"file\":\"x.png\"}]}' > pack/pack.json",
            "would leave out",
        ),
    ];
    for (name, body, said) in table {
        let ws = workspace(name);
        let maker = script(&ws, "wish-char-broken", body);
        let mut run = wished_room();
        // The worn painter has answered: its ink is on seat 2.
        let mut card = run.mirror_paint_card(2, WORDS).unwrap();
        card.looks.worn = Some(looks::Sprite { frames: vec![vec!["7".into()]], ..Default::default() });
        assert!(run.update_mirror_paint(&card));
        let mut view = view(&ws, run);
        let wardrobe = view.shooter.as_ref().unwrap().home.wardrobe.clone();
        let (queued, card) = ask(&mut view, &ws, Some(maker));
        assert_eq!(queued, BodyAsk::Queued, "{name}");
        watch(&mut view, &ws);
        view.watch_looks(&ws);
        let run = view.shooter.as_ref().unwrap();
        assert!(view.notice.contains("could not finish your look") && view.notice.contains(said), "{name}: {}", view.notice);
        assert!(view.notice.contains("no wish spent"), "{name}: {}", view.notice);
        assert!(!run.has_mirror_body(2), "{name}");
        assert_eq!(run.home.wardrobe, wardrobe, "{name}: the run is as it was");
        assert_eq!(run.players[&2].worn(&run.book).len(), 1, "{name}: today's overlay still shows");
        assert!(!looks_dir(&ws).join(&card.id).exists(), "{name}: nothing kept");
        assert!(!looks_dir(&ws).join("pack.json").exists(), "{name}: nothing composed");
        assert!(run.asked.is_empty(), "{name}: nothing queued to spend or refund");
    }
}

#[test]
fn no_command_means_no_job_and_the_old_notices() {
    let ws = workspace("none");
    // Nothing set: no env word, no executable in the home.
    let home = ws.join("home");
    std::fs::create_dir_all(home.join(".angelX")).unwrap();
    assert_eq!(command_in(None, Some(home.clone())), None);
    assert_eq!(command_in(Some("".into()), Some(home.clone())), None, "an empty word is no word");
    assert_eq!(command_in(None, None), None);
    // A file that is not executable is not a body-maker.
    std::fs::write(home.join(".angelX/wish-char"), "#!/bin/sh\n").unwrap();
    assert_eq!(command_in(None, Some(home.clone())), None);
    let own = script(&home.join(".angelX"), "wish-char", "exit 0");
    assert_eq!(command_in(None, Some(home.clone())), Some(own));
    assert_eq!(command_in(Some("/opt/maker".into()), Some(home)), Some(PathBuf::from("/opt/maker")));

    let mut view = view(&ws, wished_room());
    let (queued, card) = ask(&mut view, &ws, None);
    assert_eq!(queued, BodyAsk::Off, "no command, no job");
    assert_eq!((view.lookers.running(), view.lookers.queued()), (0, 0));
    assert!(!ws.join(".angel/dungeon/paint-jobs").join(&card.id).exists(), "no folder was made for it");
    let notice = view.notice.clone();
    view.watch_looks(&ws);
    assert_eq!(view.notice, notice);
    assert!(!looks_dir(&ws).join("pack.json").exists());
    // A command that cannot start is logged, and the game goes on.
    let mut view = self::view(&ws, wished_room());
    let (queued, _) = ask(&mut view, &ws, Some(ws.join("not-there")));
    assert_eq!(queued, BodyAsk::Queued, "queued; it failed to start");
    assert_eq!((view.lookers.running(), view.lookers.queued()), (0, 0));
    view.watch_looks(&ws);
    assert!(!view.shooter.as_ref().unwrap().has_mirror_body(2));
    assert!(view.notice.contains("could not finish your look") && view.notice.contains("did not start") && view.notice.contains("the worn ink stays"), "{}", view.notice);
}

#[test]
fn a_newer_ask_for_a_card_replaces_its_older_job_and_one_job_runs_at_a_time() {
    let ws = workspace("queue");
    let maker = fake_maker(&ws, &frog(&ws));
    let slow = script(&ws, "slow", "echo started >> slow-started; sleep 30");
    let mut lookers = Lookers::default();
    let (a, b) = ("mirror-look-aaaaaaaaaaaaaaaa", "mirror-look-bbbbbbbbbbbbbbbb");
    lookers.start(slow, job(&ws, a, "Goose", "old words", 2));
    lookers.start(maker.clone(), job(&ws, b, "Matt", "another knight", 1));
    assert_eq!((lookers.running(), lookers.queued()), (1, 1), "one at a time: the pipeline is a hog");
    // The same card asked again: the older job is let go (killed), the newer waits its turn behind b.
    lookers.start(maker.clone(), job(&ws, a, "Goose", "new words", 2));
    assert_eq!((lookers.running(), lookers.queued()), (1, 1), "a's slow job was killed; b runs, a waits");
    let done = settle(&mut lookers, &ws, &[]);
    let mut cards: Vec<_> = done.iter().map(|f| (f.job.card.clone(), f.job.words.clone(), f.outcome.is_ok())).collect();
    cards.sort();
    assert_eq!(cards, vec![(a.into(), "new words".into(), true), (b.into(), "another knight".into(), true)]);
    assert!(looks_dir(&ws).join(a).join("pack.json").is_file() && looks_dir(&ws).join(b).join("pack.json").is_file());
    // Seat 1's job was keyed to seat 1.
    assert!(keys_of(&looks_dir(&ws).join(b).join("pack.json")).iter().all(|k| k.starts_with("knight/great-helm/") && k.contains("/seat1/")));
    // An older run's folder is never this run's: a maker that leaves nothing fails, whatever was there.
    let none = script(&ws, "none", "exit 0");
    lookers.start(none, job(&ws, a, "Goose", "new words", 2));
    let done = settle(&mut lookers, &ws, &[]);
    assert!(done[0].outcome.as_ref().is_err_and(|e| e.contains("no pack/pack.json")), "{:?}", done[0].outcome);
}

#[test]
fn a_restart_keeps_the_look_and_composes_the_same_pack() {
    let ws = workspace("restart");
    let maker = fake_maker(&ws, &frog(&ws));
    let mut first = view(&ws, wished_room());
    let (_, card) = ask(&mut first, &ws, Some(maker));
    watch(&mut first, &ws);
    first.watch_looks(&ws);
    let looks = looks_dir(&ws);
    let composed = std::fs::read(looks.join("pack.json")).unwrap();
    first.checkpoint(Instant::now(), true).unwrap();
    drop(first);

    // The window's file is gone (a cleaned folder) or still there: the same pack either way, from the save and the set alone.
    for wipe in [false, true] {
        if wipe {
            std::fs::remove_file(looks.join("pack.json")).unwrap();
        }
        let mut next = DungeonView::default();
        next.restore(&ws).unwrap();
        let run = next.shooter.as_ref().unwrap();
        assert_eq!(run.mirror_bodies(), vec![(key(&run.players[&2].name), 2)]);
        assert_eq!(key(&run.players[&2].name), card.id);
        assert!(run.has_mirror_body(2) && !run.has_mirror_body(1));
        next.watch_looks(&ws);
        assert_eq!(std::fs::read(looks.join("pack.json")).unwrap(), composed, "wipe {wipe}");
        let pack = Pack::open(&looks).unwrap();
        let run = next.shooter.as_ref().unwrap();
        assert_eq!(drawn_by(run, &pack, 2)[0].1, Some((64, 80)));
        assert_eq!(drawn_by(run, &pack, 1)[0].1, None);
    }

    // A set taken off the disk: the run puts the worn ink back on the knight.
    std::fs::remove_dir_all(looks.join(&card.id)).unwrap();
    let mut lost = DungeonView::default();
    lost.restore(&ws).unwrap();
    lost.watch_looks(&ws);
    assert!(!lost.shooter.as_ref().unwrap().has_mirror_body(2), "no set, no body");
    lost.watch_looks(&ws);
    assert!(!looks.join("pack.json").exists());
}

#[test]
fn a_new_wish_while_its_look_is_drawn_whole_goes_back_to_the_ink_and_the_old_job_is_turned_away() {
    let ws = workspace("rewish");
    let maker = fake_maker(&ws, &frog(&ws));
    let mut view = view(&ws, wished_room());
    let (_, card) = ask(&mut view, &ws, Some(maker.clone()));
    watch(&mut view, &ws);
    assert!(view.shooter.as_ref().unwrap().has_mirror_body(2));
    // The same words again: the knight already is that look, nothing is drawn, nothing is lost.
    {
        let mut run = view.shooter.as_ref().unwrap().clone();
        run.begin_mirror_wish(2).unwrap();
        run.offer_wish(2, WORDS, None).unwrap();
        run.choose(2, 0).unwrap();
        view.shooter.set(Some(run));
    }
    let (again, _) = ask(&mut view, &ws, Some(maker));
    assert_eq!(again, BodyAsk::Already);
    assert_eq!((view.lookers.running(), view.lookers.queued()), (0, 0));
    assert!(view.shooter.as_ref().unwrap().has_mirror_body(2));
    // Seat 2 wishes again, other words: the old body goes at once, and the late answer for the old words changes nothing.
    {
        let words = "a heron knight";
        let run_ref = view.shooter.as_ref().unwrap();
        assert_eq!(run_ref.players[&2].worn(&run_ref.book).len(), 0);
        let mut run = run_ref.clone();
        run.begin_mirror_wish(2).unwrap();
        run.offer_wish(2, words, None).unwrap();
        run.choose(2, 0).unwrap();
        view.shooter.set(Some(run));
    }
    let run = view.shooter.as_ref().unwrap();
    assert!(!run.has_mirror_body(2));
    assert_eq!(run.book.get(&card.id).unwrap().text, "a heron knight");
    view.watch_looks(&ws);
    assert!(!looks_dir(&ws).join("pack.json").exists(), "nobody is drawn whole: the window gets the base alone");
    assert!(!view.shooter.command(Game::MirrorBody { card: card.id.clone(), words: WORDS.into(), on: true }).is_ok_and(|r| r == angel_delve::rules::host_commands::Reply::Changed(true)));
}

/// The real converter through the host, on a finished wish2char run: the proof's way in. Not run by default.
///
///   WISH_LOOK_CMD=<wish-char> WISH_CHAR_LADDER=<runs/B/pixels/ladder64> WISH_LOOK_WS=<empty dir> \
///     cargo test -p angel-realm-host --lib -- --ignored the_real_converter_through_the_host
///
/// The workspace's `.angel/dungeon/looks` is left behind for `wish_look_proof` (glass example) to shoot.
#[test]
#[ignore = "needs WISH_LOOK_CMD, WISH_CHAR_LADDER and WISH_LOOK_WS: the finished frog run through the real converter"]
fn the_real_converter_through_the_host() {
    let command = PathBuf::from(std::env::var_os("WISH_LOOK_CMD").expect("WISH_LOOK_CMD"));
    let ws = PathBuf::from(std::env::var_os("WISH_LOOK_WS").expect("WISH_LOOK_WS"));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let mut view = view(&ws, wished_room());
    let (queued, card) = ask(&mut view, &ws, Some(command));
    assert_eq!(queued, BodyAsk::Queued);
    let start = Instant::now();
    while view.lookers.running() + view.lookers.queued() > 0 && start.elapsed() < Duration::from_secs(300) {
        view.watch_looks(&ws);
        std::thread::sleep(Duration::from_millis(100));
    }
    view.watch_looks(&ws);
    assert!(view.shooter.as_ref().unwrap().has_mirror_body(2), "{}", view.notice);
    view.watch_looks(&ws);
    let looks = looks_dir(&ws);
    let pack = Pack::open(&looks).expect("composed pack");
    assert!(pack.warnings().is_empty(), "{:?}", pack.warnings());
    let run = view.shooter.as_ref().unwrap();
    let mine = drawn_by(run, &pack, 2);
    assert_eq!(mine.len(), 1);
    assert!(mine[0].1.is_some(), "seat 2 is drawn by the frog: {mine:?}");
    assert_eq!(drawn_by(run, &pack, 1)[0].1, None);
    println!("{} entries composed for {} in {}", keys_of(&looks.join("pack.json")).len(), card.id, looks.display());
}
