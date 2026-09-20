use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

const LEGACY: &str = r#"{
      "version": 1,
      "reports": [{
        "slug": "fleet-latency",
        "title": "Fleet Latency Survey",
        "date": "2026-07-01",
        "category": "benchmarks",
        "tags": ["fleet"],
        "file": "fleet-latency.html",
        "summary": "Legacy report"
      }]
    }"#;

const VERSION_TWO: &str = include_str!("../../../cockpit/fixtures/observatory/manifest-v2.json");

fn long_manifest(campaigns: usize, reports_per_campaign: usize) -> String {
    let reports = (0..campaigns)
        .flat_map(|campaign| {
            (0..reports_per_campaign).map(move |report| {
                json!({
                    "slug": format!("report-{campaign:02}-{report:02}"),
                    "title": format!("Report {campaign:02} {report:02}"),
                    "date": "2026-07-13",
                    "category": "verification",
                    "file": format!("reports/report-{campaign:02}-{report:02}.html"),
                    "summary": "Retained evidence",
                    "campaignId": format!("campaign-{campaign:02}"),
                    "campaignTitle": format!("Campaign {campaign:02}"),
                    "status": "active",
                    "reportKind": "evidence",
                    "evidencePath": format!("evidence/report-{campaign:02}-{report:02}.json"),
                    "linkedRunIds": [format!("run-{campaign:02}-{report:02}")]
                })
            })
        })
        .collect::<Vec<_>>();
    json!({ "version": 2, "reports": reports }).to_string()
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let area = *buffer.area();
    (area.y..area.y + area.height)
        .map(|y| {
            (area.x..area.x + area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|cell| cell.symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn legacy_reports_map_explicitly_to_unassigned() {
    let catalog = Catalog::from_str(Path::new("/tmp/reports/manifest.json"), LEGACY).unwrap();
    assert_eq!(catalog.version, LEGACY_MANIFEST_VERSION);
    assert_eq!(catalog.campaigns.len(), 1);
    assert_eq!(catalog.campaigns[0].id, UNASSIGNED_CAMPAIGN_ID);
    assert_eq!(catalog.campaigns[0].title, UNASSIGNED_CAMPAIGN_TITLE);
    assert_eq!(catalog.campaigns[0].status, "unassigned");
    assert_eq!(catalog.reports[0].campaign_id, UNASSIGNED_CAMPAIGN_ID);
    assert_eq!(catalog.reports[0].report_kind, "benchmarks");
    assert_eq!(catalog.reports[0].target, "/tmp/reports/fleet-latency.html");
}

#[test]
fn active_cockpit_report_fixture_loads_without_external_assets() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/observatory/manifest-v2.json");
    let catalog = Catalog::load(&path).unwrap();
    assert_eq!(catalog.version, CURRENT_MANIFEST_VERSION);
    assert_eq!(catalog.reports.len(), 3);
    assert_eq!(catalog.campaigns.len(), 2);
    assert!(catalog.campaign("visual-verifier").is_some());
    assert!(catalog.campaign(UNASSIGNED_CAMPAIGN_ID).is_some());
}

#[test]
fn version_two_loads_campaign_evidence_and_linked_runs() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/observatory/manifest-v2.json");
    let catalog = Catalog::from_str(&path, VERSION_TWO).unwrap();
    assert_eq!(catalog.version, CURRENT_MANIFEST_VERSION);
    assert_eq!(catalog.campaigns.len(), 2);
    let campaign = catalog.campaign("visual-verifier").unwrap();
    assert_eq!(campaign.title, "Production Visual Verifier");
    assert_eq!(campaign.status, "active");
    assert_eq!(campaign.report_count, 2);

    let report = catalog.report("visual-verifier-acceptance").unwrap();
    assert_eq!(report.report_kind, "acceptance");
    assert!(
        report
            .evidence_path
            .as_deref()
            .unwrap()
            .ends_with("evidence/visual-verifier-frame.json")
    );
    assert_eq!(
        report.linked_run_ids,
        ["cockpit-ui-verify-observatory-same-frame"]
    );
    assert!(matches!(report.media_link(), Media::Link { .. }));

    let legacy = catalog.report("legacy-routing-notes").unwrap();
    assert_eq!(legacy.campaign_title, UNASSIGNED_CAMPAIGN_TITLE);
    assert_eq!(legacy.status, "unassigned");
}

#[test]
fn snake_case_aliases_load_and_partial_campaigns_stay_unassigned() {
    let catalog = Catalog::from_str(
        Path::new("/tmp/reports/manifest.json"),
        r#"{
              "version": 2,
              "reports": [
                {
                  "slug": "snake",
                  "title": "Snake Case",
                  "file": "snake.html",
                  "campaign_id": "campaign-a",
                  "campaign_title": "Campaign A",
                  "status": "verified",
                  "report_kind": "evidence",
                  "evidence_path": "evidence/snake.json",
                  "linked_run_ids": ["run-snake-001"]
                },
                {
                  "slug": "partial",
                  "title": "Partial Campaign",
                  "file": "partial.html",
                  "campaignId": "missing-title"
                }
              ]
            }"#,
    )
    .unwrap();
    let snake = catalog.report("snake").unwrap();
    assert_eq!(snake.campaign_id, "campaign-a");
    assert_eq!(
        snake.evidence_path.as_deref(),
        Some("/tmp/reports/evidence/snake.json")
    );
    assert_eq!(snake.linked_run_ids, ["run-snake-001"]);
    let partial = catalog.report("partial").unwrap();
    assert_eq!(partial.campaign_id, UNASSIGNED_CAMPAIGN_ID);
    assert_eq!(partial.campaign_title, UNASSIGNED_CAMPAIGN_TITLE);
}

#[test]
fn unsupported_versions_and_duplicate_ids_are_rejected() {
    let error =
        Catalog::from_str(Path::new("manifest.json"), r#"{"version":3,"reports":[]}"#).unwrap_err();
    assert!(error.to_string().contains("unsupported catalog version 3"));

    let duplicate = LEGACY.replace(
        "]\n    }",
        ", {\"slug\":\"FLEET-LATENCY\",\"title\":\"Again\",\"file\":\"again.html\"}]\n    }",
    );
    let error = Catalog::from_str(Path::new("manifest.json"), &duplicate).unwrap_err();
    assert!(error.to_string().contains("duplicate report id"));
}

#[test]
fn renderer_keeps_campaign_rail_and_evidence_ledger_in_one_native_frame() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/observatory/manifest-v2.json");
    let catalog = Catalog::from_str(&path, VERSION_TWO).unwrap();
    let mut state = ObservatoryState::with_catalog(catalog);
    state.select_campaign("visual-verifier").unwrap();

    let backend = TestBackend::new(72, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            let outer = crate::hud::hud_block(state.title());
            let inner = outer.inner(area);
            frame.render_widget(outer, area);
            render(frame, &mut state, inner, 1.7);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let area = *buffer.area();
    let text = (area.y..area.y + area.height)
        .map(|y| {
            (area.x..area.x + area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|cell| cell.symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("CAMPAIGNS"), "{text}");
    assert!(text.contains("REPORT LEDGER"), "{text}");
    assert!(text.contains("visual-verifier-acceptance"), "{text}");
    assert!(text.contains("visual-verifier-frame.json"), "{text}");
    assert!(!text.contains("legacy-routing-notes"), "{text}");
}

#[test]
fn long_catalog_windows_keep_selected_rows_visible_and_semantics_exact() {
    let raw = long_manifest(12, 8);
    let catalog = Catalog::from_str(Path::new("/tmp/reports/manifest.json"), &raw).unwrap();
    let mut state = ObservatoryState::with_catalog(catalog);
    state.select_campaign("campaign-11").unwrap();
    state.focus_reports();
    for _ in 0..7 {
        state.move_focused(1);
    }

    let backend = TestBackend::new(48, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render(frame, &mut state, frame.area(), 1.7))
        .unwrap();

    let text = buffer_text(&terminal);
    let semantics = state.semantic_state(true);
    assert_eq!(semantics["selected_campaign"]["id"], "campaign-11");
    assert_eq!(semantics["selected_report"], "report-11-07");
    assert_eq!(semantics["focus"], "reports");
    assert_eq!(
        semantics["filtered_report_ids"].as_array().unwrap().len(),
        8
    );
    assert!(
        semantics["visible_campaign_ids"]
            .as_array()
            .unwrap()
            .contains(&json!("campaign-11"))
    );
    let visible = semantics["visible_report_ids"].as_array().unwrap();
    assert!(visible.len() < 8, "compact viewport must window reports");
    assert!(visible.contains(&json!("report-11-07")));
    for report in semantics["filtered_report_ids"].as_array().unwrap() {
        let id = report.as_str().unwrap();
        assert_eq!(
            visible.contains(report),
            text.contains(id),
            "visible semantics disagree with rendered identity row {id}\n{text}"
        );
    }
    assert!(text.contains('↑'), "late windows need a range cue\n{text}");
}

#[test]
fn empty_composer_navigates_gallery_but_typed_commands_keep_route_and_cursor() {
    let raw = long_manifest(3, 10);
    let catalog = Catalog::from_str(Path::new("/tmp/reports/manifest.json"), &raw).unwrap();
    let mut app = crate::App::preview(crate::Viewer::new());
    app.observatory = ObservatoryState::with_catalog(catalog);
    app.focus_module("artifacts");
    app.scryglass.surface = crate::scryglass::StageSurface::Observatory;
    let route = (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode());

    app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    for _ in 0..9 {
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    let semantics = app.observatory.semantic_state(true);
    assert_eq!(semantics["selected_campaign"]["id"], "campaign-00");
    assert_eq!(semantics["selected_report"], "report-00-09");
    assert_eq!(semantics["focus"], "reports");
    assert_eq!(
        route,
        (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode()),
        "gallery arrows must not mutate the selected route"
    );

    app.input = "/observatory campaign campaign-02".to_string();
    app.cursor = app.input.chars().count();
    let end = app.cursor;
    app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.cursor, end - 1);
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.cursor, end);
    assert_eq!(
        app.observatory.semantic_state(true)["selected_report"],
        "report-00-09"
    );
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.input.is_empty());
    assert_eq!(
        app.observatory.semantic_state(true)["selected_campaign"]["id"],
        "campaign-02"
    );
    assert_eq!(
        route,
        (app.bag.in_hand_label().to_string(), app.bag.in_hand_mode()),
        "composer navigation and submission must preserve the route"
    );
}

#[test]
fn selected_report_helper_materializes_media_without_launching_it() {
    let raw = long_manifest(1, 3);
    let catalog = Catalog::from_str(Path::new("/tmp/reports/manifest.json"), &raw).unwrap();
    let mut state = ObservatoryState::with_catalog(catalog);
    state.select_campaign("campaign-00").unwrap();
    state.focus_reports();
    state.move_focused(1);

    let (title, media) = state.selected_report_media().unwrap();
    assert_eq!(title, "Report 00 01");
    assert!(matches!(
        media,
        Media::Link { label, url }
            if label == "Report 00 01" && url.ends_with("reports/report-00-01.html")
    ));
    assert_eq!(
        state.semantic_state(true)["selected_report"],
        "report-00-01"
    );
    assert_eq!(state.focus(), ObservatoryFocus::Reports);
}

#[test]
fn focused_observatory_keeps_printable_keys_in_the_composer_and_escape_closes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/observatory/manifest-v2.json");
    let catalog = Catalog::from_str(&path, VERSION_TWO).unwrap();
    let mut app = crate::App::preview(crate::Viewer::new());
    app.observatory = ObservatoryState::with_catalog(catalog);
    app.focus_module("artifacts");
    app.scryglass
        .navigate(crate::scryglass::StageRoute::Observatory);
    app.scryglass.surface = crate::scryglass::StageSurface::Observatory;

    for character in "mpvrhjkw".chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    assert_eq!(app.input, "mpvrhjkw");

    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(
        app.scryglass.controller.route(),
        crate::scryglass::StageRoute::Realm
    );
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        app.module_host
            .focused()
            .is_some_and(|id| id.as_str() == "core")
    );
}
