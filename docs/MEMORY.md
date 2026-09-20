# Repository memory

The cockpit records command outcomes in the experience ledger. Dossier compiles
recurring commands, failures and the last-session thread into workspace-specific
facts. `/dossier` shows the artifact and which facts meet the configured belief
threshold for model context.

From the source checkout, with Node.js and Python 3 installed:

```sh
node scripts/runtime/repo-dossier.mjs --refresh
```

The default stores are `~/.angel0/experience/ledger.jsonl`, `~/.angel0/cut/` and
`~/.angel0/dossier/`. `ANGEL_EXPERIENCE_LOG`, `ANGEL_CUT_DIR` and
`ANGEL_DOSSIER_DIR` override them. The graph is stored inside the Dossier output
directory. CLI overrides are `--ledger`, `--cut`, `--out` and `--graph`.

Facts require recurrence across sessions. The compiler retains evidence and
provenance; the native reader withholds low-confidence or stale facts. A command
recommended by Dossier cannot reinforce its own success. `--refresh` reads
records and publishes artifacts; it does not execute recorded commands.

Caddy separately captures verified command recipes and failure hazards during
tool execution, then selects relevant entries for later turns.

Implementation: [Dossier compiler](../scripts/runtime/repo-dossier.mjs),
[native reader](../cockpit/src/knowledge/dossier.rs), [Caddy](../cockpit/src/knowledge/caddy.rs).

[Local workers](WORKERS.md) documents probing, skill proposals, scheduling and feedback.
