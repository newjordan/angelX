use super::*;

const STARLIGHT: &str = "wish starlight | Starlight Arrows\nsay make my arrows glow like stars | starlight arrows | glowing arrows\ntier homing 1; damage 10 :: Arrows glow and seek.\ntier homing 2; damage 15 :: Arrows burn bright and seek.\ntier homing 2; damage 20 :: Arrows are stars.\n";
const DEVILS_HELD: &str = "wish golden-devils | Golden Devils\nsay tiny golden devils that explode\ntier summon 2 swarm :: Devils.\n";
const DEVILS_SPELL: &str = "wish golden-devils | Golden Devils\nkind spell\nsay tiny golden devils that explode\ntier summon 2 swarm; cooldown 8 :: Devils.\ndraw summon\n.5.|5.5\n555|555\n";

fn job(id: u64, oracle: Arc<dyn Oracle>) -> Job {
    let dir = std::env::temp_dir().join("delve-grants-no-pictures");
    Job { id, oracle, brief: format!("wish number {id}"), sheets: dir.join("sheets"), paint: dir.join("paint") }
}

/// Every answer from `runners` until `n` are back, or `within` passes.
fn gather(runners: &Runners, n: usize, within: Duration) -> Vec<Done> {
    let start = Instant::now();
    let mut done = Vec::new();
    while done.len() < n && start.elapsed() < within {
        done.extend(runners.poll());
        std::thread::sleep(Duration::from_millis(5));
    }
    done
}

#[test]
fn the_entry_is_read_out_of_a_reply_with_its_fence_and_chatter() {
    let reply = format!("Here is the entry:\n```\n{STARLIGHT}```\nEnjoy!");
    assert_eq!(entry_of(&reply).as_deref(), Some(STARLIGHT));
    assert_eq!(entry_of(STARLIGHT).as_deref(), Some(STARLIGHT));
    assert!(entry_of("I cannot do that.").is_none());
    // Two entries: the first is the wish.
    let two = format!("{STARLIGHT}\n{DEVILS_SPELL}");
    assert_eq!(entry_of(&two).unwrap().trim_end(), STARLIGHT.trim_end());
}

#[test]
fn the_runners_check_an_entry_with_the_games_own_checker() {
    let dir = std::env::temp_dir().join("delve-grants-no-pictures");
    assert_eq!(check(STARLIGHT, &dir, &dir), Ok("starlight".to_string()));
    let turned = check(DEVILS_HELD, &dir, &dir).unwrap_err();
    assert!(turned.join(" ").contains("spell or play"), "{turned:?}");
    assert_eq!(check(DEVILS_SPELL, &dir, &dir), Ok("golden-devils".to_string()));
}

#[test]
fn six_wishes_are_written_at_once_not_one_after_another() {
    let delay = Duration::from_millis(400);
    let club = Scripted::new(&[STARLIGHT], delay);
    let runners = Runners::new(RUNNERS);
    let start = Instant::now();
    for id in 1..=6 {
        runners.send(job(id, club.clone()));
    }
    let done = gather(&runners, 6, Duration::from_secs(10));
    let took = start.elapsed();
    assert_eq!(done.len(), 6);
    assert!(done.iter().all(|d| d.result.is_ok()));
    assert_eq!(club.most_inside.load(Ordering::SeqCst), 6, "all six in flight together");
    // One after another would be 2.4 s; together, about one call's time.
    assert!(took < delay * 3, "six wishes took {took:?}");
    assert!(done.iter().all(|d| d.timing.queued < Duration::from_millis(200)), "none waited for a runner");
}

#[test]
fn an_entry_the_checker_turns_away_is_mended_once_on_its_runner() {
    let club = Scripted::new(&[DEVILS_HELD, DEVILS_SPELL], Duration::ZERO);
    let runners = Runners::new(2);
    runners.send(job(7, club.clone()));
    let done = gather(&runners, 1, Duration::from_secs(5));
    let (entry, id) = done[0].result.clone().expect("mended");
    assert_eq!(id.id, "golden-devils");
    assert!(entry.contains("kind spell"));
    assert!(done[0].timing.mend.is_some());
    let asked = club.asked.lock().unwrap();
    assert_eq!(asked.len(), 2);
    assert!(asked[1].contains("THE GAME'S CHECKER TURNED IT AWAY") && asked[1].contains("spell or play"), "{}", asked[1]);
}

#[test]
fn a_wish_the_model_cannot_write_comes_back_with_why() {
    let club = Scripted::new(&["Sorry, no."], Duration::ZERO);
    let runners = Runners::new(1);
    runners.send(job(9, club));
    let done = gather(&runners, 1, Duration::from_secs(5));
    assert!(done[0].result.as_ref().is_err_and(|why| why.contains("without an entry")));
}

#[test]
fn an_entry_takes_a_new_id_when_the_book_has_its_own() {
    let entry = renamed(STARLIGHT, "starlight", "starlight-2");
    assert!(entry.starts_with("wish starlight-2 | Starlight Arrows\n"));
    assert_eq!(entry.lines().count(), STARLIGHT.lines().count());
}

#[test]
fn a_call_that_times_out_is_asked_once_more_with_longer_and_the_wish_lands() {
    // DeepSeek Flash on wave36's fresh box: the first call ran past the
    // clubs' 60 s bound; the retry, given longer, answers.
    let club = Scripted::new(
        &["ERR: decode response: timed out reading response", STARLIGHT],
        Duration::ZERO,
    );
    let runners = Runners::new(1);
    runners.send(job(11, club.clone()));
    let done = gather(&runners, 1, Duration::from_secs(5));
    let (_, id) = done[0].result.clone().expect("the retry grants");
    assert_eq!(id.id, "starlight");
    let asked = club.asked.lock().unwrap();
    assert_eq!(asked.len(), 2, "one retry");
    assert_eq!(asked[0], asked[1], "the same wish asked again");
    // A 60 s club bound is raised for a grant's call, and more for its retry.
    assert_eq!(*club.bounds.lock().unwrap(), vec![GRANT_CALL, GRANT_RETRY]);
}

#[test]
fn a_wish_whose_retry_also_fails_comes_back_with_why_after_one_retry_only() {
    let club = Scripted::new(&["ERR: decode response: timed out reading response"], Duration::ZERO);
    let runners = Runners::new(1);
    runners.send(job(12, club.clone()));
    let done = gather(&runners, 1, Duration::from_secs(5));
    assert!(done[0].result.as_ref().is_err_and(|why| why.contains("could not be reached") && why.contains("timed out")));
    assert_eq!(club.asked.lock().unwrap().len(), 2);
}

#[test]
fn the_pool_arc_can_cross_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<super::Runners>();
    assert_send_sync::<std::sync::Arc<super::Runners>>();
}
