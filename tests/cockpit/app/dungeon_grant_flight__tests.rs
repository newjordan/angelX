use super::*;

/// Local flight through the production brief, runner, checker and mend:
/// a local model as the realm's oracle, three wishes, each answer kept.
#[test]
#[ignore]
fn local_model_dungeon_grant_flight() {
    use crate::agent::club::{Club, HttpClub};
    use angel_realm_host::control::dungeon_reforge::grant_brief_for;
    use angel_realm_host::together_grants::{Job, Runners};
    use angel_realm_host::together_shooter::dead_wishes::Target;
    use std::sync::Arc;

    let url = std::env::var("ANGEL_DUNGEON_FLIGHT_URL").expect("explicit local endpoint required");
    let model = std::env::var("ANGEL_DUNGEON_FLIGHT_MODEL").expect("explicit model required");
    let out = std::path::PathBuf::from(
        std::env::var_os("ANGEL_DUNGEON_FLIGHT_OUT").expect("output directory required"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let club: Arc<dyn Club> = Arc::new(HttpClub::new("dungeon-flight", url, model, None));
    let oracle = crate::app::control::realm::club_oracle(club);
    let runners = Runners::new(1);
    for (index, words) in ["stars spinning round me", "ticking poison damage", "a bigger spiked shield"]
        .into_iter()
        .enumerate()
    {
        let brief = grant_brief_for("A knight", words, None, Target::You);
        std::fs::write(out.join(format!("wish-{index}-brief.txt")), &brief).unwrap();
        runners.send(Job {
            id: index as u64,
            oracle: oracle.clone(),
            brief,
            sheets: out.join("sheets"),
            paint: out.join("paint"),
        });
        let started = std::time::Instant::now();
        let done = loop {
            if let Some(done) = runners.poll().into_iter().next() {
                break done;
            }
            assert!(started.elapsed() < Duration::from_secs(330), "grant runner did not return");
            std::thread::sleep(Duration::from_millis(100));
        };
        let mut result = format!(
            "words: {words}\nqueue: {:?}\nfirst call: {:?}\nmend: {:?}\n",
            done.timing.queued, done.timing.model, done.timing.mend
        );
        match done.result {
            Ok((entry, wish)) => result.push_str(&format!("CHECKER ACCEPTED: {}\n{entry}", wish.id)),
            Err(error) => result.push_str(&format!("CHECKER/TRANSPORT REJECTED: {error}\n")),
        }
        std::fs::write(out.join(format!("wish-{index}-result.txt")), &result).unwrap();
        println!("{result}");
    }
}
