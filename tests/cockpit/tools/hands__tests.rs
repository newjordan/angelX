use super::*;
use serde_json::json;

#[test]
fn observe_actions_need_no_capsule_acts_do() {
    for a in ["status", "windows", "screenshot", "cursor", "ground"] {
        assert!(!is_act("desktop", a), "desktop {a} must be observe");
    }
    for a in ["focus", "key", "type", "click", "launch"] {
        assert!(is_act("desktop", a), "desktop {a} must be an act");
    }
    for a in ["snapshot", "screenshot", "status"] {
        assert!(!is_act("browser", a), "browser {a} must be observe");
    }
    for a in ["open", "click", "type", "press", "eval", "close"] {
        assert!(is_act("browser", a), "browser {a} must be an act");
    }
    // Unknown actions fail closed: treated as acts so approval still applies.
    assert!(is_act("desktop", "frobnicate"));
    assert!(is_act("browser", "frobnicate"));
}

#[test]
fn lua_dispatch_quoting_and_verdicts() {
    assert_eq!(lua_str("address:0xabc"), "'address:0xabc'");
    assert_eq!(lua_str("a'b\\c\nd"), "'a\\'b\\\\c\\nd'");
    assert!(check_dispatch("ok").is_ok());
    assert!(check_dispatch("").is_ok());
    assert!(check_dispatch("error: [string \"x\"]:1: bad").is_err());
    assert!(check_dispatch("warning: =[C]:-1: Bad workspace").is_err());
    assert!(is_act("desktop", "agent_start"));
    assert!(is_act("desktop", "agent_stop"));
}

#[test]
fn agent_display_detection_and_click_correction() {
    let monitors = json!([
        {"id": 0, "name": "HDMI-A-1"},
        {"id": 3, "name": "HEADLESS-2"}
    ]);
    assert_eq!(
        agent_display_info(&monitors),
        Some(("HEADLESS-2".to_string(), 3))
    );
    assert_eq!(agent_display_info(&json!([])), None);
    assert_eq!(plan_correction((100, 100), (100, 102)), (0, 2, true));
    assert_eq!(plan_correction((100, 100), (40, 300)), (-60, 200, false));
}

#[test]
fn browser_binary_prefers_chrome_over_chromium() {
    assert_eq!(BROWSER_CANDIDATES[0], "google-chrome-stable");
    let chrome = BROWSER_CANDIDATES
        .iter()
        .position(|n| *n == "google-chrome")
        .unwrap();
    let chromium = BROWSER_CANDIDATES
        .iter()
        .position(|n| *n == "chromium")
        .unwrap();
    assert!(chrome < chromium);
}

#[test]
fn browser_choice_roundtrips_through_workspace_file() {
    let dir = std::env::temp_dir().join(format!("angel-hands-choice-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake_bin = dir.join("fake-chrome");
    std::fs::write(&fake_bin, b"#!/bin/sh\n").unwrap();
    assert_eq!(read_browser_choice(&dir), None);
    write_browser_choice(&dir, Some(&fake_bin)).unwrap();
    assert_eq!(read_browser_choice(&dir), Some(fake_bin.clone()));
    // A pick pointing at a deleted binary is ignored, never launched.
    std::fs::remove_file(&fake_bin).unwrap();
    assert_eq!(read_browser_choice(&dir), None);
    write_browser_choice(&dir, None).unwrap();
    assert!(!browser_choice_path(&dir).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn browser_urls_fail_closed() {
    assert!(validate_browser_url("https://example.com/x").is_ok());
    assert!(validate_browser_url("http://127.0.0.1:5173/").is_ok());
    assert!(validate_browser_url("about:blank").is_ok());
    assert!(validate_browser_url("file:///etc/passwd").is_err());
    assert!(validate_browser_url("javascript:alert(1)").is_err());
    assert!(validate_browser_url("example.com").is_err());
    assert!(validate_browser_url("").is_err());
}

#[test]
fn typed_text_and_keys_are_bounded() {
    assert!(validate_type_text("hello").is_ok());
    assert!(validate_type_text("").is_err());
    assert!(validate_type_text(&"x".repeat(MAX_TYPE_CHARS + 1)).is_err());
    assert!(validate_key("Return").is_ok());
    assert!(validate_key("ctrl+l").is_ok());
    assert!(validate_key("alt+Tab").is_ok());
    assert!(validate_key("").is_err());
    assert!(validate_key("a; rm -rf /").is_err());
    assert!(validate_key("$(reboot)").is_err());
}

#[test]
fn screenshot_names_cannot_escape() {
    assert_eq!(shot_stem(Some("Demo Shot 1")), "demo-shot-1");
    assert_eq!(shot_stem(Some("../../etc/passwd")), "etc-passwd");
    assert_eq!(shot_stem(None), "shot");
    assert_eq!(shot_stem(Some("///")), "shot");
}

#[test]
fn hypr_clients_format_compactly() {
    let v = json!([
        {"address":"0xabc","class":"chromium","title":"Docs","workspace":{"id":2},"at":[10,20],"size":[800,600],"pid":42},
        {"address":"0xdef","class":"foot","title":"","workspace":{"id":1},"at":[0,0],"size":[640,480],"pid":7}
    ]);
    let out = format_clients(&v);
    assert!(out.contains("0xabc"), "{out}");
    assert!(out.contains("chromium"), "{out}");
    assert!(out.contains("ws2"), "{out}");
    assert!(out.lines().count() == 2, "{out}");
    assert_eq!(format_clients(&json!([])), "no windows");
}

#[test]
fn screenshot_envelope_parses_as_stage_presentation() {
    let envelope = present_envelope(
        "desktop · demo",
        "/work/proj/.angel/hands/demo-1.png",
        "screenshot saved: /work/proj/.angel/hands/demo-1.png (42 bytes)",
    );
    let parsed = crate::ui::media::presentation_from_result(&envelope)
        .expect("turn loop must accept the hands envelope");
    assert_eq!(
        parsed,
        (
            "image".to_string(),
            "desktop · demo".to_string(),
            "/work/proj/.angel/hands/demo-1.png".to_string()
        )
    );
    // The model-facing receipt survives inside the envelope.
    assert!(envelope.contains("screenshot saved:"));
    assert!(wants_present(&json!({"action": "screenshot"})));
    assert!(!wants_present(
        &json!({"action": "screenshot", "present": false})
    ));
}

/// Live Omarchy proof: run with `ANGEL_HANDS_LIVE=1 cargo test hands_live -- --ignored`
/// on a Hyprland session. Observe-only; never acts on the operator's desktop.
#[test]
#[ignore = "live Hyprland session required"]
fn hands_live_desktop_observe() {
    let tool = DesktopTool {
        workspace: std::env::temp_dir(),
    };
    let status = tool.call(&json!({"action": "status"})).expect("status");
    assert!(status.contains("hyprctl=yes"), "{status}");
    let windows = tool.call(&json!({"action": "windows"})).expect("windows");
    assert!(!windows.is_empty());
    let shot = tool
        .call(&json!({"action": "screenshot", "name": "live-test"}))
        .expect("screenshot");
    assert!(shot.contains("screenshot saved:"), "{shot}");
}

#[test]
fn click_coords_must_be_on_screen() {
    assert!(validate_coords(0, 0).is_ok());
    assert!(validate_coords(1919, 1079).is_ok());
    assert!(validate_coords(-1, 10).is_err());
    assert!(validate_coords(100_000, 10).is_err());
}

#[test]
fn ground_is_observe_and_reads_the_models_point() {
    assert!(!is_act("desktop", "ground"));
    let reply = "Action: Click the Save button.\n\n<tool_call>\n\
        {\"name\": \"computer_use\", \"arguments\": {\"action\": \"click\", \"coordinate\": [722, 99], \"button\": \"left\"}}\n\
        </tool_call>\n\n<tool_call>\n{\"name\": \"computer_use\", \"arguments\": {\"action\": \"wait\", \"time\": 1.0}}\n</tool_call>";
    assert_eq!(parse_ground_reply(reply), Ok((722.0, 99.0)));
    // A garbled call is skipped; the next pointed call wins.
    let reply = "<tool_call>{not json</tool_call><tool_call>{\"name\":\"computer_use\",\
        \"arguments\":{\"action\":\"type\",\"text\":\"hi\"}}</tool_call>\
        <tool_call>{\"name\":\"computer_use\",\"arguments\":{\"action\":\"double_click\",\"coordinate\":[0,999]}}</tool_call>";
    assert_eq!(parse_ground_reply(reply), Ok((0.0, 999.0)));
}

#[test]
fn ground_fails_closed_on_garbage_or_off_screen() {
    assert!(parse_ground_reply("").is_err());
    assert!(parse_ground_reply("<think>\n<think>\n<think>").is_err());
    assert!(parse_ground_reply("I would click the Save button at 300, 40.").is_err());
    let off =
        "<tool_call>{\"arguments\":{\"action\":\"click\",\"coordinate\":[1200,40]}}</tool_call>";
    assert!(parse_ground_reply(off).unwrap_err().contains("off-screen"));
    let neg =
        "<tool_call>{\"arguments\":{\"action\":\"click\",\"coordinate\":[-1,40]}}</tool_call>";
    assert!(parse_ground_reply(neg).is_err());
    let three =
        "<tool_call>{\"arguments\":{\"action\":\"click\",\"coordinate\":[1,2,3]}}</tool_call>";
    assert!(parse_ground_reply(three).is_err());
}

#[test]
fn ground_maps_model_frame_to_layout_coords() {
    // 1080p output at the layout origin: the frame's corners are the pixel corners.
    assert_eq!(
        ground_to_layout((0.0, 0.0), (1920, 1080), (0, 0), 1.0),
        (0, 0)
    );
    assert_eq!(
        ground_to_layout((999.0, 999.0), (1920, 1080), (0, 0), 1.0),
        (1919, 1079)
    );
    assert_eq!(
        ground_to_layout((500.0, 500.0), (1920, 1080), (0, 0), 1.0),
        (960, 540)
    );
    // A monitor right of another: its origin offsets the point.
    assert_eq!(
        ground_to_layout((500.0, 500.0), (3440, 1440), (1920, 0), 1.0),
        (1920 + 1721, 720)
    );
    // HiDPI: the screenshot is in physical pixels, the layout in logical ones.
    assert_eq!(
        ground_to_layout((999.0, 999.0), (3840, 2160), (0, 0), 2.0),
        (1920, 1080)
    );
    // A bogus scale never divides by zero.
    assert_eq!(
        ground_to_layout((500.0, 0.0), (1920, 1080), (0, 0), 0.0),
        (960, 0)
    );
    let monitors = json!([
        {"name": "HDMI-A-1", "x": 0, "y": 0, "scale": 1.0},
        {"name": "DP-3", "x": 1920, "y": 0, "scale": 1.25}
    ]);
    assert_eq!(monitor_origin(&monitors, "DP-3"), Some((1920, 0, 1.25)));
    assert_eq!(monitor_origin(&monitors, "HEADLESS-9"), None);
}

#[test]
fn ground_turn_names_the_target_once() {
    let t = ground_user_text("Click the \"Save\" button.");
    assert!(t.contains("Instruction: Click the \"Save\" button. The target is visible"));
    assert!(ground_user_text("the Refresh icon").contains("Instruction: Click the Refresh icon."));
    assert!(!ground_user_text("click Tools menu").contains("Click click"));
}

#[test]
fn ground_zoom_windows_stay_inside_the_frame() {
    // Centred on the point when there is room.
    assert_eq!(
        zoom_window((960.0, 540.0), (1920, 1080), 2),
        (480, 270, 960, 540)
    );
    assert_eq!(
        zoom_window((960.0, 540.0), (1920, 1080), 3),
        (640, 360, 640, 360)
    );
    // Pushed back inside at every edge.
    assert_eq!(zoom_window((10.0, 5.0), (1920, 1080), 2), (0, 0, 960, 540));
    assert_eq!(
        zoom_window((1915.0, 1079.0), (1920, 1080), 3),
        (1280, 720, 640, 360)
    );
    // The cascade is the probe-picked fixed pair.
    assert_eq!(GROUND_ZOOMS, [2, 3]);
    // Frame point → pixels → layout composes to the one-step mapping.
    let px = frame_to_px((500.0, 500.0), (1920, 1080));
    assert_eq!(
        px_to_layout(px, (1920, 0), 1.0),
        ground_to_layout((500.0, 500.0), (1920, 1080), (1920, 0), 1.0)
    );
}

/// Live: `ground` end to end on whatever Hyprland the env points at (run it
/// on a private nested seat) with a grounding model up. Prints each target's
/// layout point; `ANGEL_GROUND_LIVE_TARGETS` is a `|`-separated list.
#[test]
#[ignore = "live Hyprland session and grounding model required"]
fn hands_live_ground() {
    let tool = DesktopTool {
        workspace: std::env::temp_dir(),
    };
    let targets = std::env::var("ANGEL_GROUND_LIVE_TARGETS").expect("ANGEL_GROUND_LIVE_TARGETS");
    for target in targets.split('|') {
        let out = tool
            .call(&json!({"action": "ground", "target": target}))
            .expect("ground");
        println!("LIVE {out}");
        assert!(out.contains(" is at ("), "{out}");
    }
}

#[test]
fn ground_prompt_is_the_models_own_template() {
    // chat_template.jinja for [system, user{image, text}], add_generation_prompt,
    // enable_thinking=false — pinned from the template's own render.
    let p = ground_prompt("the Bold button");
    let user = ground_user_text("the Bold button");
    assert_eq!(
        p,
        format!(
            "<|im_start|>system\n{GROUND_SYSTEM}<|im_end|>\n<|im_start|>user\n\
             <|vision_start|><|image_pad|><|vision_end|>{user}<|im_end|>\n\
             <|im_start|>assistant\n<think>\n\n</think>\n\n"
        )
    );
    assert_eq!(p.matches("<|image_pad|>").count(), 1);
    assert!(p.ends_with("<think>\n\n</think>\n\n"), "thinking stays off");
}
