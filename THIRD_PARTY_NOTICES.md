# Attributions

Angel's original code is [MIT licensed](LICENSE). Upstream code, adaptations and
dependencies retain their own licenses and notices.

| Project / author | Use in Angel | License / provenance |
|---|---|---|
| [OpenAI Codex](https://github.com/openai/codex) — OpenAI | `code_mode` identifies its origin in Codex's code-mode work; Angel rebuilds the runtime synchronously. Command and project-instruction conventions also credit Codex. | [Apache-2.0](third-party/openai-codex-LICENSE.txt), [upstream NOTICE](third-party/openai-codex-NOTICE.txt); [local implementation](cockpit/src/code_mode.rs). |
| [oh-my-pi](https://github.com/can1357/oh-my-pi) — Mario Zechner, Can Bölük, Stencil Labs and contributors | Rust adaptations of hashline editing, stream rules, magic keywords and the advisor role. | [MIT notice](third-party/oh-my-pi-LICENSE.txt); [hashline](cockpit/src/hashline.rs), [stream rules](cockpit/src/stream_rules.rs), [keywords](cockpit/src/magic_keywords.rs), [advisor](cockpit/src/advisor.rs). |
| [DeepSeek-Reasonix](https://github.com/esengine/DeepSeek-Reasonix) — Reasonix contributors | Model-requested escalation convention, adapted in Rust. | [MIT notice](third-party/reasonix-LICENSE.txt); [local implementation](cockpit/src/harness/needs_pro.rs). |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent) — Nous Research | Bundled methodology playbooks identify Hermes as their source library. | [MIT notice](third-party/hermes-agent-LICENSE.txt); [skill loader](cockpit/src/harness/skills.rs). |
| [Deli_AutoResearch](https://victorchen96.github.io/auto_research/framework.html) — Deli Chen | Protocol inspiration for fresh-context deliberation, retained findings, stall detection and structural pivots. | [Local Rust implementation](cockpit/src/deli.rs); protocol credit, not a bundled upstream executable. |
| Sloptomizer — newjordan | Ten selected original Python algorithm modules, with an Angel integration adapter. | [Original revision and file hashes](cockpit/research/sloptomizer/UPSTREAM.json), [integration scope](cockpit/research/sloptomizer/README.md). |
| [GEPA](https://github.com/gepa-ai/gepa) and [Learning, Fast and Slow](https://arxiv.org/abs/2605.12484v2) — their research authors | Algorithm/design references for prompt evolution and Sloptomizer's small continual-learning kernel. | [GEPA-style reinforcement](cockpit/src/reinforce.rs), [selector](cockpit/research/sloptomizer/autoresearch/gepa/select.py), [kernel](cockpit/research/sloptomizer/orchestrator/micro_llm/core.py). |
| [Dotmax](https://github.com/newjordan/dotmax) — Frosty and contributors | Vendored terminal graphics library. | [MIT](vendor/dotmax/LICENSE-MIT) / [Apache-2.0](vendor/dotmax/LICENSE-APACHE); [manifest](vendor/dotmax/Cargo.toml). |
| [ureq](https://github.com/algesten/ureq) — Martin Algesten, Jacob Hoffman-Andrews and contributors | Vendored HTTP client with Angel's cancellable-connection patch. | [MIT](vendor/ureq/LICENSE-MIT) / [Apache-2.0](vendor/ureq/LICENSE-APACHE); [patch description](vendor/ureq/ANGEL_PATCH.md). |
| [Ratatui](https://github.com/ratatui/ratatui), [Crossterm](https://github.com/crossterm-rs/crossterm), [rusty_v8](https://github.com/denoland/rusty_v8), [V8](https://v8.dev) and dependency contributors | Terminal UI, terminal I/O and JavaScript execution. | Exact dependency versions are in [Cargo.lock](cockpit/Cargo.lock); license metadata accompanies the [source-release manifest](docs/release-evidence.md). |
| Project owner / OpenAI ImageGen | Bundled portraits, world art and other project imagery; Excalibur derives from owner-supplied video. | [Artwork declaration](cockpit/assets/README.md), [Excalibur provenance](cockpit/assets/excalibur/README.md). |

[License retrieval records](third-party/sources.json) pin the notice files fetched
for this release. Those revisions do not establish the original port revisions.
Product screenshots are identified separately in [image provenance](docs/images/README.md).
