# Attributions

Angel's original code is [MIT licensed](LICENSE). Upstream code, adaptations and
dependencies retain their own licenses and notices.

| Project / author | Use in Angel | License / provenance |
|---|---|---|
| [OpenAI Codex](https://github.com/openai/codex) — OpenAI | `code_mode` identifies its origin in Codex's code-mode work; Angel rebuilds the runtime synchronously. The Linux sandbox adapts Codex Landlock code; command and project-instruction conventions also credit Codex. | [Apache-2.0](third-party/openai-codex-LICENSE.txt), [upstream NOTICE](third-party/openai-codex-NOTICE.txt); [code mode](cockpit/src/code_mode.rs), [sandbox](cockpit/src/sandbox.rs). |
| [oh-my-pi](https://github.com/can1357/oh-my-pi) — Mario Zechner, Can Bölük, Stencil Labs and contributors | Rust adaptations of hashline editing, stream rules, magic keywords and the advisor role. | [MIT notice](third-party/oh-my-pi-LICENSE.txt); [hashline](cockpit/src/hashline.rs), [stream rules](cockpit/src/stream_rules.rs), [keywords](cockpit/src/magic_keywords.rs), [advisor](cockpit/src/advisor.rs). |
| [DeepSeek-Reasonix](https://github.com/esengine/DeepSeek-Reasonix) — Reasonix contributors | Model-requested escalation convention, adapted in Rust. | [MIT notice](third-party/reasonix-LICENSE.txt); [local implementation](cockpit/src/harness/needs_pro.rs). |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent) — Nous Research | Bundled methodology playbooks identify Hermes as their source library. | [MIT notice](third-party/hermes-agent-LICENSE.txt); [skill loader](cockpit/src/harness/skills.rs). |
| [Deli_AutoResearch](https://victorchen96.github.io/auto_research/framework.html) — Deli Chen | Protocol inspiration for fresh-context deliberation, retained findings, stall detection and structural pivots. | [Local Rust implementation](cockpit/src/deli.rs); protocol credit, not a bundled upstream executable. |
| [Prime Agent](https://github.com/PrimeIntellect-ai/prime-agent) — Prime Intellect, Mario Zechner and contributors | Rust adaptation of editable continual-harness state and `/refine`; the source originally identifies the `newjordan/prime-agent` fork. | [MIT notice](third-party/prime-agent-LICENSE.txt); [local implementation](cockpit/src/continual_harness.rs). |
| [OpenScience](https://github.com/synthetic-sciences/openscience) — Synthetic Sciences and contributors | Literature-layer design port in Rust; the source identifies the `newjordan/openscience` fork. | [Apache-2.0](third-party/openscience-LICENSE.txt), [upstream NOTICE](third-party/openscience-NOTICE.txt); [local implementation](cockpit/src/science.rs). The retained NOTICE describes upstream OpenScience's broader dependencies. |
| Sloptomizer — newjordan | Ten selected original Python algorithm modules, with an Angel integration adapter. | [Original revision and file hashes](cockpit/research/sloptomizer/UPSTREAM.json), [integration scope](cockpit/research/sloptomizer/README.md). |
| [GEPA](https://github.com/gepa-ai/gepa) — GEPA authors and contributors | Prompt-evolution and Pareto-selection design references. | Research citation and local implementation links below. |
| [Dotmax](https://github.com/newjordan/dotmax) — Frosty and contributors | Vendored terminal graphics library. | [MIT](vendor/dotmax/LICENSE-MIT) / [Apache-2.0](vendor/dotmax/LICENSE-APACHE); [manifest](vendor/dotmax/Cargo.toml). |
| [ureq](https://github.com/algesten/ureq) — Martin Algesten, Jacob Hoffman-Andrews and contributors | Vendored HTTP client with Angel's cancellable-connection patch. | [MIT](vendor/ureq/LICENSE-MIT) / [Apache-2.0](vendor/ureq/LICENSE-APACHE); [patch description](vendor/ureq/ANGEL_PATCH.md). |
| [Ratatui](https://github.com/ratatui/ratatui), [Crossterm](https://github.com/crossterm-rs/crossterm), [rusty_v8](https://github.com/denoland/rusty_v8), [V8](https://v8.dev) and dependency contributors | Terminal UI, terminal I/O and JavaScript execution. | Exact dependency versions are in [Cargo.lock](cockpit/Cargo.lock); license metadata accompanies the [source-release manifest](docs/release-evidence.md). |
| Project owner / OpenAI ImageGen | Bundled portraits, world art and other project imagery; Excalibur derives from owner-supplied video. | [Artwork declaration](cockpit/assets/README.md), [Excalibur provenance](cockpit/assets/excalibur/README.md). |

[License retrieval records](third-party/sources.json) pin the notice files fetched
for this release. Those revisions do not establish the original port revisions.
Product screenshots are identified separately in [image provenance](docs/images/README.md).

## Research, algorithms and public resources

These are idea and method credits. The implementation links identify Angel's
use; they do not claim to reproduce the papers' results.

| Work and authors | Idea used in Angel | Implementation |
|---|---|---|
| [A Programming Paradigm for Spatiotemporal Composability](https://arxiv.org/abs/2608.25512) — Yifan Shi, Wei Zhang and Tianyi Cui (2026) | Reactive coeffects, reversible registration, interception and independence checks. | [Coeffects](cockpit/src/harness/coeffect.rs), [registration](cockpit/src/harness/registration.rs), [interception](cockpit/src/harness/interception.rs), [independence](cockpit/src/harness/independence.rs). |
| [GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning](https://arxiv.org/abs/2507.19457) — Lakshya A. Agrawal, Shangyin Tan, Dilara Soylu, Noah Ziems, Rishi Khare, Krista Opsahl-Ong, Arnav Singhvi, Herumb Shandilya, Michael J. Ryan, Meng Jiang, Christopher Potts, Koushik Sen, Alexandros G. Dimakis, Ion Stoica, Dan Klein, Matei Zaharia and Omar Khattab (2025; revised 2026) | Reflection-guided prompt evolution and selection over a Pareto population. | [Population](cockpit/research/sloptomizer/autoresearch/gepa/population.py), [selector](cockpit/research/sloptomizer/autoresearch/gepa/select.py), [reinforcement loop](cockpit/src/reinforce.rs). |
| [Learning, Fast and Slow: Towards LLMs That Adapt Continually](https://arxiv.org/abs/2605.12484v2) — Rishabh Tiwari, Kusha Sareen, Lakshya A. Agrawal, Joseph E. Gonzalez, Matei Zaharia, Kurt Keutzer, Inderjit S. Dhillon, Rishabh Agarwal and Devvrit Khatri (2026) | Fast retrieved context and a small slow learner in Sloptomizer. | [MicroLearner](cockpit/research/sloptomizer/orchestrator/micro_llm/core.py). |
| [Finite-time Analysis of the Multiarmed Bandit Problem](https://doi.org/10.1023/A:1013689704352) — Peter Auer, Nicolò Cesa-Bianchi and Paul Fischer (2002), *Machine Learning* 47, 235–256 | UCB1 exploration bonus for source selection. | [Bandit policies](cockpit/research/sloptomizer/orchestrator/self_improvement/bandits.py). |
| [Prime Agent: A Self-Improving RLM Harness](https://arxiv.org/abs/2608.23552) — Seth Karten, Alex L. Zhang, Kevin Thomas, Sebastian Müller, Elie Bakouch, Daniel Auras, Mika Senghaas, Fares Obeid, Konstantin Dunas, Johannes Hagemann and Sami Jaghouar (2026) | Durable supplemental prompts, memories, skills and delegation specifications with reviewable refinement. | [Continual harness](cockpit/src/continual_harness.rs). |
| [RL Systems Mind the Gap: Matching Trainer and Generator Throughput](https://newsletter.semianalysis.com/p/rl-systems-mind-the-gap-matching) — Kimbo Chen, Cheang Kang Wen and Dylan Patel / SemiAnalysis (2026) | Staleness budgets, oversampling, straggler pruning and reward-variance guards. | [Reinforcement loop](cockpit/src/reinforce.rs). |
| [Deli_AutoResearch](https://victorchen96.github.io/auto_research/framework.html) — Deli Chen | Fresh-context rounds, retained findings, stall detection and structural pivots. | [Deli driver](cockpit/src/deli.rs). |
| [OpenScience](https://github.com/synthetic-sciences/openscience) — Synthetic Sciences and contributors | Literature queries across indices, deduplication and ranked synthesis. | [Science bench](cockpit/src/science.rs). |
| [OpenAlex](https://openalex.org) — OurResearch / OpenAlex | Scholarly work and citation metadata. | [Science source adapter](cockpit/src/science.rs). |
| [Crossref](https://www.crossref.org/documentation/retrieve-metadata/rest-api/) — Crossref and its metadata contributors | DOI registration metadata. | [Science source adapter](cockpit/src/science.rs). |
| [Semantic Scholar Academic Graph](https://www.semanticscholar.org/product/api) — Ai2 / Semantic Scholar | Papers, citation counts and author metadata. | [Science source adapter](cockpit/src/science.rs). |
| [Europe PMC](https://europepmc.org/RestfulWebService) — Europe PMC / EMBL-EBI and contributors | Biomedical and life-science publication metadata. | [Science source adapter](cockpit/src/science.rs). |

The file-tool design also cites [Claude Code](https://github.com/anthropics/claude-code),
[aider](https://github.com/Aider-AI/aider) and [OpenHands](https://github.com/OpenHands/OpenHands)
as interface references in [context.rs](cockpit/src/harness/context.rs).
The searchable sources retain their own metadata and content terms.
