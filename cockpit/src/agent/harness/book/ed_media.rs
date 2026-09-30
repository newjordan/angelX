//! ⠫ ed — Volume II, the tool library: the stage, video and vision, and the
//! records that carry a workspace forward. One section per tool; its pages are
//! the sentences that told the model how to use that tool, verbatim, moved off
//! the schema. The schema keeps what the tool does and ends with the section's
//! route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠫';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "media",
    surface: "media and the workspace record",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "present",
            signal: "using `present`",
            action: "",
            ideas: "",
            pages: &[
                "Use this when asked to show work, images, videos, or reports: image/video displays a local artifact; resource/report displays a local UTF-8 document (including Markdown, source, CSV, or JSON).",
                "Keep the artifact's actual path and a descriptive label; do not substitute example work.",
                "image/video = local terminal-native preview; resource/report = local UTF-8 document; link/graph = exact URL or local report",
                "Short label shown on the card.",
                "Actual local artifact path (relative to the active workspace or absolute), or an exact http(s) URL for a link.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "video_probe",
            signal: "using `video_probe`",
            action: "",
            ideas: "",
            pages: &[
                "Use before planning an edit so clip math is grounded in real container facts.",
                "media file, workspace-relative",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "video_beats",
            signal: "using `video_beats`",
            action: "",
            ideas: "",
            pages: &[
                "Cut points should snap to these beats — music owns time.",
                "Analyze a music track and return its beat grid as JSON: estimated BPM, beat timestamps, onset (transient) timestamps, and windowed RMS energy so you can hear where phrases and the crescendo live.",
                "audio file, workspace-relative",
                "seconds per energy window (default 5)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "video_cut",
            signal: "using `video_cut`",
            action: "",
            ideas: "",
            pages: &[
                "Beat-snap the boundaries yourself using video_beats output — this tool renders exactly what you specify.",
                "Give clips as {path, in, out} (seconds) in timeline order; segment boundaries are joined with a crossfade of `fade` seconds (0 = hard cuts).",
                "source in-point, seconds",
                "source out-point, seconds",
                "destination mp4, workspace-relative",
                "optional audio bed, workspace-relative",
                "seconds into the music to start (default 0)",
                "crossfade seconds between clips (default 0 = hard cuts)",
                "audio fade-out length at the end (default 1.0)",
                "output width (default: first clip's)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "video_contact_sheet",
            signal: "using `video_contact_sheet`",
            action: "",
            ideas: "",
            pages: &[
                "Feed the sheet to video_look for a machine read, or present() it to the director for circle/selects decisions.",
                "video file, workspace-relative",
                "destination png/jpg, workspace-relative",
                "number of samples (default 12, max 64)",
                "grid columns (default 4)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "video_look",
            signal: "using `video_look`",
            action: "",
            ideas: "",
            pages: &[
                "Use after video_contact_sheet for single-pass take review, or directly with `frames`/`timestamps` for targeted questions.",
                "video file (extract frames) or image file (sent as-is), workspace-relative",
                "what to look for, e.g. 'rank the takes on handoff cleanliness' or 'describe palette and framing of each shot'",
                "evenly spaced frames to extract when path is a video (default 6, max 8; ignored for images)",
                "exact timestamps (seconds) to grab instead of even spacing; overrides frames",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "vision_look",
            signal: "using `vision_look`",
            action: "",
            ideas: "",
            pages: &[
                "Prefer this over inventing visual details.",
                "image or video path, workspace-relative",
                "what to look for, e.g. 'OCR all text' or 'describe the UI state'",
                "evenly spaced frames when path is a video (default 4, max 8; ignored for images)",
                "exact timestamps (seconds) for video frames; overrides frames",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "knowledge_graph",
            signal: "using `knowledge_graph`",
            action: "",
            ideas: "",
            pages: &[
                "Facts persist across sessions — use ingest for documents worth remembering, query before re-reading sources.",
                "pipeline stage to run",
                "ingest: the document text",
                "ingest: short source id for provenance (e.g. a path or URL)",
                "query: the question to answer from the graph",
                "optional club label; default self (the in-hand driver)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "continual_harness",
            signal: "using `continual_harness`",
            action: "",
            ideas: "",
            pages: &[
                "Prefer small evidence-backed edits after a repeated failure, reusable tactic, or durable preference.",
                "default list",
                "default project",
                "required for create/update",
                "entry id (or refinement id for rollback)",
                "grouping path, default general",
                "why this edit is justified",
                "expected improvement",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "work_landing",
            signal: "using `work_landing`",
            action: "",
            ideas: "",
            pages: &[
                "Establish your WORK CONTEXT for this workspace at the start of a conversation.",
                "Then ASK the user to confirm.",
                "Call again with confirm=true (plus repo / visibility / mode overrides if they corrected anything) to RECORD the confirmed context, which is persisted and drives your behavior mode for this workspace.",
                "record the user's confirmation (sets the context as established)",
                "owner/repo override when confirming (if detection missed/misread it)",
                "visibility override when confirming",
                "behavior-mode override when confirming (else inferred from visibility)",
            ],
        },
    ],
};
