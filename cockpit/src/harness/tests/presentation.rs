//! The model's requested presentation must reach Stage only after a real success.

use super::*;

#[test]
fn present_events_require_success_and_use_the_normalized_artifact() {
    let _guard = crate::tests::env_lock();
    let _advisor = EnvGuard::set("ANGEL_ADVISOR", "0");
    let _accept = EnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("present_dispatch");
    std::fs::write(root.join("report.md"), "# Actual work\nResult: passed.\n").unwrap();
    struct PresentThenAnswer {
        kind: &'static str,
        hops: AtomicUsize,
    }
    impl Club for PresentThenAnswer {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "presentation-dispatch-fixture"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::SeqCst) != 0 {
                return Ok(ClubReply::Text("Presentation request handled.".into()));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "show-work".into(),
                name: "present".into(),
                args: serde_json::json!({
                    "kind": self.kind,
                    "label": "Actual result report",
                    "url": "report.md",
                }),
            }]))
        }
    }
    for kind in ["unknown", "report"] {
        let registry = ToolRegistry::with_team(root.clone(), Vec::new());
        let club = PresentThenAnswer {
            kind,
            hops: AtomicUsize::new(0),
        };
        let (events, received) = mpsc::channel();
        run_turn_observed(
            &club,
            &registry,
            &mut vec![ChatMsg::user("Show the actual result report.")],
            &AtomicBool::new(false),
            Some(4),
            &events,
        )
        .unwrap();
        let media: Vec<_> = received
            .try_iter()
            .filter_map(|event| match event {
                TurnEvent::Media { kind, label, url } => Some((kind, label, url)),
                _ => None,
            })
            .collect();
        if kind == "unknown" {
            assert!(media.is_empty(), "failed presentation emitted {media:?}");
        } else {
            assert_eq!(
                media,
                vec![(
                    "resource".into(),
                    "Actual result report".into(),
                    root.join("report.md").to_string_lossy().into_owned(),
                )]
            );
        }
    }
    let _ = std::fs::remove_dir_all(root);
}
