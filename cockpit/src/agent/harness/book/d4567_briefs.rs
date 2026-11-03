//! ⡸ (dots 4567) — Volume VII, the seats and the preamble, on the 8-dot
//! shelf: more briefs. The tutor's local lessons — three steps per discipline,
//! an objective, an exercise and a checkpoint, the topic beside them — which
//! the operator reads recited and the tutor seat reads as addresses; the
//! outbound pre-provisioner, connected over its own chat completion; the
//! Grok research tool's scout, a CLI seat with no tool channel, which hears its
//! pages recited (`book::connect::recite`); and the operator's `/mention` and
//! `/skills`, their stand-in tasks and the frames of what they load.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/d4567_briefs.book");
pub(crate) const CELL: char = '⡸';
pub(crate) const LESSONS: Route = Route::new(CELL, '⠁');
pub(crate) const LESSONS_MORE: Route = Route::new(CELL, '⠃');
pub(crate) const LESSONS_LAST: Route = Route::new(CELL, '⠉');
pub(crate) const PROVISIONER: Route = Route::new(CELL, '⠙');
pub(crate) const GROK_RESEARCH: Route = Route::new(CELL, '⠑');
pub(crate) const MENTION: Route = Route::new(CELL, '⠋');
pub(crate) const SKILLS: Route = Route::new(CELL, '⠛');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "briefs",
    surface: "more seats' briefs: the tutor's local lessons, the pre-provisioner, the Grok research scout, /mention and /skills",
    subs: &[
        Sub {
            route: LESSONS,
            name: "lessons",
            signal: "a local lesson's steps: probability and statistics, linear algebra, GPU kernels (the topic beside them)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'1'),
        },
        Sub {
            route: LESSONS_MORE,
            name: "lessons-more",
            signal: "a local lesson's steps: 3D graphics, game development, LLM systems (the topic beside them)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'2'),
        },
        Sub {
            route: LESSONS_LAST,
            name: "lessons-last",
            signal: "a local lesson's steps: the natural sciences, foundations (the topic beside them)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'3'),
        },
        Sub {
            route: PROVISIONER,
            name: "provisioner",
            signal: "the outbound pre-provisioner: classify the task excerpt below",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: GROK_RESEARCH,
            name: "grok-research",
            signal: "the grok_research tool's scout, recited to a seat with no tool channel",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'5'),
        },
        Sub {
            route: MENTION,
            name: "mention",
            signal: "the operator's /mention: the path beside the task, the file's contents fenced below (each value beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'6'),
        },
        Sub {
            route: SKILLS,
            name: "skills",
            signal: "the operator's /skills: the names beside the task, the skills' instructions fenced below (each value beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'7'),
        },
    ],
};
