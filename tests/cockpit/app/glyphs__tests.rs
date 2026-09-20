use super::*;
use unicode_width::UnicodeWidthStr;

#[test]
fn glyph_marks_are_static_and_compact() {
    for glyph in Glyph::ALL {
        assert_eq!(glyph.mark().len(), 2);
        assert!(glyph.mark().is_ascii());
        assert_eq!(
            UnicodeWidthStr::width(glyph.rich_mark()),
            2,
            "{glyph:?}: {:?}",
            glyph.rich_mark()
        );
        assert!(!glyph.label().is_empty());
    }
    assert_eq!(Glyph::Active.mark(), "*>");
    assert_eq!(Glyph::Thinking.label(), "thinking");
    assert_eq!(Glyph::Online.mark(), "ON");
    assert_eq!(Glyph::Offline.mark(), "NO");
    assert_eq!(activity(false), Glyph::Idle);
    assert_eq!(activity(true), Glyph::Thinking);
}

#[test]
fn system_and_event_glyphs_form_a_tiny_fixed_width_language() {
    for glyph in [
        Glyph::Tool,
        Glyph::Result,
        Glyph::Check,
        Glyph::Warning,
        Glyph::Online,
        Glyph::Offline,
        Glyph::Busy,
        Glyph::Queued,
        Glyph::Paused,
        Glyph::Blocked,
        Glyph::Error,
        Glyph::Ready,
        Glyph::Stale,
        Glyph::TransferIn,
        Glyph::TransferOut,
        Glyph::Sync,
        Glyph::Bridge,
        Glyph::Remote,
        Glyph::Local,
        Glyph::Cpu,
        Glyph::Gpu,
        Glyph::Memory,
        Glyph::Network,
        Glyph::Disk,
        Glyph::Search,
        Glyph::Mail,
    ] {
        assert_eq!(glyph.mark().len(), 2);
        assert!(glyph.mark().is_ascii());
        assert_eq!(
            UnicodeWidthStr::width(glyph.rich_mark()),
            2,
            "{glyph:?}: {:?}",
            glyph.rich_mark()
        );
    }
    assert_eq!(Glyph::Tool.mark(), "T:");
    assert_eq!(Glyph::Result.mark(), "R:");
    assert_eq!(Glyph::Network.label(), "network");
    assert_eq!(Glyph::Busy.label(), "busy");
    assert_eq!(Glyph::TransferIn.mark(), "<-");
    assert_eq!(Glyph::TransferOut.mark(), "->");
    assert_eq!(Glyph::named("wait"), Some(Glyph::Queued));
    assert_eq!(Glyph::named("hold"), Some(Glyph::Paused));
    assert_eq!(Glyph::named("stuck"), Some(Glyph::Blocked));
    assert_eq!(Glyph::named("fail"), Some(Glyph::Error));
    assert_eq!(Glyph::named("ready"), Some(Glyph::Ready));
    assert_eq!(Glyph::named("old"), Some(Glyph::Stale));
    assert_eq!(Glyph::named("xfer-in"), Some(Glyph::TransferIn));
    assert_eq!(Glyph::named("xfer-out"), Some(Glyph::TransferOut));
    assert_eq!(Glyph::named("route"), Some(Glyph::Bridge));
}

#[test]
fn agent_work_glyphs_are_fixed_width_and_alias_aware() {
    for glyph in [
        Glyph::Plan,
        Glyph::Code,
        Glyph::Review,
        Glyph::Patch,
        Glyph::Brain,
        Glyph::Spark,
        Glyph::Scout,
        Glyph::Planner,
        Glyph::Coder,
        Glyph::Reviewer,
        Glyph::Runner,
        Glyph::Specialist,
        Glyph::Moa,
        Glyph::Lock,
        Glyph::Key,
        Glyph::Session,
        Glyph::Home,
    ] {
        assert_eq!(glyph.mark().len(), 2);
        assert!(glyph.mark().is_ascii());
        assert_eq!(
            UnicodeWidthStr::width(glyph.rich_mark()),
            2,
            "{glyph:?}: {:?}",
            glyph.rich_mark()
        );
    }
    assert_eq!(Glyph::named(":plan:"), Some(Glyph::Plan));
    assert_eq!(Glyph::named("edit"), Some(Glyph::Code));
    assert_eq!(Glyph::named("audit"), Some(Glyph::Review));
    assert_eq!(Glyph::named("diff"), Some(Glyph::Patch));
    assert_eq!(Glyph::named("llm"), Some(Glyph::Brain));
    assert_eq!(Glyph::named("insight"), Some(Glyph::Spark));
    assert_eq!(Glyph::named("scout"), Some(Glyph::Scout));
    assert_eq!(Glyph::named("planner"), Some(Glyph::Planner));
    assert_eq!(Glyph::named("coder"), Some(Glyph::Coder));
    assert_eq!(Glyph::named("reviewer"), Some(Glyph::Reviewer));
    assert_eq!(Glyph::named("exec"), Some(Glyph::Runner));
    assert_eq!(Glyph::named("spec"), Some(Glyph::Specialist));
    assert_eq!(Glyph::named("fleet"), Some(Glyph::Moa));
    assert_eq!(Glyph::named("secure"), Some(Glyph::Lock));
    assert_eq!(Glyph::named("auth"), Some(Glyph::Key));
    assert_eq!(Glyph::named("thread"), Some(Glyph::Session));
    assert_eq!(Glyph::named("workspace"), Some(Glyph::Home));
}

#[test]
fn surface_glyphs_are_fixed_width_and_alias_aware() {
    for glyph in [
        Glyph::Surface,
        Glyph::Scene,
        Glyph::Avatar,
        Glyph::Transcript,
        Glyph::Terminal,
        Glyph::Carousel,
    ] {
        assert_eq!(glyph.mark().len(), 2);
        assert!(glyph.mark().is_ascii());
        assert_eq!(
            UnicodeWidthStr::width(glyph.rich_mark()),
            2,
            "{glyph:?}: {:?}",
            glyph.rich_mark()
        );
    }
    assert_eq!(Glyph::named("surface"), Some(Glyph::Surface));
    assert_eq!(Glyph::named("scene"), Some(Glyph::Scene));
    assert_eq!(Glyph::named("portrait"), Some(Glyph::Avatar));
    assert_eq!(Glyph::named("log"), Some(Glyph::Transcript));
    assert_eq!(Glyph::named("tty"), Some(Glyph::Terminal));
    assert_eq!(Glyph::named("media"), Some(Glyph::Carousel));
    assert_eq!(Glyph::named("shell"), Some(Glyph::Shell));
}

#[test]
fn tiny_font_names_resolve_to_static_tokens() {
    assert_eq!(Glyph::named("live"), Some(Glyph::Online));
    assert_eq!(Glyph::named("down"), Some(Glyph::Offline));
    assert_eq!(Glyph::named(":loading:"), Some(Glyph::Loading));
    assert_eq!(Glyph::named("working"), Some(Glyph::Busy));
    assert_eq!(Glyph::named("xfer"), Some(Glyph::Transfer));
    assert_eq!(Glyph::named("download"), Some(Glyph::TransferIn));
    assert_eq!(Glyph::named("upload"), Some(Glyph::TransferOut));
    assert_eq!(Glyph::named("ready"), Some(Glyph::Ready));
    assert_eq!(Glyph::named("net"), Some(Glyph::Network));
    assert_eq!(Glyph::named("find"), Some(Glyph::Search));
    assert_eq!(token(":active:"), Some(Glyph::Active.mark()));
    assert_eq!(token(":code:"), Some(Glyph::Code.mark()));
    assert_eq!(token(":agent.spark-r1:"), Some(AgentBadge::Sparky.mark()));
    assert_eq!(token(":mail:"), Some(Glyph::Mail.mark()));
    assert_eq!(token("agent.atlas"), Some(AgentBadge::Atlas.mark()));
    assert_eq!(label("agent.ghost-protocol"), Some("casper"));
    assert_eq!(Glyph::named("not-a-token"), None);
    assert_eq!(token("not-a-token"), None);
}

#[test]
fn agent_badges_are_static_compact_and_alias_aware() {
    for badge in AgentBadge::ALL {
        assert_eq!(badge.mark().len(), 2);
        assert!(badge.mark().is_ascii());
        assert_eq!(UnicodeWidthStr::width(badge.token()), 2);
        assert_eq!(UnicodeWidthStr::width(badge.rich_mark()), 2);
        assert!(!badge.label().is_empty());
    }
    assert_eq!(agent_badge("turbo"), AgentBadge::Turbo);
    assert_eq!(agent_badge(" Atlas "), AgentBadge::Atlas);
    assert_eq!(agent_badge("spark-r1"), AgentBadge::Sparky);
    assert_eq!(agent_badge("DICE"), AgentBadge::Apollo);
    assert_eq!(agent_badge("ghost-protocol"), AgentBadge::Casper);
    assert_eq!(agent_badge("scout"), AgentBadge::Strixus);
    assert_eq!(agent_badge("unknown"), AgentBadge::Unknown);
    assert_eq!(AgentBadge::named("turbo"), Some(AgentBadge::Turbo));
    assert_eq!(AgentBadge::named(" Atlas "), Some(AgentBadge::Atlas));
    assert_eq!(AgentBadge::named("agent.DICE"), Some(AgentBadge::Apollo));
    assert_eq!(
        AgentBadge::named("ghost-protocol"),
        Some(AgentBadge::Casper)
    );
    assert_eq!(AgentBadge::named("unknown"), None);
}

#[test]
fn every_glyph_alias_resolves_to_its_token() {
    // Locks the documented alias vocabulary: each input string must resolve to
    // exactly the intended glyph (every `||` branch of `Glyph::named`). A
    // regression that drops or reroutes an alias trips here.
    let cases: &[(&str, Glyph)] = &[
        ("active", Glyph::Active),
        ("run", Glyph::Active),
        ("online", Glyph::Online),
        ("offline", Glyph::Offline),
        ("idle", Glyph::Idle),
        ("sleep", Glyph::Idle),
        ("thinking", Glyph::Thinking),
        ("think", Glyph::Thinking),
        ("reasoning", Glyph::Thinking),
        ("loading", Glyph::Loading),
        ("load", Glyph::Loading),
        ("busy", Glyph::Busy),
        ("work", Glyph::Busy),
        ("queued", Glyph::Queued),
        ("queue", Glyph::Queued),
        ("waiting", Glyph::Queued),
        ("paused", Glyph::Paused),
        ("pause", Glyph::Paused),
        ("blocked", Glyph::Blocked),
        ("block", Glyph::Blocked),
        ("error", Glyph::Error),
        ("err", Glyph::Error),
        ("failed", Glyph::Error),
        ("stale", Glyph::Stale),
        ("transfer", Glyph::Transfer),
        ("transfer-in", Glyph::TransferIn),
        ("recv", Glyph::TransferIn),
        ("receive", Glyph::TransferIn),
        ("in", Glyph::TransferIn),
        ("transfer-out", Glyph::TransferOut),
        ("send-out", Glyph::TransferOut),
        ("out", Glyph::TransferOut),
        ("sync", Glyph::Sync),
        ("bridge", Glyph::Bridge),
        ("remote", Glyph::Remote),
        ("local", Glyph::Local),
        ("agent", Glyph::Agent),
        ("sh", Glyph::Shell),
        ("artifacts", Glyph::Artifacts),
        ("artifact", Glyph::Artifacts),
        ("avatar", Glyph::Avatar),
        ("transcript", Glyph::Transcript),
        ("terminal", Glyph::Terminal),
        ("carousel", Glyph::Carousel),
        ("approval", Glyph::Approval),
        ("approve", Glyph::Approval),
        ("send", Glyph::Send),
        ("enter", Glyph::Send),
        ("back", Glyph::Back),
        ("return", Glyph::Back),
        ("switch", Glyph::Switch),
        ("tab", Glyph::Switch),
        ("tool", Glyph::Tool),
        ("tools", Glyph::Tool),
        ("done", Glyph::Result), // "result" maps to Result; "done" too
        ("check", Glyph::Check),
        ("ok", Glyph::Check),
        ("warning", Glyph::Warning),
        ("warn", Glyph::Warning),
        ("cpu", Glyph::Cpu),
        ("gpu", Glyph::Gpu),
        ("memory", Glyph::Memory),
        ("mem", Glyph::Memory),
        ("network", Glyph::Network),
        ("disk", Glyph::Disk),
        ("store", Glyph::Disk),
        ("search", Glyph::Search),
        ("mail", Glyph::Mail),
        ("inbox", Glyph::Mail),
        ("plan", Glyph::Plan),
        ("todo", Glyph::Plan),
        ("code", Glyph::Code),
        ("review", Glyph::Review),
        ("patch", Glyph::Patch),
        ("brain", Glyph::Brain),
        ("model", Glyph::Brain),
        ("spark", Glyph::Spark),
        ("idea", Glyph::Spark),
        ("specialist", Glyph::Specialist),
        ("moa", Glyph::Moa),
        ("lock", Glyph::Lock),
        ("key", Glyph::Key),
        ("session", Glyph::Session),
        ("chat", Glyph::Session),
        ("home", Glyph::Home),
        ("root", Glyph::Home),
    ];
    for (name, expect) in cases {
        assert_eq!(Glyph::named(name), Some(*expect), "alias {name:?}");
        // Case-insensitivity + the `token`/`label` lookups must agree.
        assert_eq!(
            Glyph::named(&name.to_uppercase()),
            Some(*expect),
            "alias {name:?} is case-insensitive"
        );
        assert_eq!(token(name), Some(expect.mark()));
        assert_eq!(label(name), Some(expect.label()));
    }
    // `ready` is shared: it resolves to Ready (checked before Result).
    assert_eq!(Glyph::named("ready"), Some(Glyph::Ready));
    // Unknown names fall through to None on every public lookup.
    assert_eq!(Glyph::named("definitely-not-a-glyph"), None);
    assert_eq!(token("definitely-not-a-glyph"), None);
    assert_eq!(label("definitely-not-a-glyph"), None);
}

#[test]
fn agent_badge_aliases_and_token_routing() {
    // Names here are badge-unique (no colliding Glyph alias), so the public
    // `token`/`label` lookups route through the badge.
    let cases: &[(&str, AgentBadge)] = &[
        ("sparky", AgentBadge::Sparky),
        ("spark-v4", AgentBadge::Sparky),
        ("apollo", AgentBadge::Apollo),
        ("dice", AgentBadge::Apollo),
        ("casper", AgentBadge::Casper),
        ("ghost", AgentBadge::Casper),
        ("strixus", AgentBadge::Strixus),
    ];
    for (name, expect) in cases {
        assert_eq!(AgentBadge::named(name), Some(*expect), "badge {name:?}");
        assert_eq!(agent_badge(name), *expect);
        // Public `token()`/`label()` route through the badge when no Glyph matches.
        assert_eq!(token(name), Some(expect.mark()));
        assert_eq!(label(name), Some(expect.label()));
    }
    // `spark` is a SHARED alias: `AgentBadge::named` resolves it to Sparky, but
    // the public `token` lookup checks `Glyph` first, so it wins as Glyph::Spark.
    assert_eq!(AgentBadge::named("spark"), Some(AgentBadge::Sparky));
    assert_eq!(token("spark"), Some(Glyph::Spark.mark()));
    // The `:agent.<name>:` symbol form trims to the badge.
    assert_eq!(AgentBadge::named(":agent.turbo:"), Some(AgentBadge::Turbo));
    // Unknown badge → None, and `agent_badge` (its lossy cousin) → Unknown.
    assert_eq!(AgentBadge::named("nobody"), None);
    assert_eq!(agent_badge("nobody"), AgentBadge::Unknown);
    // The Unknown badge still has a real 2-cell mark/token.
    assert_eq!(AgentBadge::Unknown.mark(), "A>");
    assert_eq!(AgentBadge::Unknown.label(), "agent");
}

#[test]
fn symbol_name_trimming_strips_colons_whitespace_and_agent_prefix() {
    // `trim_symbol_name` = trim_ascii ∘ trim_colons ∘ trim_agent_prefix, all
    // reached through `named`. Each wrapper must compose.
    assert_eq!(Glyph::named("  :code:  "), Some(Glyph::Code));
    assert_eq!(Glyph::named("AGENT.cpu"), Some(Glyph::Cpu));
    assert_eq!(Glyph::named(":agent.gpu:"), Some(Glyph::Gpu));
    // A lone colon pair with no inner name trims to empty → no match.
    assert_eq!(Glyph::named("::"), None);
}

#[test]
fn media_prefixes_map_known_artifact_kinds() {
    assert_eq!(media_prefix("image"), "I> img   ");
    assert_eq!(media_prefix("img"), "I> img   ");
    assert_eq!(media_prefix("video"), "V> reel  ");
    assert_eq!(media_prefix("link"), "L> link  ");
    assert_eq!(media_prefix("graph"), "G> graph ");
    assert_eq!(media_prefix("res"), "R> res   ");
    assert_eq!(media_prefix("unknown"), "L> link  ");
}

#[test]
fn rich_media_prefixes_remain_fixed_width() {
    for prefix in [
        "▧  img   ",
        "▶  reel  ",
        "↗  link  ",
        "⌁  graph ",
        "◫  res   ",
    ] {
        assert_eq!(UnicodeWidthStr::width(prefix), 9);
    }
}
