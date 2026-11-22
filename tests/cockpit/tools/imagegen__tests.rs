use super::*;
use crate::agent::club::{ChatMsg, Club, ClubReply, ToolCall};
use crate::agent::harness::{TurnEvent, run_turn_observed};
use std::os::unix::fs::PermissionsExt;

const THREAD: &str = "a23a0000-b000-c000-d000-aa0000000000";

#[test]
fn metered_driver_can_generate_and_present_a_workspace_png() {
    let _lock = crate::tests::env_lock();
    let _allowlist = crate::tests::TestEnvGuard::unset("ANGEL_TOOLS");
    let _schema = crate::tests::TestEnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let scratch = Scratch::new().unwrap();
    let workspace = scratch.0.join("workspace");
    let generated_root = scratch.0.join("generated_images");
    let receipt_dir = generated_root.join(THREAD);
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&receipt_dir).unwrap();

    let executable = scratch.0.join("codex");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{}' '{}'\n",
            json!({"type": "thread.started", "thread_id": THREAD}),
            json!({"type": "turn.completed"}),
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let receipt = receipt_dir.join("generated.png");
    image::RgbImage::new(1, 1).save(&receipt).unwrap();

    let tool = ImageGen {
        executable,
        workspace: workspace.clone(),
        generated_root,
    };
    let args = json!({"prompt": "A black square", "path": "art/square.png", "label": "Square"});
    let mut registry = ToolRegistry::new();
    registry.set_workspace(workspace.clone());
    registry.register(Box::new(tool));
    assert!(
        registry
            .defs_for_driver_turn(None, false, false, true)
            .iter()
            .any(|tool| tool.name == "imagegen"),
    );

    struct ImageThenAnswer(Value, AtomicBool);
    impl Club for ImageThenAnswer {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }

        fn label(&self) -> &str {
            "imagegen-test"
        }

        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.1.swap(true, Ordering::SeqCst) {
                return Ok(ClubReply::Text("Image ready.".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "generated-asset".into(),
                name: "imagegen".into(),
                args: self.0.clone(),
            }]))
        }
    }

    let (events, received) = mpsc::channel();
    run_turn_observed(
        &ImageThenAnswer(args.clone(), AtomicBool::new(false)),
        &registry,
        &mut vec![ChatMsg::user("Generate an image of a black square.")],
        &AtomicBool::new(false),
        Some(4),
        &events,
    )
    .unwrap();
    let target = workspace.join("art/square.png");
    assert_eq!(fs::read(&target).unwrap(), fs::read(receipt).unwrap());
    assert!(received.try_iter().any(|event| matches!(
        event,
        TurnEvent::Media { kind, label, url }
            if kind == "image" && label == "Square" && Path::new(&url) == target
    )));
    assert!(
        registry
            .dispatch("imagegen", &args)
            .unwrap_err()
            .contains("already exists")
    );
}

#[test]
fn a_completed_codex_turn_needs_a_real_png() {
    let scratch = Scratch::new().unwrap();
    let thread = scratch.0.join(THREAD);
    fs::create_dir(&thread).unwrap();
    assert!(read_generated_image(&scratch.0, THREAD).is_err());
    fs::write(thread.join("fake.png"), PNG_HEADER).unwrap();
    assert!(read_generated_image(&scratch.0, THREAD).is_err());
    assert!(read_generated_image(&scratch.0, "../wrong-thread").is_err());
}
