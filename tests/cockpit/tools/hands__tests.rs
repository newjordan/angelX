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
    assert!(!wants_present(&json!({"action": "screenshot", "present": false})));
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
    let off = "<tool_call>{\"arguments\":{\"action\":\"click\",\"coordinate\":[1200,40]}}</tool_call>";
    assert!(parse_ground_reply(off).unwrap_err().contains("off-screen"));
    let neg = "<tool_call>{\"arguments\":{\"action\":\"click\",\"coordinate\":[-1,40]}}</tool_call>";
    assert!(parse_ground_reply(neg).is_err());
    let three = "<tool_call>{\"arguments\":{\"action\":\"click\",\"coordinate\":[1,2,3]}}</tool_call>";
    assert!(parse_ground_reply(three).is_err());
}

#[test]
fn ground_maps_model_frame_to_layout_coords() {
    // 1080p output at the layout origin: the frame's corners are the pixel corners.
    assert_eq!(ground_to_layout((0.0, 0.0), (1920, 1080), (0, 0), 1.0), (0, 0));
    assert_eq!(ground_to_layout((999.0, 999.0), (1920, 1080), (0, 0), 1.0), (1919, 1079));
    assert_eq!(ground_to_layout((500.0, 500.0), (1920, 1080), (0, 0), 1.0), (960, 540));
    // A monitor right of another: its origin offsets the point.
    assert_eq!(ground_to_layout((500.0, 500.0), (3440, 1440), (1920, 0), 1.0), (1920 + 1721, 720));
    // HiDPI: the screenshot is in physical pixels, the layout in logical ones.
    assert_eq!(ground_to_layout((999.0, 999.0), (3840, 2160), (0, 0), 2.0), (1920, 1080));
    // A bogus scale never divides by zero.
    assert_eq!(ground_to_layout((500.0, 0.0), (1920, 1080), (0, 0), 0.0), (960, 0));
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
fn ground_frame_fit_keeps_the_answer_on_the_same_pixel() {
    use image::{Rgb, RgbImage};
    let shot = RgbImage::from_pixel(3840, 2160, Rgb([10, 20, 30]));
    let (frame, back) = fit_frame(&shot, GROUND_FRAME);
    assert_eq!(frame.dimensions(), (1920, 1080));
    assert_eq!(back, (2.0, 2.0));
    // A capped-frame point comes back in the original screenshot's pixels.
    assert_eq!(uncap_point((960.0, 540.0), back), (1920.0, 1080.0));
    // At the reference size the frame is borrowed unchanged and maps 1:1.
    let small = RgbImage::from_pixel(1920, 1080, Rgb([1, 2, 3]));
    let (frame, back) = fit_frame(&small, GROUND_FRAME);
    assert_eq!(back, (1.0, 1.0));
    assert!(matches!(frame, std::borrow::Cow::Borrowed(_)));
    // Ultrawide is squashed onto the frame: each axis gets its own factor, so
    // a 3440x1440 screen is 0.558x wide and 0.75x tall in the model's frame.
    let wide = RgbImage::from_pixel(3440, 1440, Rgb([0, 0, 0]));
    let (frame, back) = fit_frame(&wide, GROUND_FRAME);
    assert_eq!(frame.dimensions(), (1920, 1080));
    assert!((back.0 - 3440.0 / 1920.0).abs() < 1e-9 && (back.1 - 1440.0 / 1080.0).abs() < 1e-9);
    // A smaller shot is upscaled onto the same frame, not left alone: a
    // 1280x720 frame grounds 7/32 closed-loop, the same frame fitted 27/32.
    let laptop = RgbImage::from_pixel(1280, 720, Rgb([9, 9, 9]));
    let (frame, back) = fit_frame(&laptop, GROUND_FRAME);
    assert_eq!(frame.dimensions(), (1920, 1080));
    assert_eq!(back, (1280.0 / 1920.0, 720.0 / 1080.0));
    assert_eq!(uncap_point((960.0, 540.0), back), (640.0, 360.0));
    // 4:3 is stretched, not letterboxed: inside the box it actuated 6/32,
    // stretched onto it 24/32.
    let four_three = RgbImage::from_pixel(1024, 768, Rgb([4, 4, 4]));
    let (frame, back) = fit_frame(&four_three, GROUND_FRAME);
    assert_eq!(frame.dimensions(), (1920, 1080));
    assert!((back.0 - 1024.0 / 1920.0).abs() < 1e-9 && (back.1 - 768.0 / 1080.0).abs() < 1e-9);
    // Portrait is the squeezed case: a 1080x1920 screen is 1.78x wide and
    // 1.78x *compressed* in y once fitted. Native it actuated 13/30 with an
    // x drift of -6.13% of the frame; fitted 24/30, drift +0.02%.
    let portrait = RgbImage::from_pixel(1080, 1920, Rgb([7, 7, 7]));
    let (frame, back) = fit_frame(&portrait, GROUND_FRAME);
    assert_eq!(frame.dimensions(), (1920, 1080));
    assert!((back.0 - 1080.0 / 1920.0).abs() < 1e-9 && (back.1 - 1920.0 / 1080.0).abs() < 1e-9);
    assert_eq!(uncap_point((960.0, 540.0), back), (540.0, 960.0));
    // Roughly the right place after the round trip (< 2 px on a 4K shot).
    let on_target = uncap_point((float(500) / 999.0 * 1919.0, float(500) / 999.0 * 1079.0), (2.0, 2.0));
    assert!((on_target.0 - 1921.0).abs() < 3.0 && (on_target.1 - 1080.0).abs() < 3.0);
}

fn float(v: u32) -> f64 {
    f64::from(v)
}

#[test]
fn ground_zoom_windows_stay_inside_the_frame() {
    // Centred on the point when there is room.
    assert_eq!(zoom_window((960.0, 540.0), (1920, 1080), 2), (480, 270, 960, 540));
    assert_eq!(zoom_window((960.0, 540.0), (1920, 1080), 3), (640, 360, 640, 360));
    // Pushed back inside at every edge.
    assert_eq!(zoom_window((10.0, 5.0), (1920, 1080), 2), (0, 0, 960, 540));
    assert_eq!(zoom_window((1915.0, 1079.0), (1920, 1080), 3), (1280, 720, 640, 360));
    // The cascade is the probe-picked fixed pair.
    assert_eq!(GROUND_ZOOMS, [2, 3]);
    // Frame point → pixels → layout composes to the one-step mapping.
    let px = frame_to_px((500.0, 500.0), (1920, 1080));
    assert_eq!(px_to_layout(px, (1920, 0), 1.0), ground_to_layout((500.0, 500.0), (1920, 1080), (1920, 0), 1.0));
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

#[test]
fn frame_diff_sees_one_pixel_and_ignores_jitter() {
    use image::{Rgb, RgbImage};
    let blank = || RgbImage::from_pixel(20, 10, Rgb([200u8, 200, 200]));
    let a = blank();
    assert_eq!(frame_changed_px(&a, &blank(), CLICK_PIXEL_LEVEL), 0);
    let mut b = blank();
    b.put_pixel(3, 4, Rgb([10, 200, 200])); // one channel, far past the level
    assert_eq!(frame_changed_px(&a, &b, CLICK_PIXEL_LEVEL), 1);
    let mut c = blank();
    c.put_pixel(3, 4, Rgb([205, 200, 200])); // re-encode jitter, under the level
    assert_eq!(frame_changed_px(&a, &c, CLICK_PIXEL_LEVEL), 0);
    // a different output size is a different frame, not a diff
    assert_eq!(frame_changed_px(&a, &RgbImage::from_pixel(10, 10, Rgb([0, 0, 0])), 10), 200);
}

#[test]
fn click_verdict_reports_the_frame_never_a_success() {
    let idle = click_verdict(0);
    assert!(idle.contains("did NOT change"), "{idle}");
    assert!(idle.contains("instead of repeating the point"), "{idle}");
    assert!(idle.contains("(0 px)"), "reports the count it saw: {idle}");
    let busy = click_verdict(CLICK_CHANGED_PX * 4);
    assert!(busy.contains("did something"), "{busy}");
    assert!(busy.contains(&format!("({} px)", CLICK_CHANGED_PX * 4)), "{busy}");
    // measured: a changed frame followed clicks that missed the target, so the
    // wording must never read as "the target was reached"
    for verdict in [idle, busy] {
        let low = verdict.to_lowercase();
        for claim in ["succeeded", "success", "worked", "clicked the"] {
            assert!(!low.contains(claim), "{claim} claimed in {verdict}");
        }
    }
    assert!(click_verdict(CLICK_CHANGED_PX).contains("did NOT change"));
    assert!(click_verdict(CLICK_CHANGED_PX + 1).contains("did something"));
}

#[test]
fn type_verdict_uses_its_own_floor_and_never_claims_the_field() {
    // measured 2026-11-02: one character moves 62-103 px, a lost type moves 0
    let lost = type_verdict(0, 7);
    assert!(lost.contains("did NOT change"), "{lost}");
    assert!(lost.contains("(0 px)") && lost.contains("7 characters"), "{lost}");
    // the measured failure mode is silent, so the verdict names its cause
    assert!(lost.contains("read-only") && lost.contains("no editable field"), "{lost}");
    let one = type_verdict(62, 1);
    assert!(one.contains("did change") || one.contains("changed"), "{one}");
    assert!(one.contains("(62 px)"), "{one}");
    // the click floor would have called a one-character type a failure, so the
    // type floor sits under the smallest measured real change (62 px)
    assert!(TYPE_CHANGED_PX < 62, "type floor sits under the smallest real change");
    assert!(TYPE_CHANGED_PX < CLICK_CHANGED_PX);
    // a change is still not proof of the field
    for verdict in [one, type_verdict(TYPE_CHANGED_PX * 20, 9)] {
        let low = verdict.to_lowercase();
        for claim in ["succeeded", "success", "worked", "typed into the"] {
            assert!(!low.contains(claim), "{claim} claimed in {verdict}");
        }
        assert!(low.contains("not proof"), "{verdict}");
    }
    assert!(type_verdict(TYPE_CHANGED_PX, 3).contains("did NOT change"));
    assert!(type_verdict(TYPE_CHANGED_PX + 1, 3).contains("changed ("));
}
