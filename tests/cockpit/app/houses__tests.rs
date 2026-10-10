use super::*;

#[test]
fn every_family_has_a_house_named_by_its_own_model() {
    let houses = all();
    assert_eq!(houses.len(), 16);
    let json: serde_json::Value = serde_json::from_str(ASSET).unwrap();
    for house in houses {
        let entry = json["houses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == house.key)
            .unwrap_or_else(|| panic!("{} missing from houses.json", house.key));
        if house.placeholder {
            assert!(
                house.knights.is_empty(),
                "{}: a placeholder names no one",
                house.key
            );
            continue;
        }
        // Every name comes from the model's own reply, verbatim.
        let reply = entry["reply"].as_str().unwrap();
        assert!(
            reply.contains(&house.castle),
            "{}: castle not in reply",
            house.key
        );
        for k in &house.knights {
            assert!(
                reply.contains(&k.name),
                "{}: {} not in reply",
                house.key,
                k.name
            );
        }
        assert_eq!(house.knights.len(), 6, "{}", house.key);
        assert!(!entry["model"].as_str().unwrap().is_empty());
        assert_eq!(entry["asked"], "2026-10-09");
    }
}

#[test]
fn no_two_houses_share_a_knight_or_castle_name_unless_it_is_listed_open() {
    let json: serde_json::Value = serde_json::from_str(ASSET).unwrap();
    let first = |name: &str| {
        name.split_whitespace()
            .find(|w| {
                !matches!(
                    w.to_ascii_lowercase().as_str(),
                    "sir" | "dame" | "lady" | "lord" | "knight"
                )
            })
            .unwrap_or(name)
            .to_ascii_lowercase()
    };
    let mut held: std::collections::BTreeMap<String, &str> = Default::default();
    for house in all() {
        let entry = json["houses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == house.key)
            .unwrap();
        let open = entry["open_collisions"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let mut names = vec![format!("castle:{}", house.castle.to_ascii_lowercase())];
        names.extend(
            house
                .knights
                .iter()
                .map(|k| format!("knight:{}", first(&k.name))),
        );
        for (n, k) in names.iter().zip(
            std::iter::once(house.castle.as_str())
                .chain(house.knights.iter().map(|k| k.name.as_str())),
        ) {
            if let Some(owner) = held.get(n) {
                assert!(
                    open.contains(k),
                    "{} reuses {k} from {owner} and does not list it as open",
                    house.key
                );
            } else {
                held.insert(n.clone(), house.key);
            }
        }
    }
}

#[test]
fn banners_are_signal_inks_and_no_two_houses_fly_the_same_pair() {
    use crate::stage::world_viz::overworld::ink;
    let mut seen = std::collections::BTreeSet::new();
    for house in all().iter().filter(|h| !h.placeholder) {
        for c in [house.field, house.charge] {
            let rgb = ink::ink(c).unwrap();
            assert!(
                ink::is_signal(rgb),
                "{}: {c} is not a signal ink",
                house.key
            );
        }
        assert!(
            seen.insert((house.field, house.charge)),
            "{} repeats a banner",
            house.key
        );
    }
}

#[test]
fn a_route_serves_from_its_model_familys_castle_never_a_machine() {
    let kimi = by_key("kimi").unwrap();
    assert_eq!(of_route("kimi", "kimi", Some("kimi-k3")), Some(kimi));
    // A self-hosted model is a mode of `local`: its family's house.
    let qwen = by_key("qwen").unwrap();
    assert_eq!(
        of_route("local", "local", Some("Qwen3.6-35B-A3B")),
        Some(qwen)
    );
    assert_eq!(
        of_route("local", "local", Some("deepseek-v4-flash")),
        by_key("deepseek")
    );
    assert_eq!(
        of_route("sota", "openai", Some("gpt-6.1-sol")),
        by_key("sol")
    );
    assert_eq!(
        of_route("sota", "openai", Some("gpt-6-astra")),
        by_key("astra")
    );
    // A route with no family has no castle: the Keep's own knight serves.
    assert_eq!(of_route("practice", "practice", None), None);
    assert_eq!(of_route("local", "local", Some("my-finetune")), None);
    for name in ["turbo", "atlas", "spark", "apollo", "toymaker"] {
        assert!(
            all()
                .iter()
                .all(|h| !h.key.contains(name) && !h.family.to_lowercase().contains(name)),
            "a machine has a castle: {name}"
        );
    }
}

#[test]
fn party_seats_come_from_the_serving_houses_then_the_march() {
    let none = Serving::default();
    assert_eq!(for_seat(&none, 1), None, "nothing serves: the Keep's own");
    let grok = by_key("grok").unwrap();
    let kimi = by_key("kimi").unwrap();
    let serving = Serving {
        lead: Some(grok),
        seated: vec![kimi, grok],
        turn: false,
    };
    assert_eq!(for_seat(&serving, 1), Some(grok));
    assert_eq!(for_seat(&serving, 2), Some(kimi));
    let third = for_seat(&serving, 3).unwrap();
    assert!(third != grok && third != kimi);
    for seat in 1..40 {
        assert!(for_seat(&serving, seat).is_some());
    }
}

/// The Delve's intro with a house serving, as text, for review:
/// `cargo test --bin angel print_delve_intro_with_houses -- --ignored --nocapture`.
#[test]
#[ignore]
fn print_delve_intro_with_houses() {
    use ratatui::{Terminal, backend::TestBackend};
    for (lead, kit) in [
        ("deepseek", 0),
        ("nemotron", 1),
        ("kimi", 2),
        ("longcat", 0),
    ] {
        note_serving(&Serving {
            lead: by_key(lead),
            seated: Vec::new(),
            turn: false,
        });
        let intro = crate::ui::viz::delve_intro_viz::Intro {
            knight: kit,
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(120, 34)).unwrap();
        terminal
            .draw(|frame| {
                crate::ui::viz::delve_intro_viz::render(frame, frame.area(), &intro, None)
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let text: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect();
        println!("==== {lead}\n{}", text.join("\n"));
    }
    note_serving(&Serving::default());
}
