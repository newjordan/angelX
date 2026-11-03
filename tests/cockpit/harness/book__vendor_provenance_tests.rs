use super::*;
use crate::agent::club::ChatMsg;
use std::{path::Path, sync::Arc};

#[test]
fn vendor_attribution_follows_the_pinned_generation_across_readers() {
    let route = d23457_codex_sol::AUTONOMY;
    let cells = route.cells();
    let page = format!("{cells}{}", DIGITS[0]);
    let read = |address: &str| ledger::read(Path::new("."), address).unwrap();
    let intro =
        |address: &str| introduction::introductions(&[ChatMsg::harness(address.to_owned())], None);
    let baseline = Arc::new(catalog::Catalog::baseline());
    let baseline_read;
    let baseline_recite;
    let baseline_intro;
    let baseline_chapter;
    {
        let _scope = catalog::enter(baseline.clone());
        baseline_read = read(&page);
        baseline_recite = connect::recite(&page);
        baseline_intro = intro(&page);
        baseline_chapter = read(&route.primary.to_string());
        assert!(!baseline_read.contains(PROFILE_OVERRIDE_NOTICE));
        assert!(read(&cells).contains("what Codex tells gpt-6.1-sol"));
    }
    let mut edited = catalog::Catalog::baseline();
    edited.set_test_page(route, 0, "Locally edited profile sentence.");
    {
        let _scope = catalog::enter(Arc::new(edited));
        assert!(read(&page).contains(PROFILE_OVERRIDE_NOTICE));
        let section = read(&cells);
        assert!(section.contains(PROFILE_OVERRIDE_NOTICE));
        assert!(!section.contains("what Codex tells gpt-6.1-sol"));
        assert!(read(&route.primary.to_string()).contains("locally modified Codex-derived"));
        assert!(connect::recite(&page).contains(PROFILE_OVERRIDE_NOTICE));
        assert!(connect::recite(&cells).contains(PROFILE_OVERRIDE_NOTICE));
        assert!(
            intro(&page)
                .iter()
                .any(|(_, text)| text.contains(PROFILE_OVERRIDE_NOTICE))
        );
        assert!(
            intro(&cells)
                .iter()
                .any(|(_, text)| text.contains("local override of the Codex-derived profile"))
        );
    }
    {
        let _scope = catalog::enter(baseline);
        assert_eq!(read(&page), baseline_read);
        assert_eq!(connect::recite(&page), baseline_recite);
        assert_eq!(intro(&page), baseline_intro);
        assert_eq!(read(&route.primary.to_string()), baseline_chapter);
    }
}

#[test]
fn embedded_vendor_sections_match_independent_original_identities() {
    for primary in [
        &d23457_codex_sol::PRIMARY,
        &d23457_codex_sol::SHELF_II,
        &d23457_codex_sol::SHELF_III,
        &d23457_codex_sol::CONTEXT,
    ] {
        for sub in primary.subs {
            assert!(
                !profile_override(&ResolvedSub::from(sub)),
                "{}",
                sub.route.cells()
            );
        }
    }
}
