//! Home fixtures use the same seat-local interaction boards as the tavern.
use super::*;

#[test]
fn every_home_arm_prompt_names_the_arm_and_explains_the_free_hold_or_swap() {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_alehouse();
    for (slot, card) in a::home_arms().iter().enumerate() {
        r.players.get_mut(&1).unwrap().arm = None;
        let lines = prompt(&r, 1, Spot::Arm(slot));
        assert!(lines.iter().any(|(l, _)| l == &card.name.to_uppercase()));
        assert!(lines.iter().any(|(l, _)| l.contains("FREE")));
        assert!(lines.iter().any(|(l, _)| l.contains("HOLD E TO EQUIP")));
        r.players.get_mut(&1).unwrap().arm = Some(card.id.clone());
        let lines = prompt(&r, 1, Spot::Arm(slot));
        assert!(lines.iter().any(|(l, _)| l.contains("EQUIPPED") && l.contains("SWAP")));
    }
    assert!(crate::client::sheet(&r, 1).contains("FREE WEAPONS WALL"));
}

/// Local repeatable hot-path probe, not an exporter or a host workload.
#[test]
#[ignore]
fn measure_tavern_figures_hot_path() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    let mut cv = Img::new(576, 320);
    figures(&mut cv, &r);
    let start = std::time::Instant::now();
    for _ in 0..240 {
        figures(&mut cv, &r);
        std::hint::black_box(&cv);
    }
    eprintln!("tavern figures cached: {:?} / 240 frames", start.elapsed());
    // Isolate the removed work on the same final visible page, independent
    // of which three kits were permanently visible in an earlier draft.
    let start = std::time::Instant::now();
    for _ in 0..240 {
        for p in PATRONS.iter().enumerate().filter(|(i, _)| a::patron_visible(0, *i)).map(|(_, p)| p) {
            let colours = crate::rules::knights::knight(p.kit).unwrap().colours;
            let body = match crate::rules::champions::Champion::of_kit(p.kit) {
                Some(c) => super::super::champions::standing(c),
                None => knights::of_kit(Some(p.kit), colours),
            };
            std::hint::black_box(body);
        }
    }
    eprintln!("removed builtin body construction: {:?} / 240 frames", start.elapsed());
}
