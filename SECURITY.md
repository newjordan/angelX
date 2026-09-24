# Security policy

## Supported surface

Security fixes target the latest published source release of the ordinary
terminal cockpit (`cockpit/`, built as `angel` and its `angel-sandbox` helper)
and its headless `angel --task-json` runner, on Linux x86_64 with glibc. Other
platforms are not release-qualified. The internal fleet, research, benchmark,
and historical tooling in the full checkout is not a supported security
boundary.

## Sandbox and approval defaults

- Interactive action approval and Landlock are on by default. Landlock is an
  accident guard between projects, not a cage for the model: every seat's shell
  gets the workspace, scratch/cache/install roots, the GPU device nodes and the
  network. There is no read-only or reduced-capability posture for models; see
  "Operable models" in [CONTRIBUTING.md](CONTRIBUTING.md). Shell tool spawns
  re-exec through `angel-sandbox` (or the cockpit itself with
  `ANGEL_SANDBOX_HELPER=self`).
- `--yolo`, `ANGEL_YOLO=1`, or a live `/yolo on` deliberately disables approval,
  Landlock, shared subprocess/HTTP timeouts, child-environment scrubbing, hook
  denials, and effect/network gates. Never use it for untrusted or scored work.
- Headless task mode rejects in-turn `--accept-cmd`. Run verifiers after the
  cockpit exits, in a separate secret-free, networkless, read-only sandbox.
- Bubblewrap controls namespaces and mounts; it is not a cgroup or resource
  governor. Scored attempts belong in a fresh OS-level sandbox or container with
  finite memory/PID/CPU limits and an explicit environment allowlist.
- Keep provider credentials outside task workspaces. The launcher does not load
  broad home-directory env files for headless commands; use an explicitly
  trusted `ANGEL_RUNNER_ENV_FILE` or an allowlisted parent process.

Treat task prompts, repositories, model output, tool arguments, attachments,
and verifier output as untrusted input.

## Reporting a vulnerability

Report security issues privately through the GitHub repository's Security tab
If private vulnerability reporting is unavailable,
contact the maintainer through GitHub first. Do not put exploit details,
credentials, held-out tasks, or private rollout data in a public issue.

Include the affected version or commit (`angel --version` and
`angel --build-info --json`), operating system, launch mode, whether `--yolo`
was active, a minimal reproduction, and the boundary you expected to hold.
Remove API keys and private model or fleet endpoints from logs before sending.

## Release evidence

Source releases are produced and checked by the gate described in
[docs/release-evidence.md](docs/release-evidence.md); its supply-chain pins
live in `release/supply-chain-policy.json`.
