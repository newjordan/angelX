//! The Scriptorium teaching library: resident tutors and a small, sourced
//! curriculum spine.
//!
//! This is deliberately a catalog, not vendored course content. Each shelf
//! links to the upstream public curriculum and contributes only a compact
//! sequence used to orient a world lesson.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Discipline {
    ProbabilityStatistics,
    LinearAlgebra,
    GpuKernels,
    Graphics3d,
    GameDevelopment,
    LlmSystems,
    NaturalSciences,
    Foundations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tutor {
    /// Stable catalog identity. UI selection and future persisted progress must
    /// bind to this value rather than the resident's array position.
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) role: &'static str,
    pub(crate) method: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CurriculumShelf {
    /// Stable catalog identity. Reordering the display catalog must not change
    /// which curriculum a lesson refers to.
    pub(crate) id: &'static str,
    pub(crate) discipline: Discipline,
    pub(crate) title: &'static str,
    pub(crate) source: &'static str,
    pub(crate) url: &'static str,
    pub(crate) arc: &'static [&'static str],
}

/// A deterministic teaching plan that is useful without a network or model.
///
/// `source_*` is deliberately catalog metadata, not a claim that angelX read
/// or vendored the linked course. Optional reference enrichment lives in
/// `term_lookup`; it can fail without taking this lesson away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalLesson {
    pub(crate) topic: String,
    pub(crate) discipline: Discipline,
    pub(crate) tutor: &'static Tutor,
    pub(crate) shelf: &'static CurriculumShelf,
    pub(crate) objective: String,
    pub(crate) exercise: String,
    pub(crate) checkpoint: String,
    pub(crate) source_label: &'static str,
    pub(crate) source_url: &'static str,
    pub(crate) next_arc: &'static [&'static str],
}

pub(crate) const LOCAL_TOPIC_MAX_CHARS: usize = 160;

pub(crate) const TUTORS: [Tutor; 6] = [
    Tutor {
        id: "sable-vector",
        name: "Sable Vector",
        role: "geometric cartographer",
        method: "draw the transformation, name the spaces, then find what stays invariant",
    },
    Tutor {
        id: "rhea-warpforge",
        name: "Rhea Warpforge",
        role: "kernel smith",
        method: "measure first, map the memory traffic, then change one bottleneck",
    },
    Tutor {
        id: "vesper-raster",
        name: "Vesper Raster",
        role: "graphics illusionist",
        method: "build the image from coordinates, visibility, light, and samples",
    },
    Tutor {
        id: "mira-loop",
        name: "Mira Loop",
        role: "game-systems gardener",
        method: "make one playable loop, expose its state, then grow the simulation",
    },
    Tutor {
        id: "ilex-tokenwright",
        name: "Ilex Tokenwright",
        role: "language-model engineer",
        method: "trace tensor shapes from tokens to loss before touching scale",
    },
    Tutor {
        id: "rowan-compass",
        name: "Rowan Compass",
        role: "foundations and natural-science provost",
        method: "define it plainly, solve one concrete case, then generalize",
    },
];

pub(crate) const CURRICULUM: [CurriculumShelf; 16] = [
    CurriculumShelf {
        id: "foundations-ossu",
        discipline: Discipline::Foundations,
        title: "College foundations",
        source: "OSSU Computer Science",
        url: "https://github.com/ossu/computer-science",
        arc: &["programming", "calculus", "discrete math", "systems"],
    },
    CurriculumShelf {
        id: "foundations-computational-thinking",
        discipline: Discipline::Foundations,
        title: "Computational thinking",
        source: "MIT 18.S191",
        url: "https://github.com/mitmath/computational-thinking",
        arc: &["models", "simulation", "probability", "climate"],
    },
    CurriculumShelf {
        id: "probability-openstax",
        discipline: Discipline::ProbabilityStatistics,
        title: "Probability and statistics",
        source: "OpenStax Statistics",
        url: "https://github.com/openstax/osbooks-statistics",
        arc: &[
            "random variables",
            "distributions",
            "estimation",
            "hypothesis tests",
            "regression",
        ],
    },
    CurriculumShelf {
        id: "physics-openstax",
        discipline: Discipline::NaturalSciences,
        title: "University physics",
        source: "OpenStax University Physics",
        url: "https://github.com/openstax/osbooks-university-physics-bundle",
        arc: &[
            "units and vectors",
            "mechanics",
            "waves and thermodynamics",
            "electromagnetism",
            "quantum physics",
        ],
    },
    CurriculumShelf {
        id: "chemistry-openstax",
        discipline: Discipline::NaturalSciences,
        title: "College chemistry",
        source: "OpenStax Chemistry",
        url: "https://github.com/openstax/osbooks-chemistry-bundle",
        arc: &[
            "matter and atoms",
            "bonding",
            "reactions",
            "thermochemistry",
            "equilibrium",
        ],
    },
    CurriculumShelf {
        id: "biology-openstax",
        discipline: Discipline::NaturalSciences,
        title: "College biology",
        source: "OpenStax Biology",
        url: "https://github.com/openstax/osbooks-biology-bundle",
        arc: &["cells", "genetics", "evolution", "physiology", "ecology"],
    },
    CurriculumShelf {
        id: "linear-algebra-1806",
        discipline: Discipline::LinearAlgebra,
        title: "Linear algebra spine",
        source: "MIT 18.06",
        url: "https://github.com/mitmath/1806",
        arc: &[
            "vectors",
            "linear maps",
            "four subspaces",
            "eigenstructure",
            "SVD",
        ],
    },
    CurriculumShelf {
        id: "numerical-linear-algebra-18335",
        discipline: Discipline::LinearAlgebra,
        title: "Numerical linear algebra",
        source: "MIT 18.335",
        url: "https://github.com/mitmath/18335",
        arc: &[
            "floating point",
            "factorizations",
            "stability",
            "iterative methods",
        ],
    },
    CurriculumShelf {
        id: "gpu-kernel-forge",
        discipline: Discipline::GpuKernels,
        title: "GPU kernel forge",
        source: "GPU MODE Lectures",
        url: "https://github.com/gpu-mode/lectures",
        arc: &[
            "profiling",
            "CUDA",
            "memory hierarchy",
            "reductions",
            "Triton",
        ],
    },
    CurriculumShelf {
        id: "gpu-field-references",
        discipline: Discipline::GpuKernels,
        title: "GPU field references",
        source: "GPU MODE Resource Stream",
        url: "https://github.com/gpu-mode/resource-stream",
        arc: &[
            "architecture",
            "roofline",
            "compilers",
            "distributed kernels",
        ],
    },
    CurriculumShelf {
        id: "graphics-tinyrenderer",
        discipline: Discipline::Graphics3d,
        title: "Renderer from first pixels",
        source: "ssloy/tinyrenderer",
        url: "https://github.com/ssloy/tinyrenderer",
        arc: &["lines", "triangles", "barycentrics", "camera", "shading"],
    },
    CurriculumShelf {
        id: "graphics-ray-tracing",
        discipline: Discipline::Graphics3d,
        title: "Ray tracing shelf",
        source: "Ray Tracing in One Weekend",
        url: "https://github.com/RayTracing/raytracing.github.io",
        arc: &[
            "rays",
            "materials",
            "acceleration",
            "sampling",
            "light transport",
        ],
    },
    CurriculumShelf {
        id: "graphics-webgpu",
        discipline: Discipline::Graphics3d,
        title: "WebGPU workshop",
        source: "gfx-rs/wgpu examples",
        url: "https://github.com/gfx-rs/wgpu/tree/trunk/examples",
        arc: &["adapter", "pipeline", "buffers", "textures", "compute"],
    },
    CurriculumShelf {
        id: "game-systems-bevy",
        discipline: Discipline::GameDevelopment,
        title: "Game systems workshop",
        source: "Bevy examples",
        url: "https://github.com/bevyengine/bevy/tree/latest/examples",
        arc: &["ECS", "input", "state", "rendering", "playable loop"],
    },
    CurriculumShelf {
        id: "llm-systems-cs336",
        discipline: Discipline::LlmSystems,
        title: "Language models from scratch",
        source: "Stanford CS336",
        url: "https://github.com/stanford-cs336/spring2025-lectures",
        arc: &[
            "tokenization",
            "transformer",
            "optimization",
            "scaling",
            "alignment",
        ],
    },
    CurriculumShelf {
        id: "llm-systems-nanochat",
        discipline: Discipline::LlmSystems,
        title: "Small complete LLM laboratory",
        source: "karpathy/nanochat",
        url: "https://github.com/karpathy/nanochat",
        arc: &["tokenizer", "pretraining", "evaluation", "SFT", "inference"],
    },
];

/// Resident rows for a selectable catalog. Callers should retain `Tutor::id`,
/// never the current array index, when identity must survive a reorder.
pub(crate) const fn catalog_tutors() -> &'static [Tutor] {
    &TUTORS
}

/// Curriculum rows for a selectable catalog. Callers should retain
/// `CurriculumShelf::id`, never the current array index.
pub(crate) const fn catalog_shelves() -> &'static [CurriculumShelf] {
    &CURRICULUM
}

pub(crate) fn tutor_by_id(id: &str) -> Option<&'static Tutor> {
    TUTORS.iter().find(|tutor| tutor.id == id)
}

pub(crate) fn shelf_by_id(id: &str) -> Option<&'static CurriculumShelf> {
    CURRICULUM.iter().find(|shelf| shelf.id == id)
}

/// Collapse layout/control whitespace and cap a user-authored topic without
/// splitting a Unicode scalar. This deliberately accepts multiword subjects;
/// the transcript-selection shortcut remains narrower in `term_lookup`.
pub(crate) fn normalize_topic(raw: &str) -> Option<String> {
    let cleaned = raw
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let topic = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if topic.is_empty() || !topic.chars().any(char::is_alphanumeric) {
        return None;
    }
    if topic.chars().count() <= LOCAL_TOPIC_MAX_CHARS {
        return Some(topic);
    }
    let mut clipped = topic
        .chars()
        .take(LOCAL_TOPIC_MAX_CHARS.saturating_sub(1))
        .collect::<String>();
    while clipped.ends_with(char::is_whitespace) {
        clipped.pop();
    }
    clipped.push('…');
    Some(clipped)
}

/// Build a useful local lesson for an arbitrary topic. No network, model,
/// filesystem, or environment access occurs here.
pub(crate) fn local_lesson(raw_topic: &str) -> Option<LocalLesson> {
    let topic = normalize_topic(raw_topic)?;
    let context = topic.clone();
    local_lesson_with_context(&topic, &context)
}

/// Preserve the operator's topic while allowing an optional reference excerpt
/// to improve discipline/shelf routing. The returned lesson still contains
/// only local teaching scaffolding; `context` is never represented as course
/// content that angelX has read.
pub(crate) fn local_lesson_with_context(raw_topic: &str, context: &str) -> Option<LocalLesson> {
    let topic = normalize_topic(raw_topic)?;
    let discipline = classify(context);
    let shelf = shelf_for(discipline, context);
    Some(build_local_lesson(topic, discipline, shelf))
}

/// Open one exact catalog shelf as a local lesson. The shelf title is the
/// default topic; this keeps Catalog → Enter deterministic and offline.
pub(crate) fn local_lesson_for_shelf(shelf_id: &str) -> Option<LocalLesson> {
    let shelf = shelf_by_id(shelf_id)?;
    Some(build_local_lesson(
        shelf.title.to_string(),
        shelf.discipline,
        shelf,
    ))
}

fn build_local_lesson(
    topic: String,
    discipline: Discipline,
    shelf: &'static CurriculumShelf,
) -> LocalLesson {
    let (objective, exercise, checkpoint) = local_lesson_steps(discipline, &topic);
    LocalLesson {
        topic,
        discipline,
        tutor: tutor_for(discipline),
        shelf,
        objective,
        exercise,
        checkpoint,
        source_label: shelf.source,
        source_url: shelf.url,
        next_arc: shelf.arc,
    }
}

/// A discipline's lesson steps on the book (`⡸`): the section and the page of
/// its objective; its exercise and its checkpoint follow.
fn lesson_pages(discipline: Discipline) -> (crate::agent::harness::book::Route, usize) {
    use crate::agent::harness::book::d4567_briefs::{LESSONS, LESSONS_LAST, LESSONS_MORE};
    match discipline {
        Discipline::ProbabilityStatistics => (LESSONS, 1),
        Discipline::LinearAlgebra => (LESSONS, 4),
        Discipline::GpuKernels => (LESSONS, 7),
        Discipline::Graphics3d => (LESSONS_MORE, 1),
        Discipline::GameDevelopment => (LESSONS_MORE, 4),
        Discipline::LlmSystems => (LESSONS_MORE, 7),
        Discipline::NaturalSciences => (LESSONS_LAST, 1),
        Discipline::Foundations => (LESSONS_LAST, 4),
    }
}

/// The page addresses of a discipline's objective, exercise and checkpoint.
pub(crate) fn lesson_step_addresses(discipline: Discipline) -> [String; 3] {
    use crate::agent::harness::book::d3_roles::pages;
    let (route, first) = lesson_pages(discipline);
    [first, first + 1, first + 2].map(|page| pages(route, [page]))
}

/// The steps as the operator reads them: recited from the book, the topic in
/// its place.
fn local_lesson_steps(discipline: Discipline, topic: &str) -> (String, String, String) {
    let [objective, exercise, checkpoint] = lesson_step_addresses(discipline).map(|address| {
        crate::agent::harness::book::connect::recite(&address).replace("{topic}", topic)
    });
    (objective, exercise, checkpoint)
}

impl LocalLesson {
    /// Immediate offline rendering. The source language is intentionally
    /// explicit: this plan links to a course but has not loaded course content.
    pub(crate) fn render_offline(&self) -> String {
        format!(
            "LOCAL LESSON · READY OFFLINE\nTopic · {}\n\nTutor · {} — {}\nMethod · {}\nObjective · {}\nExercise · {}\nCheckpoint · {}\n{}\nShelf · {} · {}\nCourse · {}\nSource note · catalog pointer only; course content has not been loaded\nNext · {}",
            self.topic,
            self.tutor.name,
            self.tutor.role,
            self.tutor.method,
            self.objective,
            self.exercise,
            self.checkpoint,
            self.recall_prompt(),
            self.shelf.title,
            self.source_label,
            self.source_url,
            self.next_arc.join(" → ")
        )
    }

    /// Retrieval practice, derived only from the lesson's own fields — recall
    /// beats rereading, so every lesson closes with prompts that force the
    /// reader to produce the material instead of scanning it. The answers are
    /// the objective, method, and checkpoint above the fold; nothing here is
    /// generated text and nothing is concealed from the lesson itself.
    pub(crate) fn recall_prompt(&self) -> String {
        // `⠬⠓`'s pages, recited for the operator, the tutor in place.
        use crate::agent::harness::book::{
            connect::recite, d3_roles::pages, ing_drivers::TUTOR_RECALL,
        };
        let [title, objective, method, proof] = [1, 2, 3, 4]
            .map(|page| recite(&pages(TUTOR_RECALL, [page])).replace("{tutor}", self.tutor.name));
        format!("{title}\n    {objective}\n    {method}\n    {proof}")
    }

    /// Produce the tutor's handoff, the context the tutor frame (`⠬⠃`) carries.
    /// Merely constructing this string cannot submit a turn or spend provider
    /// credit. The tutor inherits the lesson's Recall practice (`⠬⠓`), so the
    /// session ends by CHECKING the retrieval answers instead of re-reading the
    /// material — the same single-sourced pages the offline render recites for
    /// the operator.
    pub(crate) fn ask_tutor_prompt(&self) -> String {
        // `⠬⠛`: the handoff's words are the pages; the topic, the tutor and
        // the curriculum ride as data under their placeholders' names, the
        // lesson's steps as their addresses (`⡸`), its recall as `⠬⠓`.
        use crate::agent::harness::book::ing_drivers::{TUTOR_HANDOFF, TUTOR_RECALL};
        let [objective, exercise, checkpoint] = lesson_step_addresses(self.discipline);
        format!(
            "{}\ntopic: {}\ntutor: {}\nmethod: {}\nobjective: {objective}\nexercise: {exercise}\ncheckpoint: {checkpoint}\ncurriculum: {} — {} ({})\nrecall: {}",
            TUTOR_HANDOFF.cells(),
            self.topic,
            self.tutor.name,
            self.tutor.method,
            self.shelf.title,
            self.source_label,
            self.source_url,
            TUTOR_RECALL.cells(),
        )
    }
}

/// Render the Scriptorium's complete, inspectable teaching ledger. The
/// catalog is world-owned display text: it is never appended to conversation
/// history or offered to a model as hidden context.
#[cfg(test)]
pub(crate) fn catalog_text() -> String {
    let mut text = format!("RESIDENT FACULTY · {}\n", TUTORS.len());
    for (index, tutor) in TUTORS.iter().enumerate() {
        text.push_str(&format!(
            "\n{:02} · {} — {}\n     Method · {}\n",
            index + 1,
            tutor.name,
            tutor.role,
            tutor.method
        ));
    }

    text.push_str(&format!("\nCURRICULUM SHELVES · {}\n", CURRICULUM.len()));
    for (index, shelf) in CURRICULUM.iter().enumerate() {
        text.push_str(&format!(
            "\n{:02} · {} · {}\n     Source · {}\n     Course · {}\n     Arc · {}\n",
            index + 1,
            discipline_label(shelf.discipline),
            shelf.title,
            shelf.source,
            shelf.url,
            shelf.arc.join(" → ")
        ));
    }
    text
}

pub(crate) const fn discipline_label(discipline: Discipline) -> &'static str {
    match discipline {
        Discipline::ProbabilityStatistics => "PROBABILITY / STATISTICS",
        Discipline::LinearAlgebra => "LINEAR ALGEBRA",
        Discipline::GpuKernels => "GPU KERNELS",
        Discipline::Graphics3d => "GRAPHICS / 3D",
        Discipline::GameDevelopment => "GAME DEVELOPMENT",
        Discipline::LlmSystems => "LLM SYSTEMS",
        Discipline::NaturalSciences => "NATURAL SCIENCES",
        Discipline::Foundations => "FOUNDATIONS",
    }
}

#[cfg(test)]
pub(crate) fn teaching_note(term: &str, title: &str, summary: &str) -> String {
    let context = format!("{term} {title} {summary}");
    let discipline = classify(&context);
    let tutor = tutor_for(discipline);
    let shelf = shelf_for(discipline, &context);
    let cue = match discipline {
        Discipline::ProbabilityStatistics => format!(
            "For “{term}”, name the experiment, sample space, random variable, and assumptions; then calculate and simulate one case."
        ),
        Discipline::LinearAlgebra => {
            format!("Sketch “{term}” as a map. What are its input, output, and invariants?")
        }
        Discipline::GpuKernels => format!(
            "For “{term}”, name the work per element, bytes moved, and synchronization points."
        ),
        Discipline::Graphics3d => {
            format!("Place “{term}” in the path from coordinates to the final pixel.")
        }
        Discipline::GameDevelopment => {
            format!("Turn “{term}” into one observable state transition in a playable loop.")
        }
        Discipline::LlmSystems => {
            format!("Trace “{term}” through tensor shape, compute cost, and training signal.")
        }
        Discipline::NaturalSciences => format!(
            "For “{term}”, state the observation, mechanism, units or scale, and one falsifiable prediction."
        ),
        Discipline::Foundations => {
            format!("Give one concrete example of “{term}”, then say what the example generalizes.")
        }
    };
    format!(
        "Tutor · {} — {}\nMethod · {}\nPrompt · “{}”\nShelf · {} · {}\nCourse · {}\nNext · {}",
        tutor.name,
        tutor.role,
        tutor.method,
        cue,
        shelf.title,
        shelf.source,
        shelf.url,
        shelf.arc.join(" → ")
    )
}

pub(crate) fn classify(text: &str) -> Discipline {
    let text = text.to_lowercase();
    if contains_any(
        &text,
        &[
            "cuda",
            "triton",
            "gpu programming",
            "gpu performance",
            "gpu architecture",
            "kernel",
            "roofline",
            "distributed gpu",
            "warp",
            "thread block",
            "tensor core",
            "shared memory",
        ],
    ) {
        Discipline::GpuKernels
    } else if contains_any(
        &text,
        &[
            "language model",
            "transformer",
            "token",
            "attention",
            "embedding",
            "pretraining",
            "inference",
        ],
    ) {
        Discipline::LlmSystems
    } else if contains_any(
        &text,
        &[
            "game engine",
            "game development",
            "entity component",
            "gameplay",
            "simulation loop",
        ],
    ) {
        Discipline::GameDevelopment
    } else if contains_any(
        &text,
        &[
            "probability",
            "statistic",
            "distribution",
            "random variable",
            "stochastic",
            "bayes",
            "poisson",
            "gaussian",
            "variance",
            "likelihood",
        ],
    ) {
        Discipline::ProbabilityStatistics
    } else if contains_any(
        &text,
        &[
            "render",
            "raster",
            "ray tracing",
            "shader",
            "webgpu",
            "texture",
            "geometry",
            "three-dimensional",
            "3d",
        ],
    ) {
        Discipline::Graphics3d
    } else if contains_any(
        &text,
        &[
            "physics",
            "quantum",
            "thermodynamic",
            "electromagnet",
            "particle",
            "force",
            "energy",
            "chemistry",
            "chemical",
            "atom",
            "molecule",
            "reaction",
            "biology",
            "biological",
            "cellular",
            "membrane",
            "protein",
            "genetic",
            "species",
            "ecolog",
            "geolog",
            "astronom",
            "neuroscience",
            "physiology",
            "medicine",
            "medical",
            "disease",
            "syndrome",
            "anatomy",
        ],
    ) {
        Discipline::NaturalSciences
    } else if contains_any(
        &text,
        &[
            "linear algebra",
            "matrix",
            "vector space",
            "eigen",
            "singular value",
            "determinant",
            "basis",
            "tensor",
        ],
    ) {
        Discipline::LinearAlgebra
    } else {
        Discipline::Foundations
    }
}

pub(crate) fn tutor_for(discipline: Discipline) -> &'static Tutor {
    let id = match discipline {
        Discipline::ProbabilityStatistics => "rowan-compass",
        Discipline::LinearAlgebra => "sable-vector",
        Discipline::GpuKernels => "rhea-warpforge",
        Discipline::Graphics3d => "vesper-raster",
        Discipline::GameDevelopment => "mira-loop",
        Discipline::LlmSystems => "ilex-tokenwright",
        Discipline::NaturalSciences => "rowan-compass",
        Discipline::Foundations => "rowan-compass",
    };
    tutor_by_id(id).expect("every discipline maps to a resident tutor")
}

pub(crate) fn primary_shelf(discipline: Discipline) -> &'static CurriculumShelf {
    CURRICULUM
        .iter()
        .find(|shelf| shelf.discipline == discipline)
        .expect("every teaching discipline has a curriculum shelf")
}

fn shelf_for(discipline: Discipline, context: &str) -> &'static CurriculumShelf {
    let context = context.to_lowercase();
    let title = match discipline {
        Discipline::Foundations
            if contains_any(
                &context,
                &[
                    "computational thinking",
                    "climate model",
                    "simulation",
                    "scientific model",
                ],
            ) =>
        {
            "Computational thinking"
        }
        Discipline::LinearAlgebra
            if contains_any(
                &context,
                &[
                    "numerical",
                    "floating point",
                    "factorization",
                    "stability",
                    "iterative method",
                    "condition number",
                ],
            ) =>
        {
            "Numerical linear algebra"
        }
        Discipline::GpuKernels
            if contains_any(
                &context,
                &[
                    "gpu architecture",
                    "roofline",
                    "compiler",
                    "distributed kernel",
                    "distributed gpu",
                ],
            ) =>
        {
            "GPU field references"
        }
        Discipline::Graphics3d
            if contains_any(&context, &["webgpu", "wgpu", "render pipeline", "adapter"]) =>
        {
            "WebGPU workshop"
        }
        Discipline::Graphics3d
            if contains_any(
                &context,
                &[
                    "ray tracing",
                    "path tracing",
                    "light transport",
                    "acceleration structure",
                    "bvh",
                ],
            ) =>
        {
            "Ray tracing shelf"
        }
        Discipline::LlmSystems
            if contains_any(
                &context,
                &[
                    "nanochat",
                    "small language model",
                    "small llm",
                    "tokenizer",
                    "tokenization",
                    "supervised fine-tuning",
                    "sft",
                ],
            ) =>
        {
            "Small complete LLM laboratory"
        }
        Discipline::NaturalSciences
            if contains_any(
                &context,
                &[
                    "biology",
                    "biological",
                    "cellular",
                    " cell",
                    "membrane",
                    "protein",
                    "genetic",
                    "species",
                    "ecolog",
                    "physiology",
                    "neuroscience",
                    "medicine",
                    "medical",
                    "disease",
                    "syndrome",
                    "anatomy",
                ],
            ) =>
        {
            "College biology"
        }
        Discipline::NaturalSciences
            if contains_any(
                &context,
                &[
                    "chemistry",
                    "chemical",
                    "atom",
                    "molecule",
                    "reaction",
                    "bond",
                    "equilibrium",
                ],
            ) =>
        {
            "College chemistry"
        }
        Discipline::NaturalSciences => "University physics",
        _ => return primary_shelf(discipline),
    };
    CURRICULUM
        .iter()
        .find(|shelf| shelf.title == title)
        .expect("each contextual teaching branch has a curriculum shelf")
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/library__tests.rs"]
mod tests;
