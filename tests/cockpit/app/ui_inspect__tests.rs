use super::*;
use crate::Viewer;
use crate::app::PendingApproval;
use crate::ui;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn still_inspector_verify_is_typed_display_only_and_retains_draft() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.input = "/model unfinished".into();
    let action: UiOperation =
        serde_json::from_value(json!({"op":"inspect_image", "action":"zoom_in"})).unwrap();
    assert!(
        apply_operation(&mut app, &action)
            .unwrap_err()
            .contains("no ready")
    );
    assert_eq!(app.input, "/model unfinished");
    for payload in [
        json!({"op":"inspect_image", "action":"open", "path":"/tmp/arbitrary"}),
        json!({"op":"inspect_image", "action":"fit", "path":"/tmp/arbitrary"}),
        json!({"op":"inspect_image", "action":{"zoom":1e300}}),
    ] {
        assert!(serde_json::from_value::<UiOperation>(payload).is_err());
    }
    assert_eq!(
        semantic_state(&app)["scryglass"]["image_inspector"]["viewport"],
        Value::Null
    );
    app.media.push(crate::ui::media::Media::Image {
        label: "typed inspection fixture".into(),
        path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/agents/apollo-neutral.png")
            .display()
            .to_string(),
    });
    app.scryglass.reveal_media(0, true);
    app.focus_module("artifacts");
    let mut terminal = Terminal::new(TestBackend::new(144, 48)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.viewer.inspector.viewport.is_none() && Instant::now() < deadline {
        terminal.draw(|f| crate::ui::draw::ui(f, &mut app)).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(app.viewer.inspector.viewport.is_some());
    assert!(apply_operation(&mut app, &action).is_ok());
    assert_eq!(app.input, "/model unfinished");
    assert_eq!(
        semantic_state(&app)["scryglass"]["image_inspector"]["zoom_percent"],
        125
    );
    assert_eq!(
        semantic_state(&app)["scryglass"]["image_inspector"]["loading"],
        true
    );
}

#[test]
fn semantic_state_exposes_trusted_approval_scope() {
    let mut app = App::preview(Viewer::static_preview());
    let (reply, _decision) = std::sync::mpsc::channel();
    app.pending_approval = Some(PendingApproval {
        prompt: "free-form explanation".to_string(),
        scope_label: Some("remote host · spark".to_string()),
        reply,
    });

    let state = semantic_state(&app);
    assert_eq!(state["focus"]["approval"], true);
    assert_eq!(state["focus"]["approval_scope"], "remote host · spark");
}

#[test]
fn semantic_state_exposes_atlas_lane_trust_and_review_health() {
    let root = std::env::temp_dir().join(format!("angel-atlas-inspect-{}", std::process::id()));
    let workspace = root.join("workspace");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&workspace).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.atlas = crate::knowledge::atlas::AtlasService::open_in(&workspace, root.join("store"));
    let item = app
        .atlas
        .propose(
            crate::knowledge::atlas::AtlasKind::Fact,
            "inspection proposal",
            Some(0.7),
            vec![crate::knowledge::atlas::AtlasSource {
                id: "inspect:source".to_string(),
                kind: "test".to_string(),
                digest: "digest".to_string(),
                excerpt: None,
                independent: true,
                influenced_by: None,
            }],
        )
        .unwrap();
    app.atlas_view
        .set_lane(crate::knowledge::atlas::AtlasLane::Review);
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Vault);
    let state = semantic_state(&app);
    assert_eq!(state["atlas"]["open"], true);
    assert_eq!(state["atlas"]["lane"], "review");
    assert_eq!(state["atlas"]["review_count"], 1);
    assert_eq!(state["atlas"]["selected"]["id"], item.id);
    assert_eq!(state["atlas"]["selected"]["lifecycle"], "proposed");
    assert_eq!(state["atlas"]["selected"]["injection"], "never");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn observatory_open_semantics_follow_stage_route_after_raytrace_returns() {
    let mut app = App::preview(Viewer::static_preview());
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Observatory);
    app.observatory.clear_viewport();
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Raytrace);

    assert_eq!(semantic_state(&app)["observatory"]["open"], false);
    assert!(
        app.scryglass
            .controller
            .leave_route(crate::ui::scryglass::StageRoute::Raytrace)
    );
    assert_eq!(
        app.scryglass.controller.route(),
        crate::ui::scryglass::StageRoute::Observatory
    );
    assert_eq!(semantic_state(&app)["observatory"]["open"], true);
}

#[test]
fn world_mode_semantics_follow_unwound_explore_route() {
    let mut app = App::preview(Viewer::static_preview());
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Explore(
            crate::stage::world_viz::Building::Smithy,
        ));
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Observatory);
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Realm);
    assert_eq!(semantic_state(&app)["scryglass"]["world_mode"], "Map");

    assert!(app.scryglass.controller.back());
    assert!(app.scryglass.controller.back());
    let state = semantic_state(&app);
    assert_eq!(state["scryglass"]["route"], "Explore(Smithy)");
    assert_eq!(state["scryglass"]["world_mode"], "FirstPerson");
}

#[test]
fn teaching_and_interior_semantics_name_the_exact_local_state() {
    let mut app = App::preview(Viewer::static_preview());
    assert!(app.scryglass.set_ready_lesson(
        "linear algebra",
        "Linear algebra",
        "Vectors and linear maps."
    ));
    let lesson = semantic_state(&app);
    assert_eq!(
        lesson["scryglass"]["teaching"]["lesson"]["topic"],
        "linear algebra"
    );
    assert_eq!(
        lesson["scryglass"]["teaching"]["lesson"]["source_url"],
        "https://github.com/mitmath/1806"
    );

    assert!(app.scryglass.back_overlay());
    assert!(app.scryglass.open_catalog());
    app.scryglass.move_catalog_selection(2);
    let catalog = semantic_state(&app);
    assert_eq!(catalog["scryglass"]["teaching"]["catalog"]["selection"], 2);
    assert_eq!(
        catalog["scryglass"]["teaching"]["catalog"]["shelf_id"],
        "probability-openstax"
    );

    app.world
        .settle_at_for_test(crate::stage::world_viz::Building::Scriptorium);
    assert!(app.world.enter_interior());
    let interior = semantic_state(&app);
    assert_eq!(interior["world"]["inside_interior"], true);
    assert_eq!(interior["world"]["interior"], "Scriptorium");
}

#[test]
fn requested_frame_contains_exact_cells_and_matching_stage_state() {
    let broker = UiSnapshotBroker::new();
    let worker_broker = Arc::clone(&broker);
    let worker = std::thread::spawn(move || {
        UiInspectTool {
            broker: worker_broker,
        }
        .call(&json!({
            "scope": "scryglass",
            "format": "cells",
            "row_count": 8
        }))
    });
    for _ in 0..100 {
        if broker.has_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut app = App::preview(Viewer::static_preview());
    let _ = app
        .module_host
        .focus(&crate::platform::runtime::ModuleId::new("artifacts"));
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            let prepared =
                prepare_next_capture(&mut app, &broker).expect("inspection request pending");
            ui(frame, &mut app);
            capture_after_draw(&app, frame, &broker, prepared);
        })
        .unwrap();

    let value: Value = serde_json::from_str(&worker.join().unwrap().unwrap()).unwrap();
    assert_eq!(value["scope"], "scryglass");
    assert_eq!(value["state"]["focus"]["module"], "artifacts");
    assert!(value["state"]["scryglass"]["visible"].as_bool().unwrap());
    let rows = value["frame"].as_array().expect("exact cell rows");
    assert!(!rows.is_empty());
    assert!(rows[0][0].get("symbol").is_some());
}

#[test]
fn omitted_stage_frame_reports_hidden_surface() {
    let mut app = App::preview(Viewer::static_preview());
    let mut standard = Terminal::new(TestBackend::new(120, 40)).unwrap();
    standard.draw(|frame| ui(frame, &mut app)).unwrap();
    let visible = semantic_state(&app);
    assert_eq!(visible["scryglass"]["surface"], "WorldMap");
    assert_eq!(visible["scryglass"]["visible"], true);

    let mut compact = Terminal::new(TestBackend::new(80, 24)).unwrap();
    compact.draw(|frame| ui(frame, &mut app)).unwrap();
    let hidden = semantic_state(&app);
    assert_eq!(hidden["scryglass"]["surface"], "Hidden");
    assert_eq!(hidden["scryglass"]["visible"], false);
}

fn cell_value(cell: &Cell) -> Value {
    let cell = InspectCell::from(cell);
    json!({
        "symbol": cell.symbol,
        "fg": cell.fg,
        "bg": cell.bg,
        "underline": cell.underline,
        "modifier_bits": cell.modifier_bits,
        "modifiers": cell.modifiers,
        "skip": cell.skip
    })
}

#[test]
fn typed_control_operation_cells_and_semantics_share_one_cached_frame() {
    let broker = UiSnapshotBroker::new();
    let worker_broker = Arc::clone(&broker);
    let worker = std::thread::spawn(move || {
        UiVerifyTool {
            broker: worker_broker,
        }
        .call(&json!({
            "operation": { "op": "open_control", "control": "model" },
            "format": "cells"
        }))
    });
    for _ in 0..100 {
        if broker.has_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let mut app = App::preview(Viewer::static_preview());
    app.bag = crate::agent::club::Bag::for_render_test(&[(
        "alpha",
        &[("model-a", true), ("model-b", true)],
    )]);
    app.module_host
        .focus(&crate::platform::runtime::ModuleId::new("core"))
        .unwrap();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            let prepared =
                prepare_next_capture(&mut app, &broker).expect("verification request pending");
            ui(frame, &mut app);
            capture_after_draw(&app, frame, &broker, prepared);
        })
        .unwrap();

    let first: Value = serde_json::from_str(&worker.join().unwrap().unwrap()).unwrap();
    assert_eq!(first["version"], 2);
    assert_eq!(first["operation"]["status"], "applied");
    assert_eq!(first["operation"]["requested"]["op"], "open_control");
    assert_eq!(
        first["operation"]["before_state"]["focus"]["module"],
        "core"
    );
    assert_eq!(first["state"]["focus"]["agent_menu"]["kind"], "model");

    let snapshot_id = first["snapshot_id"].as_u64().unwrap();
    let inspect = UiInspectTool {
        broker: Arc::clone(&broker),
    };
    let second: Value = serde_json::from_str(
        &inspect
            .call(&json!({
                "snapshot_id": snapshot_id,
                "scope": "screen",
                "format": "cells",
                "row_offset": 17,
                "row_count": 17
            }))
            .unwrap(),
    )
    .unwrap();
    let third: Value = serde_json::from_str(
        &inspect
            .call(&json!({
                "snapshot_id": snapshot_id,
                "scope": "screen",
                "format": "cells",
                "row_offset": 34,
                "row_count": 6
            }))
            .unwrap(),
    )
    .unwrap();

    assert!(
        !broker.has_pending(),
        "cached paging must not request a draw"
    );
    for page in [&second, &third] {
        assert_eq!(page["snapshot_id"], snapshot_id);
        assert_eq!(page["captured_ms"], first["captured_ms"]);
        assert_eq!(page["operation"], first["operation"]);
        assert_eq!(page["state"], first["state"]);
    }

    let mut exported = Vec::new();
    for page in [&first, &second, &third] {
        exported.extend(page["frame"].as_array().unwrap().iter().cloned());
    }
    assert_eq!(exported.len(), 40);
    let buffer = terminal.backend().buffer();
    for (y, row) in exported.iter().enumerate() {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 120);
        for (x, actual) in row.iter().enumerate() {
            let expected = buffer.cell((x as u16, y as u16)).unwrap();
            assert_eq!(*actual, cell_value(expected), "cell mismatch at ({x}, {y})");
        }
    }
    let visible_text = exported
        .iter()
        .flat_map(|row| row.as_array().unwrap())
        .filter_map(|cell| cell["symbol"].as_str())
        .collect::<String>();
    assert!(
        visible_text.contains("Brain Route · MODEL"),
        "{visible_text}"
    );
}

#[test]
fn typed_observatory_operation_cells_and_campaign_semantics_share_one_frame() {
    let broker = UiSnapshotBroker::new();
    let worker_broker = Arc::clone(&broker);
    let worker = std::thread::spawn(move || {
        UiVerifyTool {
            broker: worker_broker,
        }
        .call(&json!({
            "operation": {
                "op": "open_observatory",
                "campaign": "visual-verifier"
            },
            "format": "cells"
        }))
    });
    for _ in 0..100 {
        if broker.has_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let manifest = include_str!("../../../cockpit/fixtures/observatory/manifest-v2.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/observatory/manifest-v2.json");
    let mut app = App::preview(Viewer::static_preview());
    app.observatory =
        crate::app::observatory::ObservatoryState::from_manifest_for_test(&path, manifest).unwrap();
    app.observatory.clear_viewport();

    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| {
            let prepared =
                prepare_next_capture(&mut app, &broker).expect("verification request pending");
            ui(frame, &mut app);
            capture_after_draw(&app, frame, &broker, prepared);
        })
        .unwrap();

    let first: Value = serde_json::from_str(&worker.join().unwrap().unwrap()).unwrap();
    assert_eq!(first["operation"]["status"], "applied");
    assert_eq!(first["operation"]["requested"]["op"], "open_observatory");
    assert_eq!(
        first["operation"]["before_state"]["observatory"]["open"],
        false
    );
    assert_eq!(first["state"]["focus"]["module"], "artifacts");
    assert_eq!(first["state"]["scryglass"]["surface"], "Observatory");
    assert_eq!(first["state"]["observatory"]["open"], true);
    assert_eq!(
        first["state"]["observatory"]["selected_campaign"]["id"],
        "visual-verifier"
    );
    assert_eq!(
        first["state"]["observatory"]["visible_report_ids"],
        json!(["visual-verifier-acceptance", "visual-verifier-cell-diff"])
    );

    let snapshot_id = first["snapshot_id"].as_u64().unwrap();
    let second: Value = serde_json::from_str(
        &UiInspectTool {
            broker: Arc::clone(&broker),
        }
        .call(&json!({
            "snapshot_id": snapshot_id,
            "scope": "screen",
            "format": "cells",
            "row_offset": 17,
            "row_count": 15
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second["snapshot_id"], snapshot_id);
    assert_eq!(second["captured_ms"], first["captured_ms"]);
    assert_eq!(second["operation"], first["operation"]);
    assert_eq!(second["state"], first["state"]);
    assert!(!broker.has_pending());

    let mut exported = Vec::new();
    for page in [&first, &second] {
        exported.extend(page["frame"].as_array().unwrap().iter().cloned());
    }
    assert_eq!(exported.len(), 32);
    let buffer = terminal.backend().buffer();
    for (y, row) in exported.iter().enumerate() {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 120);
        for (x, actual) in row.iter().enumerate() {
            let expected = buffer.cell((x as u16, y as u16)).unwrap();
            assert_eq!(*actual, cell_value(expected), "cell mismatch at ({x}, {y})");
        }
    }
    let visible_text = exported
        .iter()
        .flat_map(|row| row.as_array().unwrap())
        .filter_map(|cell| cell["symbol"].as_str())
        .collect::<String>();
    assert!(visible_text.contains("CAMPAIGNS"), "{visible_text}");
    assert!(visible_text.contains("REPORT LEDGER"), "{visible_text}");
}

#[test]
fn typed_late_campaign_is_windowed_into_the_same_exact_frame() {
    let broker = UiSnapshotBroker::new();
    let worker_broker = Arc::clone(&broker);
    let worker = std::thread::spawn(move || {
        UiVerifyTool {
            broker: worker_broker,
        }
        .call(&json!({
            "operation": {
                "op": "open_observatory",
                "campaign": "campaign-19"
            },
            "format": "cells"
        }))
    });
    for _ in 0..100 {
        if broker.has_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let reports = (0..20)
        .map(|index| {
            json!({
                "slug": format!("report-{index:02}"),
                "title": format!("Report {index:02}"),
                "file": format!("reports/report-{index:02}.html"),
                "campaignId": format!("campaign-{index:02}"),
                "campaignTitle": format!("Campaign {index:02}"),
                "status": "active",
                "reportKind": "evidence"
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({ "version": 2, "reports": reports }).to_string();
    let path = std::path::Path::new("/tmp/observatory-long/manifest.json");
    let mut app = App::preview(Viewer::static_preview());
    app.observatory =
        crate::app::observatory::ObservatoryState::from_manifest_for_test(path, &manifest).unwrap();
    app.observatory.clear_viewport();

    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| {
            let prepared =
                prepare_next_capture(&mut app, &broker).expect("verification request pending");
            ui(frame, &mut app);
            capture_after_draw(&app, frame, &broker, prepared);
        })
        .unwrap();

    let first: Value = serde_json::from_str(&worker.join().unwrap().unwrap()).unwrap();
    assert_eq!(first["operation"]["status"], "applied");
    assert_eq!(
        first["state"]["observatory"]["selected_campaign"]["id"],
        "campaign-19"
    );
    assert!(
        first["state"]["observatory"]["visible_campaign_ids"]
            .as_array()
            .unwrap()
            .contains(&json!("campaign-19"))
    );
    assert!(
        !first["state"]["observatory"]["visible_campaign_ids"]
            .as_array()
            .unwrap()
            .contains(&json!("campaign-00"))
    );
    assert_eq!(
        first["state"]["observatory"]["filtered_report_ids"],
        json!(["report-19"])
    );
    assert_eq!(
        first["state"]["observatory"]["visible_report_ids"],
        json!(["report-19"])
    );

    let snapshot_id = first["snapshot_id"].as_u64().unwrap();
    let second: Value = serde_json::from_str(
        &UiInspectTool {
            broker: Arc::clone(&broker),
        }
        .call(&json!({
            "snapshot_id": snapshot_id,
            "scope": "screen",
            "format": "cells",
            "row_offset": 17,
            "row_count": 15
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second["snapshot_id"], snapshot_id);
    assert_eq!(second["captured_ms"], first["captured_ms"]);
    assert_eq!(second["state"], first["state"]);

    let mut exported = Vec::new();
    for page in [&first, &second] {
        exported.extend(page["frame"].as_array().unwrap().iter().cloned());
    }
    assert_eq!(exported.len(), 32);
    let buffer = terminal.backend().buffer();
    for (y, row) in exported.iter().enumerate() {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 120);
        for (x, actual) in row.iter().enumerate() {
            let expected = buffer.cell((x as u16, y as u16)).unwrap();
            assert_eq!(*actual, cell_value(expected), "cell mismatch at ({x}, {y})");
        }
    }
}
