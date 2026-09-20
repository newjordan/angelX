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
/// `source_*` is deliberately catalog metadata, not a claim that angel0 read
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
/// content that angel0 has read.
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

fn local_lesson_steps(discipline: Discipline, topic: &str) -> (String, String, String) {
    match discipline {
        Discipline::ProbabilityStatistics => (
            format!(
                "Frame “{topic}” as an experiment with a sample space, random variable, and explicit assumptions."
            ),
            "Choose one concrete case, calculate an expected outcome, then describe a small simulation that could check it."
                .to_string(),
            "You can state what is random, what is measured, and which assumption would invalidate the result."
                .to_string(),
        ),
        Discipline::LinearAlgebra => (
            format!(
                "Represent “{topic}” as a map between named spaces and identify what the map preserves."
            ),
            "Draw a two- or three-dimensional example, label its input and output, and test one vector by hand."
                .to_string(),
            "You can name the domain, codomain, basis-dependent representation, and one invariant."
                .to_string(),
        ),
        Discipline::GpuKernels => (
            format!(
                "Explain “{topic}” in terms of work per element, bytes moved, parallel ownership, and synchronization."
            ),
            "Write a tiny input/output contract, estimate arithmetic intensity, and identify the first measurement you would take."
                .to_string(),
            "You can distinguish a compute, bandwidth, launch, or synchronization bottleneck using evidence."
                .to_string(),
        ),
        Discipline::Graphics3d => (
            format!(
                "Place “{topic}” precisely in the path from coordinates and scene data to a final pixel."
            ),
            "Trace one point, ray, or triangle through the relevant spaces and mark the visibility and sampling decisions."
                .to_string(),
            "You can predict one visible artifact caused by getting this stage wrong and explain why it appears."
                .to_string(),
        ),
        Discipline::GameDevelopment => (
            format!(
                "Turn “{topic}” into an observable state transition inside one playable feedback loop."
            ),
            "Name the input, state before, update rule, state after, and the feedback shown to the player."
                .to_string(),
            "You can replay the loop step by step and identify which state makes a bug reproducible."
                .to_string(),
        ),
        Discipline::LlmSystems => (
            format!(
                "Trace “{topic}” through tensor shapes, compute cost, information flow, and the training signal."
            ),
            "Choose a tiny batch and sequence length, write the important tensor shapes, and mark where loss can change the parameters."
                .to_string(),
            "You can explain one quality or efficiency tradeoff without hiding behind model scale."
                .to_string(),
        ),
        Discipline::NaturalSciences => (
            format!(
                "Connect “{topic}” to an observation, a proposed mechanism, a relevant scale or unit, and a falsifiable prediction."
            ),
            "Work one concrete case: list what is observed, what is inferred, and what measurement could distinguish two explanations."
                .to_string(),
            "You can separate observation from mechanism and name evidence that would change your conclusion."
                .to_string(),
        ),
        Discipline::Foundations => (
            format!(
                "Define “{topic}” plainly through one concrete example, then identify what the example generalizes."
            ),
            "Construct the smallest example you can, solve or execute it step by step, and change one condition."
                .to_string(),
            "You can explain the example in your own words and predict what changes in the nearby case."
                .to_string(),
        ),
    }
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
        format!(
            "Recall · answer from memory, then check above\n    state the objective\n    reconstruct {}'s method\n    name the checkpoint's proof",
            self.tutor.name
        )
    }

    /// Produce the visible, editable handoff for the ordinary composer. Merely
    /// constructing this string cannot submit a turn or spend provider credit.
    /// The tutor inherits the lesson's Recall practice, so the session ends by
    /// CHECKING the retrieval answers instead of re-reading the material — the
    /// same single-sourced prompts the offline render shows the operator.
    pub(crate) fn ask_tutor_prompt(&self) -> String {
        format!(
            "Tutor me on “{}” using {}'s method: {}.\n\nLearning objective: {}\nStart with this exercise: {}\nCheck my understanding against: {}\nSuggested curriculum: {} — {} ({}). This is a catalog pointer; do not claim you have read its content unless you actually access it.\n\nFirst ask what I already know. Teach one step at a time, make uncertainty explicit, and wait for my answer before continuing.\n\nEnd the lesson with retrieval practice — ask me to answer these before you confirm understanding:\n{}",
            self.topic,
            self.tutor.name,
            self.tutor.method,
            self.objective,
            self.exercise,
            self.checkpoint,
            self.shelf.title,
            self.source_label,
            self.source_url,
            self.recall_prompt()
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
mod tests {
    use super::*;
    use std::collections::HashSet;

    const DISCIPLINES: [Discipline; 8] = [
        Discipline::ProbabilityStatistics,
        Discipline::LinearAlgebra,
        Discipline::GpuKernels,
        Discipline::Graphics3d,
        Discipline::GameDevelopment,
        Discipline::LlmSystems,
        Discipline::NaturalSciences,
        Discipline::Foundations,
    ];

    #[test]
    fn every_discipline_has_a_tutor_and_multiple_world_shelves_exist() {
        for discipline in DISCIPLINES {
            assert!(!tutor_for(discipline).name.is_empty());
            assert!(!primary_shelf(discipline).url.is_empty());
            assert!(
                CURRICULUM
                    .iter()
                    .any(|shelf| shelf.discipline == discipline),
                "{discipline:?} has no curriculum"
            );
        }
        assert!(CURRICULUM.len() >= 10);
    }

    #[test]
    fn resident_faculty_and_curriculum_catalog_are_unique_and_actionable() {
        let mut tutor_ids = HashSet::new();
        let mut tutor_names = HashSet::new();
        for tutor in TUTORS {
            assert!(
                tutor_ids.insert(tutor.id),
                "duplicate tutor id {}",
                tutor.id
            );
            assert_eq!(tutor_by_id(tutor.id), Some(&tutor));
            assert!(
                tutor_names.insert(tutor.name),
                "duplicate tutor {}",
                tutor.name
            );
            assert!(!tutor.role.trim().is_empty());
            assert!(
                tutor.method.split_whitespace().count() >= 6,
                "{} needs a concrete teaching method",
                tutor.name
            );
        }

        let mut shelf_ids = HashSet::new();
        let mut urls = HashSet::new();
        let mut titles = HashSet::new();
        for shelf in CURRICULUM {
            assert!(
                shelf_ids.insert(shelf.id),
                "duplicate curriculum id {}",
                shelf.id
            );
            assert_eq!(shelf_by_id(shelf.id), Some(&shelf));
            assert!(
                shelf.url.starts_with("https://github.com/"),
                "{} must remain an inspectable upstream curriculum",
                shelf.title
            );
            assert!(
                urls.insert(shelf.url),
                "duplicate curriculum URL {}",
                shelf.url
            );
            let github_path = shelf
                .url
                .strip_prefix("https://github.com/")
                .expect("curriculum URL has a GitHub path");
            let parts = github_path.split('/').collect::<Vec<_>>();
            assert!(
                parts.len() == 2
                    || (parts.len() >= 5
                        && parts[2] == "tree"
                        && parts[3..].iter().all(|part| !part.is_empty())),
                "{} has a malformed repository or tree URL: {}",
                shelf.title,
                shelf.url
            );
            assert!(
                !parts[0].is_empty() && !parts[1].is_empty(),
                "{} is missing a GitHub owner or repository",
                shelf.title
            );
            assert!(
                titles.insert(shelf.title),
                "duplicate curriculum title {}",
                shelf.title
            );
            assert!(!shelf.source.trim().is_empty());
            assert!(
                shelf.arc.len() >= 4,
                "{} needs a real learning sequence",
                shelf.title
            );
            assert!(
                shelf.arc.iter().all(|concept| !concept.trim().is_empty()),
                "{} contains a blank concept",
                shelf.title
            );
        }
        assert_eq!(catalog_tutors(), &TUTORS);
        assert_eq!(catalog_shelves(), &CURRICULUM);
    }

    #[test]
    fn multiword_topics_normalize_into_immediate_offline_lessons() {
        let lesson = local_lesson("  ray\n tracing\twith   a BVH  ")
            .expect("a multiword topic produces a lesson");
        assert_eq!(lesson.topic, "ray tracing with a BVH");
        assert_eq!(lesson.discipline, Discipline::Graphics3d);
        assert_eq!(lesson.tutor.id, "vesper-raster");
        assert_eq!(lesson.shelf.id, "graphics-ray-tracing");
        assert_eq!(lesson.source_label, "Ray Tracing in One Weekend");
        assert!(lesson.next_arc.contains(&"light transport"));

        let rendered = lesson.render_offline();
        assert!(rendered.starts_with("LOCAL LESSON · READY OFFLINE"));
        assert!(rendered.contains("Objective ·"));
        assert!(rendered.contains("Exercise ·"));
        assert!(rendered.contains("Checkpoint ·"));
        assert!(rendered.contains(lesson.source_url));
        assert!(rendered.contains("course content has not been loaded"));
        assert!(!rendered.contains("Source · Wikipedia"));
    }

    #[test]
    fn topic_normalization_is_bounded_unicode_safe_and_rejects_noise() {
        assert_eq!(normalize_topic(" \n\t "), None);
        assert_eq!(normalize_topic("+++"), None);
        assert_eq!(
            normalize_topic("χ²   distribution"),
            Some("χ² distribution".into())
        );

        let long = format!("{} final concept", "界".repeat(LOCAL_TOPIC_MAX_CHARS));
        let normalized = normalize_topic(&long).expect("long topic is clipped");
        assert_eq!(normalized.chars().count(), LOCAL_TOPIC_MAX_CHARS);
        assert!(normalized.ends_with('…'));
        assert!(local_lesson(&normalized).is_some());
    }

    #[test]
    fn catalog_shelf_opens_by_stable_id_without_network_or_model_state() {
        let lesson =
            local_lesson_for_shelf("llm-systems-cs336").expect("stable shelf id opens a lesson");
        assert_eq!(lesson.topic, "Language models from scratch");
        assert_eq!(lesson.shelf.id, "llm-systems-cs336");
        assert_eq!(lesson.tutor.id, "ilex-tokenwright");
        assert_eq!(lesson.next_arc.first(), Some(&"tokenization"));
        assert!(local_lesson_for_shelf("missing-shelf").is_none());
    }

    #[test]
    fn ask_tutor_handoff_is_editable_attributed_and_does_not_claim_source_access() {
        let lesson = local_lesson("Poisson distribution").unwrap();
        let prompt = lesson.ask_tutor_prompt();
        assert!(prompt.starts_with("Tutor me on “Poisson distribution”"));
        assert!(prompt.contains(lesson.tutor.name));
        assert!(prompt.contains(lesson.tutor.method));
        assert!(prompt.contains(lesson.source_label));
        assert!(prompt.contains(lesson.source_url));
        assert!(prompt.contains("do not claim you have read its content"));
        assert!(prompt.contains("wait for my answer before continuing"));
    }

    #[test]
    fn every_discipline_builds_a_complete_local_lesson() {
        let topics = [
            "Poisson probability distribution",
            "eigenvalue of a matrix",
            "CUDA reduction kernel",
            "ray tracing BVH",
            "game engine gameplay loop",
            "transformer attention",
            "quantum particle physics",
            "recursion",
        ];
        let reached = topics
            .into_iter()
            .map(|topic| local_lesson(topic).expect("representative lesson"))
            .map(|lesson| {
                assert!(!lesson.objective.trim().is_empty());
                assert!(!lesson.exercise.trim().is_empty());
                assert!(!lesson.checkpoint.trim().is_empty());
                assert!(!lesson.source_url.trim().is_empty());
                assert!(!lesson.next_arc.is_empty());
                lesson.discipline
            })
            .collect::<HashSet<_>>();
        assert_eq!(reached, DISCIPLINES.into_iter().collect());
    }

    #[test]
    fn living_catalog_exposes_every_resident_and_sourced_study_arc_once() {
        let catalog = catalog_text();
        assert!(catalog.starts_with("RESIDENT FACULTY · 6\n"));
        assert!(catalog.contains("CURRICULUM SHELVES · 16\n"));

        for tutor in TUTORS {
            assert_eq!(catalog.matches(tutor.name).count(), 1, "{}", tutor.name);
            assert_eq!(catalog.matches(tutor.role).count(), 1, "{}", tutor.name);
            assert_eq!(catalog.matches(tutor.method).count(), 1, "{}", tutor.name);
        }
        for shelf in CURRICULUM {
            assert_eq!(catalog.matches(shelf.title).count(), 1, "{}", shelf.title);
            assert_eq!(
                catalog
                    .matches(&format!("     Source · {}", shelf.source))
                    .count(),
                1,
                "{}",
                shelf.title
            );
            assert_eq!(
                catalog
                    .matches(&format!("     Course · {}", shelf.url))
                    .count(),
                1,
                "{}",
                shelf.title
            );
            assert_eq!(
                catalog
                    .matches(&format!("     Arc · {}", shelf.arc.join(" → ")))
                    .count(),
                1,
                "{}",
                shelf.title
            );
        }
    }

    #[test]
    fn high_value_domains_route_to_their_resident_tutors() {
        assert_eq!(
            classify("eigenvalue of a matrix"),
            Discipline::LinearAlgebra
        );
        assert_eq!(classify("Triton CUDA reduction"), Discipline::GpuKernels);
        assert_eq!(classify("WebGPU shader"), Discipline::Graphics3d);
        assert_eq!(classify("Bevy game engine"), Discipline::GameDevelopment);
        assert_eq!(classify("transformer attention"), Discipline::LlmSystems);
        assert_eq!(
            classify("Poisson probability distribution"),
            Discipline::ProbabilityStatistics
        );
        assert_eq!(
            classify("quantum particle physics"),
            Discipline::NaturalSciences
        );
        assert_eq!(
            classify("chemical molecule reaction"),
            Discipline::NaturalSciences
        );
        assert_eq!(
            classify("cellular protein genetics"),
            Discipline::NaturalSciences
        );
        assert_eq!(
            classify("medical disease physiology"),
            Discipline::NaturalSciences
        );
    }

    #[test]
    fn teaching_note_names_provenance_and_next_concepts() {
        let note = teaching_note("matrix", "Matrix", "A rectangular array in linear algebra.");
        assert!(note.contains("Sable Vector"));
        assert!(note.contains("MIT 18.06"));
        assert!(note.contains("https://github.com/mitmath/1806"));
        assert!(note.contains("Next · vectors"));
    }

    #[test]
    fn every_lesson_closes_with_deterministic_recall_practice() {
        // Every shelf must deliver a lesson whose Recall section is
        // deterministic (same lesson, same prompt) and names that lesson's
        // resident tutor.
        for shelf in catalog_shelves() {
            let lesson = local_lesson_for_shelf(shelf.id).expect("shelf lesson");
            let once = lesson.recall_prompt();
            let twice = lesson.recall_prompt();
            assert_eq!(once, twice, "recall prompts are deterministic");
            assert!(
                once.contains(lesson.tutor.name),
                "recall names its tutor: {once}"
            );
            assert!(once.contains("Recall"), "{once}");
            let rendered = lesson.render_offline();
            assert!(
                rendered.contains("Recall · answer from memory, then check above"),
                "offline render embeds the recall section: {rendered}"
            );
            // The Ask Tutor handoff inherits the SAME recall practice, so the
            // tutor checks the answers instead of re-reading the material.
            let handoff = lesson.ask_tutor_prompt();
            assert!(
                handoff.contains("End the lesson with retrieval practice"),
                "handoff schedules the recall check: {handoff}"
            );
            assert!(
                handoff.contains(&lesson.recall_prompt()),
                "handoff reuses the single-sourced recall prompts: {handoff}"
            );
        }
    }

    #[test]
    fn every_resident_can_deliver_a_personalized_sourced_lesson() {
        let samples = [
            (
                Discipline::ProbabilityStatistics,
                "Poisson",
                "probability distribution",
            ),
            (Discipline::LinearAlgebra, "matrix", "linear algebra matrix"),
            (Discipline::GpuKernels, "warp", "CUDA warp kernel"),
            (Discipline::Graphics3d, "shader", "WebGPU shader rendering"),
            (
                Discipline::GameDevelopment,
                "gameplay",
                "game engine gameplay loop",
            ),
            (Discipline::LlmSystems, "attention", "transformer attention"),
            (
                Discipline::NaturalSciences,
                "quantum",
                "quantum particle physics",
            ),
            (Discipline::Foundations, "recursion", "introductory concept"),
        ];
        for (discipline, term, context) in samples {
            assert_eq!(classify(context), discipline);
            let note = teaching_note(term, term, context);
            let tutor = tutor_for(discipline);
            assert!(note.contains(tutor.name), "{discipline:?}: {note}");
            assert!(note.contains(tutor.role), "{discipline:?}: {note}");
            assert!(note.contains(tutor.method), "{discipline:?}: {note}");
            assert!(note.contains("Prompt ·"), "{discipline:?}: {note}");
            assert!(
                note.contains(shelf_for(discipline, context).source),
                "{discipline:?}: {note}"
            );
            assert!(
                note.contains(shelf_for(discipline, context).url),
                "{discipline:?}: {note}"
            );
            assert!(note.contains("Next ·"), "{discipline:?}: {note}");
        }
    }

    #[test]
    fn probability_and_natural_sciences_choose_relevant_college_shelves() {
        let poisson = teaching_note(
            "Poisson",
            "Poisson distribution",
            "A discrete probability distribution for event counts.",
        );
        assert!(poisson.contains("Probability and statistics"), "{poisson}");
        assert!(poisson.contains("OpenStax Statistics"), "{poisson}");
        assert!(poisson.contains("random variables"), "{poisson}");
        assert!(!poisson.contains("OSSU"), "{poisson}");

        let physics = teaching_note(
            "quantum",
            "Quantum mechanics",
            "A theory in physics describing particles and energy.",
        );
        assert!(physics.contains("OpenStax University Physics"), "{physics}");

        let chemistry = teaching_note(
            "molecule",
            "Molecule",
            "A chemical group of bonded atoms involved in reactions.",
        );
        assert!(chemistry.contains("OpenStax Chemistry"), "{chemistry}");

        let biology = teaching_note(
            "protein",
            "Protein",
            "A biological molecule encoded by genetic information in cells.",
        );
        assert!(biology.contains("OpenStax Biology"), "{biology}");
    }

    #[test]
    fn every_curriculum_shelf_is_reachable_from_a_representative_lesson() {
        let cases = [
            (
                "recursion in introductory programming",
                "College foundations",
            ),
            (
                "computational thinking with a climate model and simulation",
                "Computational thinking",
            ),
            (
                "Poisson probability distribution",
                "Probability and statistics",
            ),
            ("quantum particle physics", "University physics"),
            ("chemical reaction and molecular bond", "College chemistry"),
            ("cellular protein genetics", "College biology"),
            ("eigenvalue of a matrix", "Linear algebra spine"),
            (
                "numerical linear algebra factorization stability",
                "Numerical linear algebra",
            ),
            ("CUDA warp reduction kernel", "GPU kernel forge"),
            ("GPU architecture roofline compiler", "GPU field references"),
            ("raster geometry shading", "Renderer from first pixels"),
            ("ray tracing BVH light transport", "Ray tracing shelf"),
            ("WebGPU render pipeline adapter", "WebGPU workshop"),
            ("game engine gameplay loop", "Game systems workshop"),
            (
                "transformer attention scaling",
                "Language models from scratch",
            ),
            (
                "small language model tokenizer SFT",
                "Small complete LLM laboratory",
            ),
        ];
        let mut reached = HashSet::new();
        for (context, expected_title) in cases {
            let discipline = classify(context);
            let shelf = shelf_for(discipline, context);
            assert_eq!(shelf.title, expected_title, "{context}");
            let note = teaching_note(context, context, context);
            assert!(
                note.contains(&format!("Course · {}", shelf.url)),
                "{} is reachable but its upstream URL is hidden\n{note}",
                shelf.title
            );
            reached.insert(shelf.title);
        }
        assert_eq!(
            reached.len(),
            CURRICULUM.len(),
            "every linked shelf must be reachable"
        );
        assert!(
            CURRICULUM.iter().all(|shelf| reached.contains(shelf.title)),
            "one or more cataloged shelves are decorative-only"
        );
    }
}
