# angel0-cockpit

The Rust terminal runtime for angelX. It provides the interactive cockpit and
the headless task runner, with model routing, tools, workspace memory and
verifier-backed experiments.

From the repository root:

```sh
ANGEL_VIDEO=0 ./bin/angel0
```

Development checks:

```sh
bash scripts/check-cockpit-fast.sh --test-threads=1
bash scripts/check-legacy-terminal-boundary.sh
npm test
```

- [Model setup](../docs/MODELS.md), [commands](../docs/COMMANDS.md) and [environment settings](docs/ENV.md).
- [Headless task contract](docs/COMPETITION_RUNNER.md).
- [Jev tool](docs/JEV.md) and [Sloptomizer loop options](research/sloptomizer/README.md).
- [Repository memory](../docs/MEMORY.md) and [self-source navigation](../docs/SELF_MODEL.md).
- `src/harness/` runs tool turns; `src/tools/` defines the tool surface.
- `src/club/` implements provider routes; `src/swarm/` coordinates model teams.
- `src/loop_ctl.rs` owns autonomous loops; `src/rl_ctl/` owns campaign and research state.
- `src/draw/`, `src/scryglass.rs` and `src/world_viz/` render the terminal workspace.
- [Source release checks](../docs/release-evidence.md) and [attributions](../THIRD_PARTY_NOTICES.md).
