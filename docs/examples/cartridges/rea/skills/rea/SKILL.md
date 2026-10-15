---
name: rea
description: Drive REA (Reverse Engineer Anything) through its CLI for a /rea target or question.
---

# REA in angelX

REA is plugged in as a cartridge, through its CLI rather than its MCP server:

    ${ANGEL_CARTRIDGE_DIR}/bin/rea <command> <args> --json

- The REA skill that follows names MCP tools in snake_case. Each is the CLI
  command of the same name in kebab-case: `analyze_javascript_application` is
  `rea analyze-javascript-application`. `rea --help` lists the commands,
  `rea <command> --help` their flags, `rea <command> --schema` their input.
- The CLI needs no registration, so the skill's connection, doctor and setup
  steps do not apply here.
- Evidence can be very large. Write `--json` output to a file in the workspace,
  then read projections of it: `rea inspect-analysis-view`, or
  `--filter-output <keys>` on the command. `--token-limit` shortens only output
  without `--json`.
- Native decompilation needs Ghidra, IDA or Hopper; `rea providers` shows what
  is configured. JavaScript/Electron, .NET, Go, Rust, WASM, SQLite, archives,
  websites and objdump work without one.
- The skill's reference guides are in
  `${ANGEL_CARTRIDGE_DIR}/skills/reverse-engineer-anything/references/`.

The operator's task names a target (a path, app, package or URL), what to
understand about it, or both. When it asks for a version of the feature,
finish by building it in the workspace from the evidence, and say which
conclusions the evidence shows and which are inference.
