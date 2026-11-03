# Backup and peer watch

angelX keeps everything it learns under `~/.angelX`. Two facilities make
that state durable across machines without naming any operator's hosts in
the public build: both are **generic slots** the operator fills in.

## Backup: tiered snapshots

```sh
angelX --backup run [--tier core|bulk|all] [--to TARGET]
angelX --backup restore [--from TARGET] [--snapshot NAME] [--force]
angelX --backup status [--from TARGET]
angelX --backup prune --keep N [--from TARGET]   # local targets only
```

The state tree mixes the irreplaceable (sessions, memories, atlas,
knowledge — megabytes) with the bulky and regenerable (loop-experiments,
trajectories — gigabytes), so snapshots are tiered:

| Tier | Contents |
|---|---|
| `core` (default) | sessions, memories, atlas, knowledge, caddy, loops, goals, continual-harness, top-level `*.json`/`*.toml` |
| `bulk` | core + trajectories, experience, cut, rl, world-rewards, yukon-strategy, wish-paint |
| `all` | everything except `proc/`, caches, and `*.err`/`*.log` droppings |

Each snapshot is a directory under `<target>/snapshots/` whose unchanged
files are hardlinks into the previous snapshot — keeping history costs
only changed bytes. A `manifest.json` records a SHA-256 per file; the
snapshot is assembled in an `.incomplete-` directory and renamed into
place only when whole, and `<target>/latest` is a symlink repointed
atomically. Restore verifies every hash before publishing, publishes
atomically, and refuses to overwrite a differing live file without
`--force`.

### The target slot

A target is a local path (including a mounted share) or
`[user@]host:/abs/path` (rsync over ssh — a bare host uses your
ssh_config alias — with `--link-dest` so unchanged
files hardlink on the far side too). Resolution order: `--to`/`--from`,
then `ANGEL_BACKUP_TARGET` / `ANGEL_BACKUP_TIER`, then
`~/.angelX/backup.json`:

```json
{"schema": "angelX.backup/v1", "target": "you@yourhost:/srv/angel-backups", "tier": "core"}
```

Unconfigured, backup is dormant: commands say so and change nothing.
Remote restore is two steps: rsync a snapshot directory down, then
restore with `--from <local target>`.

## Peer watch: see your other instances

Every cockpit publishes a small heartbeat (host, pid, workspace, session,
activity, CPU/MEM/GPU load) to a shared **peer directory** and reads the
others'. Peer updates feed the miniworld quietly; arrivals, activity changes,
and silences stay out of the transcript. `/peers` lists the details on demand,
and transitions remain available in debug logs. When no external `overwatch`
fleet probe exists, the status bar's fleet numbers fall back to the live
peers' own reported load.

The directory is another generic slot: `ANGEL_PEER_DIR`, else
`~/.angelX/peers.json`:

```json
{"schema": "angelX.peers/v1", "dir": "/path/shared/by/every/machine"}
```

Default: `~/.angelX/peers/` — instances on one machine see each other
with zero setup. Point the slot at any directory every machine can reach
(a tailnet share, NFS, a synced folder on a fleet host) and the same
code watches across machines. A peer silent for 90 s counts as gone, so
a crashed instance never needs cleanup from its peers.

## Guardian: resurrect a dead peer's session

Peer watch notices a silence. The guardian answers it:

```sh
angel --guardian [--watch-only] [--once] [--stale-secs N] [--driver ROUTE]
angel --resurrect <session-id> --workspace DIR [--driver ROUTE]
```

A guardian is a headless `angel` on a rig with a checkout of the dead
peer's workspace. It publishes its own heartbeat
(`guardian-sleeping` / `guardian-watching` / `guardian-resurrecting` —
zero model tokens while it sleeps) and scans the same peer directory.
A peer silent past the stale horizon (default 180 s), holding a
session, whose workspace is a git checkout on the guardian's rig,
becomes a candidate. Running `--guardian` at all is the consent;
`--watch-only` detects and logs without acting, and `--once` scans a
single time (cron-friendly) instead of looping. `--driver ROUTE`
(or `ANGEL_GUARDIAN_DRIVER`) picks the model route.

Because resurrection is automatic, it is fenced:

- no candidate while a **live** peer holds the same session;
- exactly one guardian wins an atomic `create_new` claim
  (`<instance>.claim.json` in the peer directory); a claim older than
  30 minutes with no outcome may be stolen by the next guardian;
- a finished attempt writes `<instance>.resurrection.json`, so the
  same death is never answered twice — `/peers` shows the recent ones;
- guardians never resurrect guardians.

The winner spawns `angel --resurrect` as a child against the dead
peer's session with an assess-repair-continue brief. `/peers` and the
world's sleeping giant by the keep mirror the state. Run one guardian
per rig, for example as a systemd user unit:

```ini
# ~/.config/systemd/user/angel-guardian.service
[Unit]
Description=angelX guardian — resurrect dead peers' sessions
After=network-online.target

[Service]
ExecStart=/path/to/angel --guardian
WorkingDirectory=/path/to/checkout
Restart=on-failure
RestartSec=30

[Install]
WantedBy=default.target
```
