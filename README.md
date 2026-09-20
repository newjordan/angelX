# angelX

angelX is a Rust terminal workspace for coding and research agents. It brings
model teams, code tools, persistent project memory, and measured experiments
together in one cockpit.

![Excalibur raised, with the wizard beside it](docs/images/intro.png)

```sh
# Requires Linux x86_64, Rust/Cargo, C/C++ tools, Bash and Python 3.
# The first launch builds from source.
git clone https://github.com/newjordan/angelX.git
cd angelX
ANGEL_VIDEO=0 ./bin/angel0
```

- **Visual workspace** — Code, inspect images, and explore Dotmax worlds in the terminal.
- **Models and teams** — Choose models and thinking levels, configure teams, and run agent graphs.
- **Repository tools** — Search files, inspect symbols, follow definitions, and review diffs.
- **Content-checked edits** — Hashline editing checks file content before applying anchored changes.
- **Programmable tools** — Run tool calls, loops, and filters locally in JavaScript with `code_mode`.
- **Long-session context** — Compact, deduplicate, and age retained context as work continues.
- **Project memory** — Review source-linked knowledge in Atlas and reuse it in later tasks.
- **Persistent goals** — Resume sessions and autonomous work with configurable run limits.
- **Headless runs** — Record task settings, source identity, tool activity, and acceptance evidence.
- **Measured campaigns** — Evaluate isolated attempts with verifiers and independent review.
- **Research loops** — Use Sloptomizer suggestions, Deli deliberation, and paired experiments.
- **Jev and benchmarks** — Request advisory probabilities and scores; calculate measured changes from paired benchmark samples.

[Model setup](cockpit/docs/ENV.md) · [/commands](docs/COMMANDS.md) ·
[Feature evidence](docs/FEATURES.md) · [Attributions](THIRD_PARTY_NOTICES.md) · [MIT](LICENSE)
