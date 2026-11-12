#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'

export const RELEASE_SCHEMA = 'angelX-source-release-evidence/v1'
// The archive is the ordinary terminal cockpit and its headless task runtime.
export const RELEASE_SCOPE = 'ordinary-terminal-cockpit-source'
export const SUPPORTED_CARGO_TARGET = 'x86_64-unknown-linux-gnu'

// Files consumed by compile-time include_* macros (release build and unit
// tests). Keep these explicit: selecting all of cockpit/ also publishes
// operator-only skills, personas, fleet metadata, design work, and source
// artwork that the runner neither builds nor needs. The release test derives
// the include set from the packaged crates and fails when this list drifts.
export const REQUIRED_COCKPIT_EMBEDDED_FILES = Object.freeze([
  'cockpit/assets/dungeon/bosses/archive-drowned-reeve.boss',
  'cockpit/assets/dungeon/bosses/deep-blight.boss',
  'cockpit/assets/dungeon/bosses/deep-brine-wyrm.boss',
  'cockpit/assets/dungeon/bosses/deep-coal-glutton.boss',
  'cockpit/assets/dungeon/bosses/deep-drowned-toll.boss',
  'cockpit/assets/dungeon/bosses/deep-glow-drinker.boss',
  'cockpit/assets/dungeon/bosses/deep-mist-mother.boss',
  'cockpit/assets/dungeon/bosses/deep-seam-swallower.boss',
  'cockpit/assets/dungeon/bosses/deep-snare-queen.boss',
  'cockpit/assets/dungeon/bosses/lawn-flamingo.boss',
  'cockpit/assets/dungeon/cards/ember-kindle.card',
  'cockpit/assets/dungeon/cards/ember-pillar.card',
  'cockpit/assets/dungeon/cards/ember-wave.card',
  'cockpit/assets/dungeon/cards/far-brass-rain.card',
  'cockpit/assets/dungeon/cards/far-sightline.card',
  'cockpit/assets/dungeon/cards/far-steady.card',
  'cockpit/assets/dungeon/cards/gale-power.card',
  'cockpit/assets/dungeon/cards/gale-shackle.card',
  'cockpit/assets/dungeon/cards/gale-windrun.card',
  'cockpit/assets/dungeon/cards/horizon-rifle.card',
  'cockpit/assets/dungeon/cards/kilnheart-staff.card',
  'cockpit/assets/dungeon/cards/reedwake-bow.card',
  'cockpit/assets/dungeon/templates/card-find.card',
  'cockpit/assets/dungeon/templates/effect-shot.vfx',
  'cockpit/assets/dungeon/templates/effect-spell.vfx',
  'cockpit/assets/dungeon/templates/effect-worn.vfx',
  'cockpit/assets/dungeon/templates/foe-chief.boss',
  'cockpit/assets/dungeon/templates/foe-guardian.boss',
  'cockpit/assets/dungeon/templates/pastime.pastime',
  'cockpit/assets/dungeon/templates/room-great.room',
  'cockpit/assets/dungeon/templates/room-small.room',
  'cockpit/assets/dungeon/templates/wish-shot.wish',
  'cockpit/assets/dungeon/templates/wish-spell.wish',
  'cockpit/assets/dungeon/templates/wish-worn.wish',
  'cockpit/assets/dungeon/wish-art/frost-fletching.art',
  'docs/KILN-RECIPE.md',
  'cockpit/assets/dungeon/cards/barrow-warden.card',
  'cockpit/assets/dungeon/cards/bone-totem.card',
  'cockpit/assets/dungeon/cards/brute-force.card',
  'cockpit/assets/dungeon/cards/chain-haul.card',
  'cockpit/assets/dungeon/cards/chapel-cincture.card',
  'cockpit/assets/dungeon/cards/cleaver.card',
  'cockpit/assets/dungeon/cards/college-orb.card',
  'cockpit/assets/dungeon/cards/crescent-quiver.card',
  'cockpit/assets/dungeon/cards/deeps-harness.card',
  'cockpit/assets/dungeon/cards/elixir.card',
  'cockpit/assets/dungeon/cards/flask-bandolier.card',
  'cockpit/assets/dungeon/cards/forge-matchcord.card',
  'cockpit/assets/dungeon/cards/gate-aegis.card',
  'cockpit/assets/dungeon/cards/grave-axes.card',
  'cockpit/assets/dungeon/cards/grave-staff.card',
  'cockpit/assets/dungeon/cards/great-maul.card',
  'cockpit/assets/dungeon/cards/groundslam.card',
  'cockpit/assets/dungeon/cards/meat-hook.card',
  'cockpit/assets/dungeon/cards/miasma-flask.card',
  'cockpit/assets/dungeon/cards/oathshield.card',
  'cockpit/assets/dungeon/cards/quickfire.card',
  'cockpit/assets/dungeon/cards/raise-ghoul.card',
  'cockpit/assets/dungeon/cards/rookery-longbow.card',
  'cockpit/assets/dungeon/cards/rot.card',
  'cockpit/assets/dungeon/cards/round-table-blade.card',
  'cockpit/assets/dungeon/cards/sigil-staff.card',
  'cockpit/assets/dungeon/cards/smithys-dragonmouth.card',
  'cockpit/assets/dungeon/cards/soul-rip.card',
  'cockpit/assets/dungeon/cards/storm-hammer.card',
  'cockpit/assets/dungeon/cards/table-gauntlet.card',
  'cockpit/assets/dungeon/cards/the-ball.card',
  'cockpit/assets/dungeon/cards/vestal-censer.card',
  'cockpit/assets/dungeon/questlines/heart.txt',
  'cockpit/assets/dungeon/wish-art/boots.art',
  'cockpit/assets/dungeon/wish-art/breastplate.art',
  'cockpit/assets/dungeon/wish-art/bright-blade.art',
  'cockpit/assets/dungeon/wish-art/buckler.art',
  'cockpit/assets/dungeon/wish-art/falling-star.art',
  'cockpit/assets/dungeon/wish-art/firebolt.art',
  'cockpit/assets/dungeon/wish-art/gauntlet.art',
  'cockpit/assets/dungeon/wish-art/great-helm.art',
  'cockpit/assets/dungeon/wish-art/hand-cannon.art',
  'cockpit/assets/dungeon/wish-art/mending-light.art',
  'cockpit/assets/dungeon/wish-art/plain-frame.art',
  'cockpit/assets/dungeon/wish-art/spark-ring.art',
  'cockpit/assets/dungeon/wish-art/ultimate.art',
  'cockpit/assets/dungeon/wish-art/war-crossbow.art',
  'cockpit/assets/dungeon/wish-art/yew-bow.art',
  'cockpit/assets/fonts/ShareTechMono-Regular.ttf',
  'cockpit/web-bundle/build.rs',
  'cockpit/native/skill_router.c',
  'tests/cockpit/fixtures/swarm-routing-golden.bin',
  'crates/term/native/splitter.c',
  'crates/term/native/sixel.c',
  'scripts/runtime/repo-recon.js',
  'scripts/runtime/test-runner-report.mjs',
  'lib/test-runner/report.mjs',
  'lib/club/moa-preferences.json',
  'lib/tools/definitions.json',
  'lib/control/slash-help.json',
  'lib/knowledge/lesson-routing.json',
  'cockpit/book/gh_navigation.tsv',
  'cockpit/book/and_session.book',
  'cockpit/book/ch_edits.book',
  'cockpit/book/ed_media.book',
  'cockpit/book/of_patches.book',
  'cockpit/book/sh_shell.book',
  'cockpit/book/th_team.book',
  'cockpit/book/the_sight.book',
  'cockpit/book/wh_world.book',
  'cockpit/book/with_build.book',
  'cockpit/book/t_personality.book',
  'cockpit/book/z_brevity.book',
  'cockpit/book/ou_checkpoints.book',
  'cockpit/book/d4_angles.book',
  'cockpit/book/o_orchestration.book',
  'cockpit/book/d56_replies.book',
  'cockpit/book/ar_seats.book',
  'cockpit/book/d4567_briefs.book',
  'cockpit/book/ing_drivers.book',
  'cockpit/book/d456_knowledge.book',
  'cockpit/book/d5_frames.book',
  'cockpit/book/n_environment.book',
  'cockpit/book/ow_ledgers.book',
  'cockpit/book/m_method.book',
  'cockpit/book/r_relentless.book',
  'cockpit/book/u_skills.book',
  'cockpit/book/d467_receipts.book',
  'cockpit/book/d45_iteration.book',
  'cockpit/book/d6_long_run.book',
  'cockpit/book/er_loop.book',
  'cockpit/book/s_sources.book',
  'cockpit/book/d3_roles.book',
  'cockpit/book/y_types.book',
  'cockpit/book/p_processes.book',
  'cockpit/book/w_workflow.book',
  'cockpit/book/x_execution.book',
  'cockpit/book/d46_recovery.book',
  'cockpit/book/v_verification.book',
  'cockpit/book/codex_sol_primary.book',
  'cockpit/book/codex_sol_shelf_ii.book',
  'cockpit/book/codex_sol_shelf_iii.book',
  'cockpit/book/codex_sol_context.book',
  'cockpit/book/competition_primary.book',
  'cockpit/book/competition_shelf.book',
  'cockpit/book/connected_primary.book',
  'cockpit/book/connected_shelf.book',
  'cockpit/book/labyrinth_primary.book',
  'cockpit/book/labyrinth_shelf.book',
  'cockpit/live/rooms-rules/rooms-art.json',
  'cockpit/live/showcase-squire/pack.json',
  'cockpit/live/showcase-card-motion/pack.json',
  'cockpit/research/labyrinth/lab.py',
  'cockpit/research/labyrinth/dashboard.html',
  'cockpit/research/labyrinth/upstream/.github/workflows/tests.yml',
  'cockpit/research/labyrinth/upstream/.gitignore',
  'cockpit/research/labyrinth/upstream/CHANGELOG.md',
  'cockpit/research/labyrinth/upstream/LICENSE',
  'cockpit/research/labyrinth/upstream/MAINTAINING.md',
  'cockpit/research/labyrinth/upstream/README.md',
  'cockpit/research/labyrinth/upstream/SKILL.md',
  'cockpit/research/labyrinth/upstream/agents/openai.yaml',
  'cockpit/research/labyrinth/upstream/assets/campaign.svg',
  'cockpit/research/labyrinth/upstream/assets/dashboard-board.png',
  'cockpit/research/labyrinth/upstream/assets/dashboard-frontier.png',
  'cockpit/research/labyrinth/upstream/assets/dashboard-graph.png',
  'cockpit/research/labyrinth/upstream/assets/dashboard-map.png',
  'cockpit/research/labyrinth/upstream/assets/dashboard-sota.png',
  'cockpit/research/labyrinth/upstream/assets/hero.svg',
  'cockpit/research/labyrinth/upstream/assets/loop.svg',
  'cockpit/research/labyrinth/upstream/assets/social-preview.png',
  'cockpit/research/labyrinth/upstream/assets/tiers.svg',
  'cockpit/research/labyrinth/upstream/docs/codex-testing.md',
  'cockpit/research/labyrinth/upstream/docs/tutorial.md',
  'cockpit/research/labyrinth/upstream/examples/README.md',
  'cockpit/research/labyrinth/upstream/examples/prompts.md',
  'cockpit/research/labyrinth/upstream/examples/triangle-counts/README.md',
  'cockpit/research/labyrinth/upstream/examples/triangle-counts/events.jsonl',
  'cockpit/research/labyrinth/upstream/examples/triangle-counts/exhaustive_n8.py',
  'cockpit/research/labyrinth/upstream/examples/triangle-counts/knowledge.json',
  'cockpit/research/labyrinth/upstream/examples/triangle-counts/make_example.py',
  'cockpit/research/labyrinth/upstream/examples/triangle-counts/sota.json',
  'cockpit/research/labyrinth/upstream/references/campaigns.md',
  'cockpit/research/labyrinth/upstream/references/codex.md',
  'cockpit/research/labyrinth/upstream/references/compute.md',
  'cockpit/research/labyrinth/upstream/references/lessons.md',
  'cockpit/research/labyrinth/upstream/references/loop.md',
  'cockpit/research/labyrinth/upstream/references/saturation.md',
  'cockpit/research/labyrinth/upstream/references/schema.md',
  'cockpit/research/labyrinth/upstream/templates/briefs/attack.md',
  'cockpit/research/labyrinth/upstream/templates/briefs/referee.md',
  'cockpit/research/labyrinth/upstream/templates/briefs/writer.md',
  'cockpit/research/labyrinth/upstream/templates/dashboard.html',
  'cockpit/research/labyrinth/upstream/templates/lab.py',
  'cockpit/research/labyrinth/upstream/tests/test_codex.py',
  'cockpit/research/labyrinth/upstream/tests/test_lab.py',
  'cockpit/research/labyrinth/upstream/tests/test_skill.py',
  'cockpit/research/labyrinth/upstream/tools/codex_smoke.py',
  'cockpit/research/labyrinth/upstream/tools/make_graphics.py',
  'cockpit/research/labyrinth/upstream/tools/screenshots.py',
  // Offline Sloptomizer options are embedded; no experimental checkout dependency.
  'cockpit/research/sloptomizer/runner.py',
  'cockpit/research/sloptomizer/relations.py',
  'cockpit/research/sloptomizer/UPSTREAM.json',
  'cockpit/research/sloptomizer/autoresearch/__init__.py',
  'cockpit/research/sloptomizer/autoresearch/gepa/__init__.py',
  'cockpit/research/sloptomizer/autoresearch/gepa/population.py',
  'cockpit/research/sloptomizer/autoresearch/gepa/select.py',
  'cockpit/research/sloptomizer/orchestrator/__init__.py',
  'cockpit/research/sloptomizer/orchestrator/self_improvement/__init__.py',
  'cockpit/research/sloptomizer/orchestrator/self_improvement/bandits.py',
  'cockpit/research/sloptomizer/orchestrator/self_improvement/stats.py',
  'cockpit/research/sloptomizer/orchestrator/micro_llm/__init__.py',
  'cockpit/research/sloptomizer/orchestrator/micro_llm/core.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_crystal_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_taxonomy_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_vocab_analogy_codec.py',
  'cockpit/research/sloptomizer/orchestrator/caseops_vocab_tensor_codec.py',
  'cockpit/modules/agent.toml',
  'cockpit/modules/artifacts.toml',
  'cockpit/modules/core.toml',
  'cockpit/modules/graph.toml',
  'cockpit/modules/image.toml',
  'cockpit/modules/shell.toml',
  'cockpit/personas/architect/PERSONA.md',
  'cockpit/personas/code-help/PERSONA.md',
  'cockpit/personas/librarian/PERSONA.md',
  'cockpit/personas/reviewer/PERSONA.md',
  'cockpit/personas/security/PERSONA.md',
  'cockpit/personas/skeptic/PERSONA.md',
  'cockpit/personas/speed-freak/PERSONA.md',
  'cockpit/personas/treebeard/PERSONA.md',
  'cockpit/skills/benchmark-inference/SKILL.md',
  'cockpit/skills/competition-loop/SKILL.md',
  'cockpit/skills/gpu-fleet-recon/SKILL.md',
  'cockpit/skills/hf-model-ops/SKILL.md',
  'cockpit/skills/navigate-code/SKILL.md',
  'cockpit/skills/plan/SKILL.md',
  'cockpit/skills/quantize-model/SKILL.md',
  'cockpit/skills/requesting-code-review/SKILL.md',
  'cockpit/skills/research-answer/SKILL.md',
  'cockpit/skills/self-modify/SKILL.md',
  'cockpit/skills/serve-local-llm/SKILL.md',
  'cockpit/skills/simplify-code/SKILL.md',
  'cockpit/skills/systematic-debugging/SKILL.md',
  'cockpit/skills/test-driven-development/SKILL.md',
  'cockpit/skills/verify-changes/SKILL.md',
  'cockpit/skills/write-skill/SKILL.md',
  'cockpit/fixtures/sheets/golden-devils.png',
  'cockpit/fixtures/sheets/spiked-pavise.png',
  'cockpit/fixtures/observatory/manifest-v2.json',
  // Painted crawl textures and sprites are compiled into the game renderer.
  'cockpit/assets/crawl/ceiling.ink',
  'cockpit/assets/crawl/door.ink',
  'cockpit/assets/crawl/floor_flags.ink',
  'cockpit/assets/crawl/sprites/banner.ink',
  'cockpit/assets/crawl/sprites/boulder.ink',
  'cockpit/assets/crawl/sprites/campfire_a.ink',
  'cockpit/assets/crawl/sprites/campfire_b.ink',
  'cockpit/assets/crawl/sprites/chest.ink',
  'cockpit/assets/crawl/sprites/chest_open.ink',
  'cockpit/assets/crawl/sprites/hand_sword_idle.ink',
  'cockpit/assets/crawl/sprites/hand_sword_raise.ink',
  'cockpit/assets/crawl/sprites/hand_sword_swing.ink',
  'cockpit/assets/crawl/sprites/hand_torch_a.ink',
  'cockpit/assets/crawl/sprites/hand_torch_b.ink',
  'cockpit/assets/crawl/sprites/lectern.ink',
  'cockpit/assets/crawl/sprites/sconce_a.ink',
  'cockpit/assets/crawl/sprites/sconce_b.ink',
  'cockpit/assets/crawl/stairs.ink',
  'cockpit/assets/crawl/wall_archive.ink',
  'cockpit/assets/crawl/wall_crypt.ink',
  'cockpit/assets/crawl/wall_fungal.ink',
  'cockpit/assets/crawl/wall_keep.ink',
  'cockpit/assets/crawl/wall_mine.ink',
  'cockpit/assets/crawl/wall_stone.ink',
  'cockpit/assets/crawl/wall_unknown.ink',
  'cockpit/assets/excalibur/rise.png',
  'cockpit/assets/realm/ambient/chapel-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/chapel-mountain-v2.png',
  'cockpit/assets/realm/ambient/gatehouse-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/gatehouse-mountain-v2.png',
  'cockpit/assets/realm/ambient/keep-waterfall-source.png',
  'cockpit/assets/realm/ambient/keep-waterfall.png',
  'cockpit/assets/realm/ambient/motion.json',
  'cockpit/assets/realm/ambient/observatory-mountain-source.png',
  'cockpit/assets/realm/ambient/observatory-mountain.png',
  'cockpit/assets/realm/ambient/rookery-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/rookery-mountain-v2.png',
  'cockpit/assets/realm/ambient/round-table-mountain-source.png',
  'cockpit/assets/realm/ambient/round-table-mountain.png',
  'cockpit/assets/realm/ambient/scriptorium-mountain-v2-source.png',
  'cockpit/assets/realm/ambient/scriptorium-mountain-v2.png',
  'cockpit/assets/realm/ambient/smithy-mountain-source.png',
  'cockpit/assets/realm/ambient/smithy-mountain.png',
  'cockpit/assets/realm/inhabitants/archivist.png',
  'cockpit/assets/realm/inhabitants/courier.png',
  'cockpit/assets/realm/inscriptions/accelerando-spoken-here.json',
  'cockpit/assets/realm/palette.json',
  'cockpit/assets/realm/v2/interiors/station-round-table.png',
  // The Delve's built-in cards, bosses, phrasebook, chorus and realm wishes.
  'cockpit/assets/dungeon/bosses/archive-bellkeeper.boss',
  'cockpit/assets/dungeon/bosses/cinderjaw.boss',
  'cockpit/assets/dungeon/bosses/crypt-relic-liege.boss',
  'cockpit/assets/dungeon/bosses/crypt-tax-keeper.boss',
  'cockpit/assets/dungeon/bosses/crypt-tithe-marshal.boss',
  'cockpit/assets/dungeon/bosses/fungal-rootwarden.boss',
  'cockpit/assets/dungeon/bosses/fungal-spore-crown.boss',
  'cockpit/assets/dungeon/bosses/hellforge-ash-castellan.boss',
  'cockpit/assets/dungeon/bosses/hellforge-chain-keeper.boss',
  'cockpit/assets/dungeon/bosses/hellforge-free-captain.boss',
  'cockpit/assets/dungeon/bosses/hellforge-fuel-reeve.boss',
  'cockpit/assets/dungeon/bosses/hellforge-slag-sovereign.boss',
  'cockpit/assets/dungeon/bosses/iron-tusk-chief.boss',
  'cockpit/assets/dungeon/bosses/late-fee-leviathan.boss',
  'cockpit/assets/dungeon/bosses/mother-of-spores.boss',
  'cockpit/assets/dungeon/bosses/red-pick-chief.boss',
  'cockpit/assets/dungeon/bosses/salvage-claimant.boss',
  'cockpit/assets/dungeon/bosses/shoggoth-heart.boss',
  'cockpit/assets/dungeon/bosses/the-bone-choir.boss',
  'cockpit/assets/dungeon/bosses/the-foreman.boss',
  'cockpit/assets/dungeon/bosses/the-index.boss',
  'cockpit/assets/dungeon/bosses/unknown-memory-warden.boss',
  'cockpit/assets/dungeon/bosses/unknown-silence-warden.boss',
  'cockpit/assets/dungeon/bosses/waxen-warden.boss',
  'cockpit/assets/dungeon/bosses/grinwick.boss',
  'cockpit/assets/dungeon/bosses/the-big-ripper.boss',
  'cockpit/assets/dungeon/cards/awakened-fork.card',
  'cockpit/assets/dungeon/cards/axe-spin.card',
  'cockpit/assets/dungeon/cards/bodkin-heads.card',
  'cockpit/assets/dungeon/cards/bomb.card',
  'cockpit/assets/dungeon/cards/bone.card',
  'cockpit/assets/dungeon/cards/bow.card',
  'cockpit/assets/dungeon/cards/censer.card',
  'cockpit/assets/dungeon/cards/choir-crumb.card',
  'cockpit/assets/dungeon/cards/cinder-wick.card',
  'cockpit/assets/dungeon/cards/crossbow.card',
  'cockpit/assets/dungeon/cards/dodge-roll.card',
  'cockpit/assets/dungeon/cards/ember-quiver.card',
  'cockpit/assets/dungeon/cards/ember.card',
  'cockpit/assets/dungeon/cards/fae-dagger.card',
  'cockpit/assets/dungeon/cards/gem.card',
  'cockpit/assets/dungeon/cards/glass-needle.card',
  'cockpit/assets/dungeon/cards/gold.card',
  'cockpit/assets/dungeon/cards/handgonne.card',
  'cockpit/assets/dungeon/cards/heart.card',
  'cockpit/assets/dungeon/cards/hearth-bell.card',
  'cockpit/assets/dungeon/cards/holy-water.card',
  'cockpit/assets/dungeon/cards/kite-shield.card',
  'cockpit/assets/dungeon/cards/knights-blade.card',
  'cockpit/assets/dungeon/cards/lantern-mote.card',
  'cockpit/assets/dungeon/cards/leech-fang.card',
  'cockpit/assets/dungeon/cards/mail.card',
  'cockpit/assets/dungeon/cards/marrow-sip.card',
  'cockpit/assets/dungeon/cards/ore.card',
  'cockpit/assets/dungeon/cards/pendragon-sceptre.card',
  'cockpit/assets/dungeon/cards/potion.card',
  'cockpit/assets/dungeon/cards/quick-draw.card',
  'cockpit/assets/dungeon/cards/reed-flute.card',
  'cockpit/assets/dungeon/cards/rime-whistle.card',
  'cockpit/assets/dungeon/cards/salt-thread.card',
  'cockpit/assets/dungeon/cards/scale.card',
  'cockpit/assets/dungeon/cards/swift-spurs.card',
  'cockpit/assets/dungeon/cards/thunder-scroll.card',
  'cockpit/assets/dungeon/cards/twin-axes.card',
  'cockpit/assets/dungeon/cards/twin-string.card',
  'cockpit/assets/dungeon/cards/wax.card',
  'cockpit/assets/dungeon/cards/body-armour.card',
  'cockpit/assets/dungeon/cards/cabbage-storm.card',
  'cockpit/assets/dungeon/cards/fart-bombs.card',
  'cockpit/assets/dungeon/cards/fork.card',
  'cockpit/assets/dungeon/cards/hedge-maze.card',
  'cockpit/assets/dungeon/cards/iron-helm.card',
  'cockpit/assets/dungeon/cards/the-imposture.card',
  'cockpit/assets/dungeon/phrasebook.txt',
  'cockpit/assets/dungeon/voices/chorus.txt',
  'cockpit/assets/realm/houses/houses.json',
  'cockpit/assets/realm/props/dragon_skull.obj',
  'cockpit/assets/realm/props/knight_statue.obj',
  'cockpit/assets/realm/props/siege_perilous.obj',
  'cockpit/assets/realm/wishes/beacon-tower.wish',
  'cockpit/assets/realm/wishes/fellowship-stone.wish',
  'cockpit/assets/realm/wishes/grail-chapel.wish',
  'cockpit/assets/realm/wishes/hall-of-the-dragonslayers.wish',
  'cockpit/assets/realm/wishes/miners-lodge.wish',
  'cockpit/assets/realm/wishes/tavern.wish',
  'cockpit/assets/realm/wishes/wayside-shrine.wish',
  'cockpit/assets/together/ember-spell.json',
  'cockpit/assets/together/spark-wand.json',
  // Telemetry tables the cockpit compiles in for cost/calibration/store caps.
  'docs/telemetry/model-calibration.toml',
  'docs/telemetry/prices.toml',
  'docs/telemetry/store-caps.toml',
  // Synthetic wire fixtures consumed by the provider parser tests.
  'cockpit/fixtures/usage/native-usage-wire-v1.json',
  'cockpit/fixtures/usage/supplemental-chat-usage-wire-v1.json',
  'cockpit/assets/dungeon/wish-art/chain.art',
  'cockpit/assets/dungeon/wish-art/fart.art',
  'cockpit/assets/dungeon/wish-art/fleet.art',
  'cockpit/assets/dungeon/wish-art/fork.art',
  'cockpit/assets/dungeon/wish-art/heavy.art',
  'cockpit/assets/dungeon/wish-art/iron.art',
  'cockpit/assets/dungeon/wish-art/leech.art',
  'cockpit/assets/dungeon/wish-art/mend.art',
  'cockpit/assets/dungeon/wish-art/morningstar.art',
  'cockpit/assets/dungeon/wish-art/pierce.art',
  'cockpit/assets/dungeon/wish-art/plain-hound.art',
  'cockpit/assets/dungeon/wish-art/powder.art',
  'cockpit/assets/dungeon/wish-art/quick.art',
  'cockpit/assets/dungeon/wish-art/ricochet.art',
  'cockpit/assets/dungeon/wish-art/seeker.art',
  'cockpit/assets/dungeon/wish-art/stout.art',
  'cockpit/assets/dungeon/wish-art/twin.art',
  'cockpit/assets/dungeon/wish-art/volley.art',
  'cockpit/assets/dungeon/bosses/the-mire-king.boss',
  'cockpit/assets/dungeon/cards/note-stream.card',
  'cockpit/assets/dungeon/cards/thrown-stones.card',
  'cockpit/assets/dungeon/learned-20261014.txt',
  'cockpit/assets/dungeon/questlines/armor.txt',
  'cockpit/assets/dungeon/questlines/bom-erinys.txt',
  'cockpit/assets/dungeon/questlines/bom-ivy.txt',
  'cockpit/assets/dungeon/questlines/bom-kekkit.txt',
  'cockpit/assets/dungeon/questlines/bom-machines.txt',
  'cockpit/assets/dungeon/questlines/bom-ratfolk.txt',
  'cockpit/assets/dungeon/questlines/bom-steel.txt',
  'cockpit/assets/dungeon/questlines/bom-still.txt',
  'cockpit/assets/dungeon/questlines/bounce.txt',
  'cockpit/assets/dungeon/questlines/chain.txt',
  'cockpit/assets/dungeon/questlines/fart.txt',
  'cockpit/assets/dungeon/questlines/fire.txt',
  'cockpit/assets/dungeon/questlines/fork.txt',
  'cockpit/assets/dungeon/questlines/gas.txt',
  'cockpit/assets/dungeon/questlines/heavy.txt',
  'cockpit/assets/dungeon/questlines/homing.txt',
  'cockpit/assets/dungeon/questlines/mend.txt',
  'cockpit/assets/dungeon/questlines/mine.txt',
  'cockpit/assets/dungeon/questlines/orbit.txt',
  'cockpit/assets/dungeon/questlines/pierce.txt',
  'cockpit/assets/dungeon/questlines/powder.txt',
  'cockpit/assets/dungeon/questlines/quick.txt',
  'cockpit/assets/dungeon/questlines/shield.txt',
  'cockpit/assets/dungeon/questlines/shots.txt',
  'cockpit/assets/dungeon/questlines/speed.txt',
  'cockpit/assets/dungeon/questlines/stout.txt',
  'cockpit/assets/dungeon/questlines/vamp.txt',
  'cockpit/assets/dungeon/questlines/volley.txt',
  'cockpit/assets/dungeon/questlines/wall.txt',
  'cockpit/fixtures/anthropic/sonnet-5-5-tool-stream.sse',
  'docs/WORLD-3D.md',
  'scripts/live/bend.mjs',
  'scripts/boss-effect-slots.json',
  'scripts/boss-library-registry.json',
  'cockpit/assets/dungeon/cards/abyssal-bellhelm.card',
  'cockpit/assets/dungeon/cards/abyssal-compass.card',
  'cockpit/assets/dungeon/cards/adder-hourglass.card',
  'cockpit/assets/dungeon/cards/arcneedle-spindle.card',
  'cockpit/assets/dungeon/cards/arcwell-salute.card',
  'cockpit/assets/dungeon/cards/astral-barspoon.card',
  'cockpit/assets/dungeon/cards/bankshot-medallion.card',
  'cockpit/assets/dungeon/cards/bastion-bell.card',
  'cockpit/assets/dungeon/cards/bellkeepers-mail.card',
  'cockpit/assets/dungeon/cards/blackberry-buckler.card',
  'cockpit/assets/dungeon/cards/blackcap-reserve.card',
  'cockpit/assets/dungeon/cards/blacktide-censer.card',
  'cockpit/assets/dungeon/cards/bloodlust.card',
  'cockpit/assets/dungeon/cards/borrowed-minute.card',
  'cockpit/assets/dungeon/cards/borrowed-regular.card',
  'cockpit/assets/dungeon/cards/bottled-squall-honey.card',
  'cockpit/assets/dungeon/cards/brace.card',
  'cockpit/assets/dungeon/cards/breachbell-tonic.card',
  'cockpit/assets/dungeon/cards/brineheart-fig.card',
  'cockpit/assets/dungeon/cards/candlefish-skewer.card',
  'cockpit/assets/dungeon/cards/cellar-key.card',
  'cockpit/assets/dungeon/cards/choir-grenade.card',
  'cockpit/assets/dungeon/cards/choirwire-coil.card',
  'cockpit/assets/dungeon/cards/cinder-parasol.card',
  'cockpit/assets/dungeon/cards/cinder-seahorse.card',
  'cockpit/assets/dungeon/cards/cinder-service.card',
  'cockpit/assets/dungeon/cards/cinder-tithe.card',
  'cockpit/assets/dungeon/cards/cinder-votive.card',
  'cockpit/assets/dungeon/cards/cinderpear-bisque.card',
  'cockpit/assets/dungeon/cards/cinderpin-escapement.card',
  'cockpit/assets/dungeon/cards/coatcheck-comet.card',
  'cockpit/assets/dungeon/cards/comet-dumpling.card',
  'cockpit/assets/dungeon/cards/comet-glass.card',
  'cockpit/assets/dungeon/cards/comet-siphon.card',
  'cockpit/assets/dungeon/cards/confetti-antidote.card',
  'cockpit/assets/dungeon/cards/copperhead-siphon.card',
  'cockpit/assets/dungeon/cards/copperhead-spool.card',
  'cockpit/assets/dungeon/cards/copperhead-volley.card',
  'cockpit/assets/dungeon/cards/corkscrew-comet.card',
  'cockpit/assets/dungeon/cards/crabclaw-pauldron.card',
  'cockpit/assets/dungeon/cards/crescendo-clef.card',
  'cockpit/assets/dungeon/cards/crown-of-cinders.card',
  'cockpit/assets/dungeon/cards/cuckoo-breach.card',
  'cockpit/assets/dungeon/cards/deadletter-flare.card',
  'cockpit/assets/dungeon/cards/dialkeepers-ring.card',
  'cockpit/assets/dungeon/cards/doppelganger.card',
  'cockpit/assets/dungeon/cards/dragonfruit-kebab.card',
  'cockpit/assets/dungeon/cards/drydock-salute.card',
  'cockpit/assets/dungeon/cards/duelists-mainspring.card',
  'cockpit/assets/dungeon/cards/echo-strike.card',
  'cockpit/assets/dungeon/cards/eclipse-punchcard.card',
  'cockpit/assets/dungeon/cards/eclipse-toll.card',
  'cockpit/assets/dungeon/cards/embercrab-bento.card',
  'cockpit/assets/dungeon/cards/emberwake-charge.card',
  'cockpit/assets/dungeon/cards/escapement-heart.card',
  'cockpit/assets/dungeon/cards/eventide-fan.card',
  'cockpit/assets/dungeon/cards/eventide-key.card',
  'cockpit/assets/dungeon/cards/excommunicate.card',
  'cockpit/assets/dungeon/cards/eye-of-the-squall.card',
  'cockpit/assets/dungeon/cards/firefly-prism.card',
  'cockpit/assets/dungeon/cards/firefly-sorbet.card',
  'cockpit/assets/dungeon/cards/foxstep-spindle.card',
  'cockpit/assets/dungeon/cards/frostbite-ampoule.card',
  'cockpit/assets/dungeon/cards/fulgurite-fork.card',
  'cockpit/assets/dungeon/cards/fulgurite-spurs.card',
  'cockpit/assets/dungeon/cards/furnace-curtain.card',
  'cockpit/assets/dungeon/cards/gallows-bell.card',
  'cockpit/assets/dungeon/cards/gallows-sun.card',
  'cockpit/assets/dungeon/cards/ghostlight-lantern.card',
  'cockpit/assets/dungeon/cards/glasshouse-mercy.card',
  'cockpit/assets/dungeon/cards/glassway-key.card',
  'cockpit/assets/dungeon/cards/glassway-ticket.card',
  'cockpit/assets/dungeon/cards/gravewire-spool.card',
  'cockpit/assets/dungeon/cards/halo-breach.card',
  'cockpit/assets/dungeon/cards/harpoon-ratchet.card',
  'cockpit/assets/dungeon/cards/hearthglass-pear.card',
  'cockpit/assets/dungeon/cards/hurl-axe.card',
  'cockpit/assets/dungeon/cards/javelin.card',
  'cockpit/assets/dungeon/cards/juxtapose.card',
  'cockpit/assets/dungeon/cards/keelbreaker.card',
  'cockpit/assets/dungeon/cards/keeltooth-spur.card',
  'cockpit/assets/dungeon/cards/kettle-eclipse.card',
  'cockpit/assets/dungeon/cards/kilnwing-reprieve.card',
  'cockpit/assets/dungeon/cards/kraken-corkscrew.card',
  'cockpit/assets/dungeon/cards/lantern-jury.card',
  'cockpit/assets/dungeon/cards/last-call-buckler.card',
  'cockpit/assets/dungeon/cards/last-call-carom.card',
  'cockpit/assets/dungeon/cards/last-call-censer.card',
  'cockpit/assets/dungeon/cards/last-call-fork.card',
  'cockpit/assets/dungeon/cards/last-call-saucers.card',
  'cockpit/assets/dungeon/cards/last-call-thunder.card',
  'cockpit/assets/dungeon/cards/last-light.card',
  'cockpit/assets/dungeon/cards/last-orders-ladle.card',
  'cockpit/assets/dungeon/cards/last-platform-signal.card',
  'cockpit/assets/dungeon/cards/lastlight-postmark.card',
  'cockpit/assets/dungeon/cards/leap.card',
  'cockpit/assets/dungeon/cards/lobster-caliper.card',
  'cockpit/assets/dungeon/cards/lobster-telegraph.card',
  'cockpit/assets/dungeon/cards/mantis-tongs.card',
  'cockpit/assets/dungeon/cards/masquerade-matchbox.card',
  'cockpit/assets/dungeon/cards/matchstick-ferry.card',
  'cockpit/assets/dungeon/cards/mercy-lantern.card',
  'cockpit/assets/dungeon/cards/mercy-procession.card',
  'cockpit/assets/dungeon/cards/mercy-trigger.card',
  'cockpit/assets/dungeon/cards/meteor-cordial.card',
  'cockpit/assets/dungeon/cards/mirage-foil.card',
  'cockpit/assets/dungeon/cards/mirror-swap.card',
  'cockpit/assets/dungeon/cards/monks-patience.card',
  'cockpit/assets/dungeon/cards/mooncrab-cracker.card',
  'cockpit/assets/dungeon/cards/moonmilk-flask.card',
  'cockpit/assets/dungeon/cards/moonshine-prism.card',
  'cockpit/assets/dungeon/cards/moth-sanctuary.card',
  'cockpit/assets/dungeon/cards/mothglass-lens.card',
  'cockpit/assets/dungeon/cards/mothlight-censer.card',
  'cockpit/assets/dungeon/cards/mothwake-lantern.card',
  'cockpit/assets/dungeon/cards/mothwing-tea.card',
  'cockpit/assets/dungeon/cards/needle-cathedra.card',
  'cockpit/assets/dungeon/cards/needlewake-sight.card',
  'cockpit/assets/dungeon/cards/needlewake-spool.card',
  'cockpit/assets/dungeon/cards/noon-shatter.card',
  'cockpit/assets/dungeon/cards/orchard-hourglass.card',
  'cockpit/assets/dungeon/cards/orchard-of-ashes.card',
  'cockpit/assets/dungeon/cards/orrery-collapse.card',
  'cockpit/assets/dungeon/cards/orrery-heart.card',
  'cockpit/assets/dungeon/cards/oyster-aegis.card',
  'cockpit/assets/dungeon/cards/oyster-mortgage.card',
  'cockpit/assets/dungeon/cards/pallbearer-lanterns.card',
  'cockpit/assets/dungeon/cards/parlour-labyrinth.card',
  'cockpit/assets/dungeon/cards/pearl-diver-fork.card',
  'cockpit/assets/dungeon/cards/pearlwake-flask.card',
  'cockpit/assets/dungeon/cards/permafrost-flywheel.card',
  'cockpit/assets/dungeon/cards/phantom-cue.card',
  'cockpit/assets/dungeon/cards/phantom-lance.card',
  'cockpit/assets/dungeon/cards/phoenix-matchbook.card',
  'cockpit/assets/dungeon/cards/phoenix-suture.card',
  'cockpit/assets/dungeon/cards/pinwheel-break.card',
  'cockpit/assets/dungeon/cards/plague-divers-rebreather.card',
  'cockpit/assets/dungeon/cards/plague-of-frogs.card',
  'cockpit/assets/dungeon/cards/plague-switchboard.card',
  'cockpit/assets/dungeon/cards/powder-saint.card',
  'cockpit/assets/dungeon/cards/powderplum-tart.card',
  'cockpit/assets/dungeon/cards/pressure-seal.card',
  'cockpit/assets/dungeon/cards/quarantine-star.card',
  'cockpit/assets/dungeon/cards/queenstorm-decree.card',
  'cockpit/assets/dungeon/cards/railglass-prism.card',
  'cockpit/assets/dungeon/cards/railjaw-stapler.card',
  'cockpit/assets/dungeon/cards/railwake-prism.card',
  'cockpit/assets/dungeon/cards/railway-shaker.card',
  'cockpit/assets/dungeon/cards/reaver-axes.card',
  'cockpit/assets/dungeon/cards/redoubt-tuning-fork.card',
  'cockpit/assets/dungeon/cards/redshift-cuff.card',
  'cockpit/assets/dungeon/cards/redshift-ripcord.card',
  'cockpit/assets/dungeon/cards/ricochet-decanter.card',
  'cockpit/assets/dungeon/cards/rimewheel-bearing.card',
  'cockpit/assets/dungeon/cards/rimewheel-escapement.card',
  'cockpit/assets/dungeon/cards/rook-mortar.card',
  'cockpit/assets/dungeon/cards/rookstep-sigil.card',
  'cockpit/assets/dungeon/cards/roulette-spur.card',
  'cockpit/assets/dungeon/cards/rustwing-locket.card',
  'cockpit/assets/dungeon/cards/saints-saltcellar.card',
  'cockpit/assets/dungeon/cards/saints-tourniquet.card',
  'cockpit/assets/dungeon/cards/sanguine-ampoule.card',
  'cockpit/assets/dungeon/cards/sawtooth-menu.card',
  'cockpit/assets/dungeon/cards/scorpion-tap.card',
  'cockpit/assets/dungeon/cards/siege-samovar.card',
  'cockpit/assets/dungeon/cards/spark-staff.card',
  'cockpit/assets/dungeon/cards/spirit-lance.card',
  'cockpit/assets/dungeon/cards/starfall-warrant.card',
  'cockpit/assets/dungeon/cards/starfall-writ.card',
  'cockpit/assets/dungeon/cards/stock-blade.card',
  'cockpit/assets/dungeon/cards/stock-maul.card',
  'cockpit/assets/dungeon/cards/stock-staff.card',
  'cockpit/assets/dungeon/cards/storm-receipt.card',
  'cockpit/assets/dungeon/cards/storm-tab.card',
  'cockpit/assets/dungeon/cards/stormglass-dial.card',
  'cockpit/assets/dungeon/cards/stormglass-eclair.card',
  'cockpit/assets/dungeon/cards/stormglass-vespers.card',
  'cockpit/assets/dungeon/cards/stormglass-writ.card',
  'cockpit/assets/dungeon/cards/stormpipe-organ.card',
  'cockpit/assets/dungeon/cards/stormrail-conductor.card',
  'cockpit/assets/dungeon/cards/stormrail-ticket.card',
  'cockpit/assets/dungeon/cards/sundial-mortar.card',
  'cockpit/assets/dungeon/cards/sunken-mint.card',
  'cockpit/assets/dungeon/cards/swashbuckle.card',
  'cockpit/assets/dungeon/cards/switchglass-escapement.card',
  'cockpit/assets/dungeon/cards/tempest-skate.card',
  'cockpit/assets/dungeon/cards/thorn-testament.card',
  'cockpit/assets/dungeon/cards/thundercomb.card',
  'cockpit/assets/dungeon/cards/thunderhead-gavel.card',
  'cockpit/assets/dungeon/cards/thunderhead-phial.card',
  'cockpit/assets/dungeon/cards/tollkeepers-visor.card',
  'cockpit/assets/dungeon/cards/umbra-switchfork.card',
  'cockpit/assets/dungeon/cards/undertow-anchor.card',
  'cockpit/assets/dungeon/cards/undertow-bell.card',
  'cockpit/assets/dungeon/cards/undertow-benediction.card',
  'cockpit/assets/dungeon/cards/undertow-carillon.card',
  'cockpit/assets/dungeon/cards/undertow-coil.card',
  'cockpit/assets/dungeon/cards/undertow-crown.card',
  'cockpit/assets/dungeon/cards/vault.card',
  'cockpit/assets/dungeon/cards/vaultkey-spur.card',
  'cockpit/assets/dungeon/cards/velvet-intermission.card',
  'cockpit/assets/dungeon/cards/velvet-rebuttal.card',
  'cockpit/assets/dungeon/cards/velvet-rope-ticket.card',
  'cockpit/assets/dungeon/cards/velvet-rope.card',
  'cockpit/assets/dungeon/cards/vesper-apiary.card',
  'cockpit/assets/dungeon/cards/vesper-bell.card',
  'cockpit/assets/dungeon/cards/vesper-knell.card',
  'cockpit/assets/dungeon/cards/viper-decanter.card',
  'cockpit/assets/dungeon/cards/vipers-hourglass.card',
  'cockpit/assets/dungeon/cards/votive-aegis.card',
  'cockpit/assets/dungeon/cards/wake-engine.card',
  'cockpit/assets/dungeon/cards/wake-lantern.card',
  'cockpit/assets/dungeon/cards/war-lance.card',
  'cockpit/assets/dungeon/cards/wayfarer-stitch.card',
  'cockpit/assets/dungeon/cards/wayfarer-wick.card',
  'cockpit/assets/dungeon/cards/whiteout-muzzle.card',
  'cockpit/assets/dungeon/cards/whiteout-parasol.card',
  'cockpit/assets/dungeon/cards/widow-kite.card',
  'cockpit/assets/dungeon/cards/widowglass-ampoule.card',
  'cockpit/assets/dungeon/cards/widowglass-hourglass.card',
  'cockpit/assets/dungeon/cards/widows-spindle.card',
  'cockpit/assets/dungeon/cards/widows-toast.card',
  'cockpit/assets/dungeon/cards/widowseed-cage.card',
  'cockpit/assets/dungeon/cards/winterfang-bayonet.card',
  'cockpit/assets/dungeon/cards/wishbone-reticle.card',
  'cockpit/assets/dungeon/cards/witching-turntable.card',
  'cockpit/assets/dungeon/cards/zero-hour-bell.card',
  'cockpit/assets/dungeon/voices/goblin.txt',
  'cockpit/assets/kingdom/items/ember-edge.card',
  'cockpit/assets/kingdom/items/hearth-wardstone.card',
  'cockpit/assets/kingdom/items/iron-oathblade.card',
  'cockpit/assets/kingdom/items/mooncrystal-staff.card',
  'cockpit/assets/kingdom/items/prospectors-pick.card',
  'cockpit/assets/kingdom/items/runebound-aegis.card',
  'delve/kingdom/frontier.json',
])

export const REQUIRED_RUNTIME_HELPERS = Object.freeze([
  'scripts/runtime/background-build.py',
  'cockpit/book/catalog-manifest.json',
  'scripts/runtime/publish-book-catalog.py',
  'tests/python/test_book_catalog_generation.py',
  'tests/python/book_catalog_reader_model.py',
  'scripts/check/convert-book-chapter.py',
  'tests/python/test_book_converter.py',
  'tests/cockpit/fixtures/book-migration-batch2.json',
  'tests/cockpit/fixtures/book-migration-batch3.json',
  'tests/cockpit/fixtures/book-migration-vendor.json',
  'tests/cockpit/fixtures/book-migration-batch5.json',
  'tests/cockpit/fixtures/runner-output-cases.json',
  'tests/cockpit/fixtures/verification-cases.json',
  'tests/cockpit/fixtures/comp-watch-path-cases.json',
  'tests/cockpit/fixtures/lean-schema-cases.json',
  'tests/cockpit/fixtures/original-book-hints.json',
  'tests/cockpit/fixtures/original-tool-notes.json',
  'tests/cockpit/fixtures/tool-error-cases.json',
  'tests/cockpit/fixtures/command-idle-cases.json',
  'tests/cockpit/fixtures/sleep-guard-cases.json',
  'tests/cockpit/fixtures/board-classification-cases.json',
  'tests/cockpit/fixtures/skill-routing-cases.json',
  'tests/cockpit/fixtures/provider-retry-cases.json',
  'scripts/check/check-live-modules.sh',
  'scripts/dev/systems.mjs',
  'cockpit/live/peers/module.json',
  'cockpit/live/peers/index.js',
  'cockpit/live/peers/format.js',
  'cockpit/live/realm/module.json',
  'cockpit/live/realm/index.js',
  'cockpit/live/agent/module.json',
  'cockpit/live/agent/index.js',
  'cockpit/live/world-help/module.json',
  'cockpit/live/world-help/index.js',
  'cockpit/live/ui/module.json',
  'cockpit/live/ui/index.js',
  'cockpit/live/game/module.json',
  'cockpit/live/game/index.js',
  'cockpit/live/tool-router/module.json',
  'cockpit/live/tool-router/index.js',
  'cockpit/live/tool-router/rank.js',
  'cockpit/live/errors/module.json',
  'cockpit/live/errors/index.js',
  'cockpit/live/errors/errors.bend',
  'cockpit/live/errors/LAWS.bend',
  'cockpit/live/errors/PROOF.bend',
  'cockpit/live/errors/errors.compiled.js',
  'cockpit/live/errors/bend-build.json',
  'cockpit/live/characters/module.json',
  'cockpit/live/characters/index.js',
  'cockpit/live/characters/looks.js',
  'cockpit/live/showcase-squire/module.json',
  'cockpit/live/showcase-squire/index.js',
  'cockpit/live/showcase-squire/pack.json',
  'cockpit/live/showcase-card-motion/module.json',
  'cockpit/live/showcase-card-motion/index.js',
  'cockpit/live/showcase-card-motion/pack.json',
  'cockpit/live/abilities/module.json',
  'cockpit/live/abilities/index.js',
  'cockpit/live/character-motion/module.json',
  'cockpit/live/character-motion/index.js',
  'cockpit/live/character-graphs/module.json',
  'cockpit/live/character-graphs/index.js',
  'cockpit/live/ability-code/module.json',
  'cockpit/live/ability-code/index.js',
  'cockpit/live/ability-code/spells.js',
  'cockpit/live/ability-code/fairies.bend',
  'cockpit/live/ability-code/LAWS.bend',
  'cockpit/live/ability-code/PROOF.bend',
  'cockpit/live/ability-code/fairies.compiled.js',
  'cockpit/live/ability-code/bend-build.json',
  'cockpit/live/deathmatch-rules/module.json',
  'cockpit/live/deathmatch-rules/index.js',
  'cockpit/live/deathmatch-rules/deathmatch-rules.bend',
  'cockpit/live/deathmatch-rules/LAWS.bend',
  'cockpit/live/deathmatch-rules/PROOF.bend',
  'cockpit/live/deathmatch-rules/deathmatch-rules.compiled.js',
  'cockpit/live/deathmatch-rules/bend-build.json',
  'cockpit/live/delve-ossuary/module.json',
  'cockpit/live/delve-ossuary/index.js',
  'cockpit/live/delve-ossuary/ossuary.bend',
  'cockpit/live/delve-ossuary/LAWS.bend',
  'cockpit/live/delve-ossuary/PROOF.bend',
  'cockpit/live/delve-ossuary/ossuary.compiled.js',
  'cockpit/live/delve-ossuary/bend-build.json',
  'cockpit/live/bannerpeak-rules/module.json',
  'cockpit/live/bannerpeak-rules/index.js',
  'cockpit/live/bannerpeak-rules/nums.bend',
  'cockpit/live/bannerpeak-rules/map.bend',
  'cockpit/live/bannerpeak-rules/tiles.bend',
  'cockpit/live/bannerpeak-rules/layout.bend',
  'cockpit/live/bannerpeak-rules/answer.bend',
  'cockpit/live/bannerpeak-rules/boss.bend',
  'cockpit/live/bannerpeak-rules/bannerpeak-rules.bend',
  'cockpit/live/bannerpeak-rules/LAWS.bend',
  'cockpit/live/bannerpeak-rules/PROOF.bend',
  'cockpit/live/bannerpeak-rules/bannerpeak-rules.compiled.js',
  'cockpit/live/bannerpeak-rules/bend-build.json',
  'cockpit/live/lightgallop-rules/module.json',
  'cockpit/live/lightgallop-rules/index.js',
  'cockpit/live/lightgallop-rules/math.bend',
  'cockpit/live/lightgallop-rules/sl.bend',
  'cockpit/live/lightgallop-rules/hs.bend',
  'cockpit/live/lightgallop-rules/rs.bend',
  'cockpit/live/lightgallop-rules/terrain.bend',
  'cockpit/live/lightgallop-rules/build.bend',
  'cockpit/live/lightgallop-rules/level.bend',
  'cockpit/live/lightgallop-rules/road.bend',
  'cockpit/live/lightgallop-rules/rider.bend',
  'cockpit/live/lightgallop-rules/world.bend',
  'cockpit/live/lightgallop-rules/session.bend',
  'cockpit/live/lightgallop-rules/bot.bend',
  'cockpit/live/lightgallop-rules/lightgallop-rules.bend',
  'cockpit/live/lightgallop-rules/LAWS.bend',
  'cockpit/live/lightgallop-rules/PROOF.bend',
  'cockpit/live/lightgallop-rules/lightgallop-rules.compiled.js',
  'cockpit/live/lightgallop-rules/bend-build.json',
  'cockpit/live/effort/module.json',
  'cockpit/live/effort/index.js',
  'cockpit/live/effort/effort.bend',
  'cockpit/live/effort/LAWS.bend',
  'cockpit/live/effort/PROOF.bend',
  'cockpit/live/effort/effort.compiled.js',
  'cockpit/live/effort/bend-build.json',
  'cockpit/live/reasoning/module.json',
  'cockpit/live/reasoning/index.js',
  'cockpit/live/reasoning/reasoning.bend',
  'cockpit/live/reasoning/LAWS.bend',
  'cockpit/live/reasoning/PROOF.bend',
  'cockpit/live/reasoning/reasoning.compiled.js',
  'cockpit/live/reasoning/bend-build.json',
  'cockpit/live/verify-policy/module.json',
  'cockpit/live/verify-policy/index.js',
  'cockpit/live/verify-policy/verify-policy.bend',
  'cockpit/live/verify-policy/LAWS.bend',
  'cockpit/live/verify-policy/PROOF.bend',
  'cockpit/live/verify-policy/verify-policy.compiled.js',
  'cockpit/live/verify-policy/bend-build.json',
  'cockpit/live/promotion/module.json',
  'cockpit/live/promotion/index.js',
  'cockpit/live/promotion/promotion.bend',
  'cockpit/live/promotion/LAWS.bend',
  'cockpit/live/promotion/PROOF.bend',
  'cockpit/live/promotion/promotion.compiled.js',
  'cockpit/live/promotion/bend-build.json',
  'cockpit/live/race3d-rules/module.json',
  'cockpit/live/race3d-rules/index.js',
  'cockpit/live/race3d-rules/race3d-rules.bend',
  'cockpit/live/race3d-rules/LAWS.bend',
  'cockpit/live/race3d-rules/PROOF.bend',
  'cockpit/live/race3d-rules/race3d-rules.compiled.js',
  'cockpit/live/race3d-rules/bend-build.json',
  'cockpit/live/swarm-route/module.json',
  'cockpit/live/swarm-route/index.js',
  'cockpit/live/swarm-route/swarm-route.bend',
  'cockpit/live/swarm-route/LAWS.bend',
  'cockpit/live/swarm-route/PROOF.bend',
  'cockpit/live/swarm-route/swarm-route.compiled.js',
  'cockpit/live/swarm-route/bend-build.json',
  'cockpit/live/worker-health/module.json',
  'cockpit/live/worker-health/index.js',
  'cockpit/live/worker-health/worker-health.bend',
  'cockpit/live/worker-health/LAWS.bend',
  'cockpit/live/worker-health/PROOF.bend',
  'cockpit/live/worker-health/worker-health.compiled.js',
  'cockpit/live/worker-health/bend-build.json',
  'cockpit/live/compaction-window/module.json',
  'cockpit/live/compaction-window/index.js',
  'cockpit/live/compaction-window/compaction-window.bend',
  'cockpit/live/compaction-window/LAWS.bend',
  'cockpit/live/compaction-window/PROOF.bend',
  'cockpit/live/compaction-window/compaction-window.compiled.js',
  'cockpit/live/compaction-window/bend-build.json',
  'cockpit/live/frontier/module.json',
  'cockpit/live/frontier/index.js',
  'cockpit/live/frontier/frontier.bend',
  'cockpit/live/frontier/LAWS.bend',
  'cockpit/live/frontier/PROOF.bend',
  'cockpit/live/frontier/frontier.compiled.js',
  'cockpit/live/frontier/bend-build.json',
  'cockpit/live/rewards/module.json',
  'cockpit/live/rewards/index.js',
  'cockpit/live/rewards/rewards.bend',
  'cockpit/live/rewards/LAWS.bend',
  'cockpit/live/rewards/PROOF.bend',
  'cockpit/live/rewards/rewards.compiled.js',
  'cockpit/live/rewards/bend-build.json',
  'cockpit/live/episode-reducer/module.json',
  'cockpit/live/episode-reducer/index.js',
  'cockpit/live/episode-reducer/episode-reducer.bend',
  'cockpit/live/episode-reducer/LAWS.bend',
  'cockpit/live/episode-reducer/PROOF.bend',
  'cockpit/live/episode-reducer/episode-reducer.compiled.js',
  'cockpit/live/episode-reducer/bend-build.json',
  'cockpit/live/lane-schedule/module.json',
  'cockpit/live/lane-schedule/index.js',
  'cockpit/live/lane-schedule/lane-schedule.bend',
  'cockpit/live/lane-schedule/LAWS.bend',
  'cockpit/live/lane-schedule/PROOF.bend',
  'cockpit/live/lane-schedule/lane-schedule.compiled.js',
  'cockpit/live/lane-schedule/bend-build.json',
  'cockpit/live/lanternward-rules/module.json',
  'cockpit/live/lanternward-rules/index.js',
  'cockpit/live/lanternward-rules/lanternward-rules.bend',
  'cockpit/live/lanternward-rules/b.bend',
  'cockpit/live/lanternward-rules/bo.bend',
  'cockpit/live/lanternward-rules/bs.bend',
  'cockpit/live/lanternward-rules/c.bend',
  'cockpit/live/lanternward-rules/cn.bend',
  'cockpit/live/lanternward-rules/ct.bend',
  'cockpit/live/lanternward-rules/cx.bend',
  'cockpit/live/lanternward-rules/dr.bend',
  'cockpit/live/lanternward-rules/fe.bend',
  'cockpit/live/lanternward-rules/fo.bend',
  'cockpit/live/lanternward-rules/fu.bend',
  'cockpit/live/lanternward-rules/fx.bend',
  'cockpit/live/lanternward-rules/g.bend',
  'cockpit/live/lanternward-rules/gd.bend',
  'cockpit/live/lanternward-rules/gr.bend',
  'cockpit/live/lanternward-rules/ht.bend',
  'cockpit/live/lanternward-rules/ix.bend',
  'cockpit/live/lanternward-rules/k.bend',
  'cockpit/live/lanternward-rules/ke.bend',
  'cockpit/live/lanternward-rules/kt.bend',
  'cockpit/live/lanternward-rules/l.bend',
  'cockpit/live/lanternward-rules/m.bend',
  'cockpit/live/lanternward-rules/n.bend',
  'cockpit/live/lanternward-rules/nr.bend',
  'cockpit/live/lanternward-rules/pr.bend',
  'cockpit/live/lanternward-rules/pt.bend',
  'cockpit/live/lanternward-rules/q.bend',
  'cockpit/live/lanternward-rules/r.bend',
  'cockpit/live/lanternward-rules/se.bend',
  'cockpit/live/lanternward-rules/sg.bend',
  'cockpit/live/lanternward-rules/sh.bend',
  'cockpit/live/lanternward-rules/st.bend',
  'cockpit/live/lanternward-rules/tg.bend',
  'cockpit/live/lanternward-rules/tk.bend',
  'cockpit/live/lanternward-rules/w.bend',
  'cockpit/live/lanternward-rules/z.bend',
  'cockpit/live/lanternward-rules/LAWS.bend',
  'cockpit/live/lanternward-rules/PROOF.bend',
  'cockpit/live/lanternward-rules/lanternward-rules.compiled.js',
  'cockpit/live/lanternward-rules/bend-build.json',
  'cockpit/live/swarm-waves/module.json',
  'cockpit/live/swarm-waves/index.js',
  'cockpit/live/swarm-waves/swarm-waves.bend',
  'cockpit/live/swarm-waves/LAWS.bend',
  'cockpit/live/swarm-waves/PROOF.bend',
  'cockpit/live/swarm-waves/swarm-waves.compiled.js',
  'cockpit/live/swarm-waves/bend-build.json',
  'cockpit/live/swarm-mandate/module.json',
  'cockpit/live/swarm-mandate/index.js',
  'cockpit/live/swarm-mandate/swarm-mandate.bend',
  'cockpit/live/swarm-mandate/LAWS.bend',
  'cockpit/live/swarm-mandate/PROOF.bend',
  'cockpit/live/swarm-mandate/swarm-mandate.compiled.js',
  'cockpit/live/swarm-mandate/bend-build.json',
  'cockpit/live/board-reducer/module.json',
  'cockpit/live/board-reducer/index.js',
  'cockpit/live/board-reducer/board-reducer.bend',
  'cockpit/live/board-reducer/LAWS.bend',
  'cockpit/live/board-reducer/PROOF.bend',
  'cockpit/live/board-reducer/board-reducer.compiled.js',
  'cockpit/live/board-reducer/bend-build.json',
  'cockpit/live/cost/module.json',
  'cockpit/live/cost/index.js',
  'cockpit/live/cost/cost.bend',
  'cockpit/live/cost/LAWS.bend',
  'cockpit/live/cost/PROOF.bend',
  'cockpit/live/cost/cost.compiled.js',
  'cockpit/live/cost/bend-build.json',
  'cockpit/live/verify-plan/module.json',
  'cockpit/live/verify-plan/index.js',
  'cockpit/live/verify-plan/verify-plan.bend',
  'cockpit/live/verify-plan/LAWS.bend',
  'cockpit/live/verify-plan/PROOF.bend',
  'cockpit/live/verify-plan/verify-plan.compiled.js',
  'cockpit/live/verify-plan/bend-build.json',
  'cockpit/live/deep-cut/module.json',
  'cockpit/live/deep-cut/index.js',
  'cockpit/live/deep-cut/deep-cut.bend',
  'cockpit/live/deep-cut/LAWS.bend',
  'cockpit/live/deep-cut/PROOF.bend',
  'cockpit/live/deep-cut/deep-cut.compiled.js',
  'cockpit/live/deep-cut/bend-build.json',
  'cockpit/live/result-classify/module.json',
  'cockpit/live/result-classify/index.js',
  'cockpit/live/result-classify/result-classify.bend',
  'cockpit/live/result-classify/LAWS.bend',
  'cockpit/live/result-classify/PROOF.bend',
  'cockpit/live/result-classify/result-classify.compiled.js',
  'cockpit/live/result-classify/bend-build.json',
  'cockpit/live/competition-mode/module.json',
  'cockpit/live/competition-mode/index.js',
  'cockpit/live/competition-mode/competition-mode.bend',
  'cockpit/live/competition-mode/LAWS.bend',
  'cockpit/live/competition-mode/PROOF.bend',
  'cockpit/live/competition-mode/competition-mode.compiled.js',
  'cockpit/live/competition-mode/bend-build.json',
  'cockpit/live/patterns-reducer/module.json',
  'cockpit/live/patterns-reducer/index.js',
  'cockpit/live/patterns-reducer/patterns-reducer.bend',
  'cockpit/live/patterns-reducer/LAWS.bend',
  'cockpit/live/patterns-reducer/PROOF.bend',
  'cockpit/live/patterns-reducer/patterns-reducer.compiled.js',
  'cockpit/live/patterns-reducer/bend-build.json',
  'scripts/live/bend.mjs',
  'tests/scripts/bend-modules.test.mjs',
  'cockpit/live/ability-sounds/module.json',
  'cockpit/live/ability-sounds/index.js',
  'cockpit/live/ability-sounds/sounds.js',
  'cockpit/live/ability-sounds/spark.wav',
  'cockpit/live/ability-sounds/bloom.wav',
  'cockpit/live/ability-effects/module.json',
  'cockpit/live/ability-effects/index.js',
  'cockpit/live/ability-effects/effects.js',
  'cockpit/live/ability-effects/fairies.js',
  'cockpit/live/ability-effects/sigil.png',
  'cockpit/live/seat-order/module.json',
  'cockpit/live/seat-order/index.js',
  'cockpit/live/seat-order/seat-order.bend',
  'cockpit/live/seat-order/LAWS.bend',
  'cockpit/live/seat-order/PROOF.bend',
  'cockpit/live/seat-order/seat-order.compiled.js',
  'cockpit/live/seat-order/bend-build.json',
  'cockpit/live/moonfall-rules/module.json',
  'cockpit/live/moonfall-rules/index.js',
  'cockpit/live/moonfall-rules/moonfall-rules.bend',
  'cockpit/live/moonfall-rules/model.bend',
  'cockpit/live/moonfall-rules/town.bend',
  'cockpit/live/moonfall-rules/LAWS.bend',
  'cockpit/live/moonfall-rules/PROOF.bend',
  'cockpit/live/moonfall-rules/moonfall-rules.compiled.js',
  'cockpit/live/moonfall-rules/bend-build.json',
  'cockpit/live/rooms-rules/module.json',
  'cockpit/live/rooms-rules/index.js',
  'cockpit/live/rooms-rules/types.bend',
  'cockpit/live/rooms-rules/crypt.bend',
  'cockpit/live/rooms-rules/cavern.bend',
  'cockpit/live/rooms-rules/hellforge.bend',
  'cockpit/live/rooms-rules/archive.bend',
  'cockpit/live/rooms-rules/fungal.bend',
  'cockpit/live/rooms-rules/unknown.bend',
  'cockpit/live/rooms-rules/swamp.bend',
  'cockpit/live/rooms-rules/rooms-rules.bend',
  'cockpit/live/rooms-rules/LAWS.bend',
  'cockpit/live/rooms-rules/PROOF.bend',
  'cockpit/live/rooms-rules/rooms-rules.compiled.js',
  'cockpit/live/rooms-rules/bend-build.json',
  'cockpit/live/rooms-rules/rooms-art.json',
  'cockpit/live/griffinwing-rules/module.json',
  'cockpit/live/griffinwing-rules/index.js',
  'cockpit/live/griffinwing-rules/griffinwing-rules.bend',
  'cockpit/live/griffinwing-rules/LAWS.bend',
  'cockpit/live/griffinwing-rules/PROOF.bend',
  'cockpit/live/griffinwing-rules/griffinwing-rules.compiled.js',
  'cockpit/live/griffinwing-rules/bend-build.json',
  'cockpit/live/layout-rules/module.json',
  'cockpit/live/layout-rules/index.js',
  'cockpit/live/layout-rules/layout-rules.bend',
  'cockpit/live/layout-rules/LAWS.bend',
  'cockpit/live/layout-rules/PROOF.bend',
  'cockpit/live/layout-rules/layout-rules.compiled.js',
  'cockpit/live/layout-rules/bend-build.json',
  'scripts/runtime/worker-paths.mjs',
  'scripts/runtime/worker-lock.mjs',
  'scripts/runtime/bounded-child.mjs',
  'scripts/runtime/bounded-child.py',
  'scripts/runtime/habitsmith.mjs',
  'scripts/runtime/habitsmith-tick.mjs',
  'tests/scripts/habitsmith.test.mjs',
  'scripts/runtime/dossier-tick.mjs',
  'tests/scripts/dossier-tick.test.mjs',
  'tests/scripts/process-lease.test.mjs',
  'lib/control/process-lease.mjs',
  'scripts/runtime/repo-dossier.mjs',
  'tests/scripts/repo-dossier.test.mjs',
  'scripts/runtime/store-caps.mjs',
  'scripts/runtime/causal-loop.mjs',
  'scripts/runtime/cut-evidence.mjs',
  'scripts/runtime/cut-report.mjs',
  'scripts/runtime/moa-report.mjs',
  'tests/python/test_moa_report.py',
  'scripts/runtime/self-map.mjs',
  'lib/self-map/report.mjs',
  'tests/scripts/self-map.test.mjs',
  'scripts/runtime/test-runner-report.mjs',
  'scripts/runtime/build-native-modules.py',
  'lib/test-runner/report.mjs',
  'tests/scripts/test-runner-report.test.mjs',
  'lib/cut/report.mjs',
  'tests/scripts/cut-report.test.mjs',
  'scripts/runtime/repo-search.mjs',
  'lib/repo-search/rules.mjs',
  'tests/scripts/repo-search.test.mjs',
  'scripts/runtime/science-brief.mjs',
  'lib/science/brief.mjs',
  'tests/scripts/science-brief.test.mjs',
  'cockpit/research/sloptomizer/publish_runtime.py',
  'lib/research/CausalGraph.js',
  'lib/research/causal-loop.mjs',
  'lib/evidence/authored-write.mjs',
  'lib/dossier/core.mjs',
  'lib/dossier/probes.mjs',
  'lib/habits/core.mjs',
  'scripts/runtime/private-store-fs.mjs',
  'scripts/runtime/private-store-bridge.py',
  'scripts/runtime/process-start-token.py',
  'scripts/runtime/pxpipe-transform.mjs',
  'scripts/runtime/idle.mjs',
])

const PUBLIC_RELEASE_DOCS = Object.freeze([
  'docs/telemetry/model-calibration-hotload.md',
  'docs/SLOT-ART-IMPORT.md',
  'docs/examples/cartridges/dig/.gitignore',
  'docs/examples/cartridges/dig/bin/digup',
  'docs/examples/cartridges/dig/bin/dig-search',
  'docs/examples/cartridges/dig/cartridge.toml',
  'docs/examples/cartridges/dig/install.sh',
  'docs/examples/cartridges/dig/skills/dig/SKILL.md',
  'docs/images/realm/accelerando-hall.png',
  'docs/FEATURES.md',
  'docs/BACKUP.md',
  'docs/RELEASE_0.2.4.md',
  'docs/images/release-0.2.4.png',
  'docs/RELEASE_0.2.7.md',
  'docs/RELEASE_0.2.85.md',
  'docs/PLUGINS.md',
  'scripts/runtime/install-blaze.py',
  'scripts/delve/spellfx.mjs',
  'docs/images/glass/cockpit-window.png',
  'docs/images/glass/test-card.png',
  'docs/images/glass/cockpit-macos.png',
  'docs/images/glass/blast.png',
  'scripts/perf/repro-blaze-image-freshness.py',
  'scripts/perf/measure-delve-watch-startup.py',
  'scripts/perf/measure-delve-watch-freshness.py',
  'scripts/perf/measure-native-invalidation.py',
  'scripts/check/check-locks.sh',
  'scripts/perf/measure-terminal-ready.py',
  'scripts/perf/measure-dev-rebuild.py',
  'scripts/perf/measure-code-mode-boundary.py',
  'docs/performance/blaze-v8-sandbox-reduction-20261028.md',
  'docs/performance/blaze-upstream-opportunities-20261028.md',
  'docs/performance/blaze-restore-replay-20261028.md',
  'docs/RENDER_FOUNDATION.md',
  'docs/performance/browser-pack-reuse-20261028.md',
  'scripts/perf/audit-startup-evidence-iteration45.py',
  'docs/performance/blaze-callback-boundary-20261028.md',
  'docs/performance/blaze-feature-boundary-20261028.md',
  'docs/HANDS.md',
  'docs/GLASS_TERM.md',
  'docs/performance/dev-loop-20261028.md',
  'docs/performance/launcher-artifact-location-20261029.md',
  'docs/performance/launcher-pin-hash-and-blaze-cwd-20261029.md',
  'docs/performance/native-reload-parallel-20261028.md',
  'docs/examples/cartridges/vfx/.gitignore',
  'docs/examples/cartridges/vfx/LICENSE',
  'docs/examples/cartridges/vfx/README.md',
  'docs/examples/cartridges/vfx/THIRD_PARTY_NOTICES.md',
  'docs/examples/cartridges/vfx/bend/engine.bend',
  'docs/examples/cartridges/vfx/bin/vfx',
  'docs/examples/cartridges/vfx/bin/vfx-catalog',
  'docs/examples/cartridges/vfx/bin/vfx-render',
  'docs/examples/cartridges/vfx/build.sh',
  'docs/examples/cartridges/vfx/cartridge.toml',
  'docs/examples/cartridges/vfx/examples/frost-nova.recipe.json',
  'docs/examples/cartridges/vfx/install.sh',
  'docs/examples/cartridges/vfx/lib/codecs.mjs',
  'docs/examples/cartridges/vfx/lib/main.mjs',
  'docs/examples/cartridges/vfx/lib/recipes.mjs',
  'docs/examples/cartridges/vfx/lib/render.mjs',
  'docs/examples/cartridges/vfx/licenses/Bend-Apache-2.0.txt',
  'docs/examples/cartridges/vfx/package.json',
  'docs/examples/cartridges/vfx/skills/pixel-vfx/SKILL.md',
  'docs/examples/cartridges/vfx/tests/vfx.test.mjs',
  'docs/examples/cartridges/vfx/toolchain.py',
  'docs/GLASS_CORE.md',
  'docs/GLASS_DIALECT.md',
  'docs/REMOTE_GLASS.md',
  'docs/performance/v8-compile-boundary-20261028.md',
  'docs/performance/native-fingerprint-20261028.md',
  'docs/performance/startup-readiness-20261028.md',
  'docs/performance/delve-watch-startup-20261028.md',
  'docs/performance/video-playback-startup-20261028.md',
  'docs/performance/startup-evidence-checkpoint-iteration45.md',
  'docs/performance/showcase-squire-activation-latency.md',
  'docs/RELEASE_0.2.6.md',
  'docs/HEARTGLASS.md',
  'docs/RELEASE_0.2.5.md',
  'docs/images/release-0.2.5.png',
  'docs/images/delve/black-dungeon.png',
  'docs/SHOWCASE.md',
  'docs/images/showcase/wizard-chart.png',
  'docs/images/showcase/forge-model.png',
  'docs/images/showcase/squire-name.png',
  'docs/RELEASE_0.2.3.md',
  'docs/ENGINE.md',
  'docs/images/engine/seats.png',
  'docs/RELEASE_0.2.1.md',
  'docs/RELEASE_0.2.0.md',
  'docs/RELEASE_0.1.9.md',
  'docs/world-adventure.md',
  'docs/world-school.md',
  'docs/GRAPH_GARDEN.md',
  'docs/DELVE.md',
  'docs/DELVE-FEEL-REVIEW.md',
  'docs/KINGDOM.md',
  'docs/SCHOOL.md',
  'docs/ART-V2.md',
  'docs/DELVE-LORE.md',
  'docs/HEART-QUEST.md',
  'docs/DELVE-TEACHERS.md',
  'docs/KINGDOM-OPEN.md',
  'docs/CASTLES.md',
  'docs/BARONY.md',
  'docs/images/realm/castles.png',
  'docs/images/realm/gate.png',
  'docs/images/realm/kings-hall.png',
  'docs/images/realm/joust.png',
  'docs/images/delve/undercroft.png',
  'docs/images/delve/fortune-wheel.png',
  'docs/images/delve/fungal-deep.png',
  'docs/images/delve/tavern.png',
  'docs/images/delve/trophy-hall.png',
  'docs/images/delve/bounties.png',
  'docs/images/crawl/fight.png',
  'docs/images/crawl/chest.png',
  'docs/images/crawl/guardian.png',
  'docs/LABYRINTH.md',
  'docs/images/graph-garden-growing.png',
  'docs/images/graph-garden-live.png',
  'docs/images/graph-garden-planted.png',
  'docs/SELF_MODEL.md',
  'docs/MEMORY.md',
  'docs/MODELS.md',
  'docs/WORKERS.md',
  'docs/COMMANDS.md',
  'docs/CARTRIDGES.md',
  'docs/images/cartridges/slot-dark.png',
  'docs/images/cartridges/slot-light.png',
  'docs/images/cartridges/turn-dark.png',
  'docs/images/cartridges/turn-light.png',
  'docs/images/cartridges/lines-dark.png',
  'docs/images/cartridges/lines-light.png',
  'docs/images/cartridges/cartridges.json',
  'docs/examples/cartridges/lean-kernel/cartridge.toml',
  'docs/examples/cartridges/lean-kernel/bench.sh',
  'docs/examples/cartridges/lean-kernel/fleet.sh',
  'docs/examples/cartridges/rea/cartridge.toml',
  'docs/examples/cartridges/rea/install.sh',
  'docs/examples/cartridges/rea/skills/rea/SKILL.md',
  'docs/images/task.png',
  'docs/images/ui/model-picker.png',
  'docs/images/ui/approval.png',
  'docs/images/ui/receipts.png',
  'docs/images/ui/think-picker.png',
  'docs/images/ui/edit-approval.png',
  'docs/images/ui/diff.png',
  'docs/images/ui/goal.png',
  'docs/images/ui/loop-workshop.png',
  'docs/images/ui/formation.png',
  'docs/images/ui/context.png',
  'docs/images/README.md',
  'docs/images/provenance.json',
  'docs/images/intro.png',
  'docs/images/ui/command-picker.png',
  'docs/images/world.png',
  'docs/images/bench/tokens-time-deepseek.png',
  'docs/images/bench/solved-dark.png',
  'docs/images/bench/solved-light.png',
  'docs/images/bench/score-dark.png',
  'docs/images/bench/score-light.png',
  'docs/images/bench/bars-dark.png',
  'docs/images/bench/bars-light.png',
  'docs/images/bench/seconds-dark.png',
  'docs/images/bench/seconds-light.png',
  'docs/images/bench/context-dark.png',
  'docs/images/bench/context-light.png',
  'docs/images/bench/cache-dark.png',
  'docs/images/bench/cache-light.png',
])

const PUBLIC_RELEASE_NOTICES = Object.freeze([
  'third-party/bend-LICENSE.txt',
  'THIRD_PARTY_NOTICES.md',
  'third-party/openai-codex-LICENSE.txt',
  'third-party/openai-codex-NOTICE.txt',
  'third-party/oh-my-pi-LICENSE.txt',
  'third-party/reasonix-LICENSE.txt',
  'third-party/hermes-agent-LICENSE.txt',
  'third-party/prime-agent-LICENSE.txt',
  'third-party/openscience-LICENSE.txt',
  'third-party/openscience-NOTICE.txt',
  'third-party/labyrinth-exploration-LICENSE.txt',
  'third-party/share-tech-mono-OFL.txt',
  'third-party/sources.json',
])

// Build/runtime data is separate from the exact literal-include inventory,
// but both enter the source identity and are required in source releases.
export const REQUIRED_COCKPIT_BUILD_RUNTIME_FILES = Object.freeze([
  'cockpit/native/activity_classifier.c',
  'cockpit/native/generate_moa_router.py',
  'cockpit/native/moa_router.c',
  'cockpit/native/verify_shell.c',
  'lib/native/native_module.rs',
])

export const RELEASE_PATHS = Object.freeze([
  ...REQUIRED_COCKPIT_BUILD_RUNTIME_FILES,
  '.gitignore',
  'README.md',
  'LICENSE',
  'CONTRIBUTING.md',
  'SECURITY.md',
  ...PUBLIC_RELEASE_DOCS,
  ...PUBLIC_RELEASE_NOTICES,
  ...REQUIRED_RUNTIME_HELPERS,
  'package.json',
  'package-lock.json',
  'rust-toolchain.toml',
  'release/supply-chain-policy.json',
  'release/Dockerfile.clean-builder',
  'bin/angelX',
  'install.sh',
  'scripts/runtime/angel-setup.sh',
  'scripts/release/build-prebuilt.sh',
  'scripts/release/package-prebuilt.sh',
  // The Delve's art scripts, named by the art notes beside each asset set.
  'scripts/arcade-art.py',
  'scripts/arcade-fx-polish.py',
  'scripts/arcade-knight.py',
  'scripts/brasswake-art.py',
  'scripts/brasswake-review.py',
  'scripts/cinder-art.py',
  'scripts/cinder-review.py',
  'scripts/latch-art.py',
  'scripts/mossbell-art.py',
  'scripts/mossbell-compose.py',
  'scripts/mossbell-fit-evidence.py',
  'scripts/test-mossbell-compose.py',
  'scripts/pennon-art.py',
  'scripts/pennon-review.py',
  'scripts/rafterwake-art.py',
  'scripts/rafterwake-clean.py',
  'scripts/rafterwake-review.py',
  'scripts/slot-art-import.py',
  'scripts/slot-art-quality.py',
  'tests/scripts/install.test.mjs',
  'tests/python/test_build_native_modules.py',
  'scripts/check/angel-club-policy.sh',
  'tests/scripts/angel-club-policy.test.mjs',
  'tests/scripts/native-launch.test.mjs',
  'cockpit/Cargo.toml',
  'cockpit/build.rs',
  'cockpit/build_fast.rs',
  'cockpit/bundle_bytes.rs',
  'cockpit/native/Cargo.toml',
  'cockpit/native/build.rs',
  'cockpit/native/build_parallel.rs',
  'cockpit/native/lib.rs',
  'cockpit/web-bundle/Cargo.toml',
  'cockpit/web-bundle/build.rs',
  'cockpit/web-bundle/lib.rs',
  'cockpit/web-bundle/pack_asset.rs',
  // V8's compilation boundary: the cockpit's angel-code-mode path dependency.
  'cockpit/code-mode',
  'cockpit/delve-cart',
  'cockpit/README.md',
  'cockpit/Cargo.lock',
  'cockpit/src',
  'cockpit/examples',
  // The engine's crates: the cockpit's path dependencies (the Delve, its net
  // and server) and the browser client build.rs compiles and embeds.
  'crates',
  'tests/cockpit',
  'tests/README.md',
  'cockpit/assets',
  'cockpit/graphs/council.toml',
  'cockpit/graphs/research-pool.toml',
  'cockpit/graphs/research-write-review.toml',
  'cockpit/graphs/polyglot-departments.toml',
  ...REQUIRED_COCKPIT_EMBEDDED_FILES,
  'cockpit/research/sloptomizer/README.md',
  'cockpit/research/labyrinth/README.md',
  'cockpit/research/labyrinth/schema.md',
  'cockpit/research/labyrinth/UPSTREAM.json',
  'cockpit/docs/COMPETITION_RUNNER.md',
  'cockpit/docs/ENV.md',
  'cockpit/docs/JEV.md',
  'scripts/check/cockpit-source-digest.sh',
  'tests/python/test_cockpit_source_digest.py',
  'scripts/check/check-cockpit-fast.sh',
  'scripts/check/copy-cockpit-test-image.py',
  'scripts/check/check-cockpit-quality.sh',
  'scripts/check/check-active-connections.py',
  'tests/python/test_active_connections.py',
  'tests/python/test_sloptomizer_relations.py',
  'tests/python/test_labyrinth_engine.py',
  'scripts/runtime/trajectory-redact.py',
  'scripts/runtime/private_store_io.py',
  'vendor/dotmax/Cargo.toml',
  'vendor/dotmax/LICENSE-APACHE',
  'vendor/dotmax/LICENSE-MIT',
  'vendor/dotmax/README.md',
  'vendor/dotmax/src',
  // ureq is the cockpit's second local Cargo path dependency (a vendored,
  // patched fork); the archive cannot build offline without it.
  'vendor/ureq/ANGEL_PATCH.md',
  'vendor/ureq/Cargo.toml',
  'vendor/ureq/LICENSE-APACHE',
  'vendor/ureq/LICENSE-MIT',
  'vendor/ureq/README.md',
  'vendor/ureq/src',
  'scripts/release/release-evidence.mjs',
  'tests/scripts/release-evidence.test.mjs',
  'tests/scripts/live-sound.test.mjs',
  'scripts/check/check-legacy-terminal-boundary.sh',
  'scripts/release/verify-release-evidence.mjs',
  'tests/scripts/verify-release-evidence.test.mjs',
  'scripts/release/verify-release-container.mjs',
  'tests/scripts/verify-release-container.test.mjs',
  'scripts/release/verify-release-advisories.mjs',
  'tests/scripts/verify-release-advisories.test.mjs',
  'scripts/release/verify-release-install.mjs',
  'tests/scripts/verify-release-install.test.mjs',
  'scripts/release/verify-release-rollback.mjs',
  'scripts/release/verify-runner-smoke.mjs',
  'tests/scripts/verify-runner-smoke.test.mjs',
  'docs/release-evidence.md',
  'docs/telemetry/trace-schema-v1.json',
])

export const REQUIRED_RELEASE_FILES = Object.freeze([
  ...REQUIRED_COCKPIT_BUILD_RUNTIME_FILES,
  ...PUBLIC_RELEASE_DOCS,
  ...PUBLIC_RELEASE_NOTICES,
  ...REQUIRED_RUNTIME_HELPERS,
  'cockpit/README.md',
  'cockpit/graphs/council.toml',
  'cockpit/graphs/research-pool.toml',
  'cockpit/graphs/research-write-review.toml',
  'cockpit/graphs/polyglot-departments.toml',
  '.gitignore',
  'README.md',
  'LICENSE',
  'SECURITY.md',
  'package.json',
  'package-lock.json',
  'rust-toolchain.toml',
  'release/supply-chain-policy.json',
  'release/Dockerfile.clean-builder',
  'bin/angelX',
  'install.sh',
  'scripts/runtime/angel-setup.sh',
  'scripts/release/build-prebuilt.sh',
  'scripts/release/package-prebuilt.sh',
  'tests/scripts/install.test.mjs',
  'tests/python/test_build_native_modules.py',
  'scripts/check/angel-club-policy.sh',
  'tests/scripts/angel-club-policy.test.mjs',
  'tests/scripts/native-launch.test.mjs',
  'cockpit/Cargo.toml',
  'cockpit/Cargo.lock',
  'cockpit/build.rs',
  'cockpit/build_fast.rs',
  'cockpit/bundle_bytes.rs',
  'cockpit/native/Cargo.toml',
  'cockpit/native/build.rs',
  'cockpit/native/build_parallel.rs',
  'cockpit/native/lib.rs',
  'cockpit/web-bundle/Cargo.toml',
  'cockpit/web-bundle/build.rs',
  'cockpit/web-bundle/lib.rs',
  'cockpit/web-bundle/pack_asset.rs',
  'cockpit/code-mode/Cargo.toml',
  'cockpit/code-mode/src/lib.rs',
  'cockpit/delve-cart/Cargo.toml',
  'cockpit/delve-cart/src/lib.rs',
  'cockpit/src/main.rs',
  'crates/Cargo.toml',
  'crates/Cargo.lock',
  // The Delve's browser page is built and embedded by the realm host's build
  // script, with its working-build helper module.
  'crates/realm-host/build.rs',
  'crates/realm-host/build_fast.rs',
  'crates/web/rules_js.rs',
  'crates/delve/src/lib.rs',
  'crates/web/delve/Cargo.toml',
  'tests/cockpit/integration/interactive_terminal_smoke.rs',
  'tests/cockpit/fixtures/book-migration-baseline.json',
  ...REQUIRED_COCKPIT_EMBEDDED_FILES,
  'cockpit/research/sloptomizer/README.md',
  'cockpit/docs/COMPETITION_RUNNER.md',
  'cockpit/docs/ENV.md',
  'cockpit/docs/JEV.md',
  'scripts/check/cockpit-source-digest.sh',
  'tests/python/test_cockpit_source_digest.py',
  'scripts/check/check-cockpit-fast.sh',
  'scripts/check/copy-cockpit-test-image.py',
  'scripts/check/check-cockpit-quality.sh',
  'scripts/check/check-active-connections.py',
  'tests/python/test_active_connections.py',
  'tests/python/test_sloptomizer_relations.py',
  'scripts/runtime/trajectory-redact.py',
  'scripts/runtime/private_store_io.py',
  'vendor/dotmax/Cargo.toml',
  'vendor/dotmax/LICENSE-APACHE',
  'vendor/dotmax/LICENSE-MIT',
  'vendor/dotmax/README.md',
  'vendor/dotmax/src/lib.rs',
  'vendor/ureq/ANGEL_PATCH.md',
  'vendor/ureq/Cargo.toml',
  'vendor/ureq/LICENSE-APACHE',
  'vendor/ureq/LICENSE-MIT',
  'vendor/ureq/README.md',
  'vendor/ureq/src/lib.rs',
  'scripts/release/release-evidence.mjs',
  'tests/scripts/release-evidence.test.mjs',
  'scripts/check/check-legacy-terminal-boundary.sh',
  'scripts/release/verify-release-evidence.mjs',
  'tests/scripts/verify-release-evidence.test.mjs',
  'scripts/release/verify-release-container.mjs',
  'tests/scripts/verify-release-container.test.mjs',
  'scripts/release/verify-release-advisories.mjs',
  'tests/scripts/verify-release-advisories.test.mjs',
  'scripts/release/verify-release-install.mjs',
  'tests/scripts/verify-release-install.test.mjs',
  'scripts/release/verify-release-rollback.mjs',
  'scripts/release/verify-runner-smoke.mjs',
  'tests/scripts/verify-runner-smoke.test.mjs',
  'docs/release-evidence.md',
  'docs/telemetry/trace-schema-v1.json',
])

const FORBIDDEN_NAMESPACE = 'off-limits'
const MAX_COMMAND_OUTPUT = 64 * 1024 * 1024

export class ReleaseGateError extends Error {
  constructor(message, details = undefined) {
    super(message)
    this.name = 'ReleaseGateError'
    this.details = details
  }
}

function fail(message, details) {
  throw new ReleaseGateError(message, details)
}

function run(command, args, { cwd, encoding = 'utf8' } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding,
    maxBuffer: MAX_COMMAND_OUTPUT,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  if (result.error) fail(`could not run ${command}: ${result.error.message}`)
  if (result.status !== 0) {
    const stderr = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString('utf8')
      : String(result.stderr || '')
    fail(`${command} exited ${result.status}: ${stderr.trim() || 'no diagnostic'}`)
  }
  return result.stdout
}

export function sha256Bytes(value) {
  return createHash('sha256').update(value).digest('hex')
}

export function sha256File(path) {
  return sha256Bytes(readFileSync(path))
}

export function isCockpitSourceIdentityInput(path) {
  // dotmax is the cockpit's local Cargo path dependency, so its manifest and
  // complete source tree must share the binary's source identity.
  return (
    path === 'cockpit/Cargo.toml' ||
    path === 'cockpit/Cargo.lock' ||
    path === 'cockpit/build.rs' ||
    path === 'cockpit/build_fast.rs' ||
    path === 'cockpit/bundle_bytes.rs' ||
    path.startsWith('cockpit/native/') ||
    path.startsWith('cockpit/web-bundle/') ||
    path.startsWith('cockpit/src/') ||
    path.startsWith('tests/cockpit/') ||
    path.startsWith('crates/') ||
    path === 'vendor/dotmax/Cargo.toml' ||
    path.startsWith('vendor/dotmax/src/') ||
    path === 'vendor/ureq/Cargo.toml' ||
    path.startsWith('vendor/ureq/src/') ||
    REQUIRED_COCKPIT_EMBEDDED_FILES.includes(path) ||
    REQUIRED_COCKPIT_BUILD_RUNTIME_FILES.includes(path)
  )
}

function u64Frame(value) {
  if (!Number.isSafeInteger(value) || value < 0) fail(`invalid identity frame length: ${value}`)
  const frame = Buffer.alloc(8)
  frame.writeBigUInt64BE(BigInt(value))
  return frame
}

export function cockpitSourceIdentityFromRows(rows) {
  const source = rows
    .filter((row) => isCockpitSourceIdentityInput(row.path))
    .toSorted((left, right) => left.path.localeCompare(right.path))
  if (source.length === 0) fail('release contains no cockpit source identity inputs')
  const hash = createHash('sha256')
  hash.update('angelX-cockpit-source-identity/v2\0')
  for (const row of source) {
    const path = Buffer.from(row.path, 'utf8')
    const content = Buffer.from(row.content)
    hash.update(u64Frame(path.length))
    hash.update(path)
    hash.update(u64Frame(content.length))
    hash.update(content)
  }
  return hash.digest('hex')
}

function normalizedObject(value) {
  if (Array.isArray(value)) return value.map(normalizedObject)
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, normalizedObject(value[key])]),
    )
  }
  return value
}

export function canonicalJson(value) {
  return `${JSON.stringify(normalizedObject(value), null, 2)}\n`
}

export function assertSafeReleasePath(path) {
  if (!path || isAbsolute(path) || path.includes('\\')) fail(`unsafe release path: ${path}`)
  if ([...path].some((character) => character.codePointAt(0) < 32 || character === '\x7f')) {
    fail(`release path contains control bytes: ${path}`)
  }
  const pieces = path.split('/')
  if (pieces.some((piece) => !piece || piece === '.' || piece === '..')) {
    fail(`release path is not normalized: ${path}`)
  }
  if (path === FORBIDDEN_NAMESPACE || path.startsWith(`${FORBIDDEN_NAMESPACE}/`)) {
    fail(`quarantined path cannot enter a release: ${path}`)
  }
  return path
}

function repositoryRoot(cwd) {
  const root = String(run('git', ['rev-parse', '--show-toplevel'], { cwd })).trim()
  if (!root) fail('git did not report a repository root')
  return realpathSync(root)
}

function parseIndexRows(output) {
  const rows = String(output).split('\0').filter(Boolean)
  return rows.map((row) => {
    const tab = row.indexOf('\t')
    if (tab < 0) fail(`could not parse git index row: ${row}`)
    const [mode, object, stage] = row.slice(0, tab).split(' ')
    const path = assertSafeReleasePath(row.slice(tab + 1))
    if (!/^[0-9]{6}$/u.test(mode) || !/^[0-9a-f]{40,64}$/u.test(object) || stage !== '0') {
      fail(`unsupported git index row for ${path}`)
    }
    return { path, mode, object }
  })
}

export function inventoryReleaseIndex(repoRoot, pathspecs = RELEASE_PATHS) {
  const output = run('git', ['ls-files', '--stage', '-z', '--', ...pathspecs], {
    cwd: repoRoot,
  })
  const rows = parseIndexRows(output).sort((a, b) =>
    a.path < b.path ? -1 : a.path > b.path ? 1 : 0,
  )
  if (rows.length === 0) fail('release path allowlist selected no tracked files')
  const seen = new Set()
  for (const row of rows) {
    if (seen.has(row.path)) fail(`duplicate release path: ${row.path}`)
    seen.add(row.path)
    if (row.mode !== '100644' && row.mode !== '100755') {
      fail(`release entries must be regular files, found mode ${row.mode}: ${row.path}`)
    }
  }
  return rows
}

export function inventoryReleaseFiles(repoRoot, pathspecs = RELEASE_PATHS) {
  const rows = inventoryReleaseIndex(repoRoot, pathspecs)

  const entries = rows.map((row) => {
    const absolute = resolve(repoRoot, row.path)
    const lexicalRelative = relative(repoRoot, absolute)
    if (lexicalRelative.startsWith(`..${sep}`) || lexicalRelative === '..') {
      fail(`release path escapes repository: ${row.path}`)
    }
    const info = lstatSync(absolute)
    if (!info.isFile()) fail(`release entry is not a regular file: ${row.path}`)
    const executable = (info.mode & 0o111) !== 0
    if (executable !== (row.mode === '100755')) {
      fail(`checkout mode differs from git mode for ${row.path}`)
    }
    const content = readFileSync(absolute)
    const gitObject = String(run('git', ['hash-object', '--', row.path], { cwd: repoRoot })).trim()
    if (gitObject !== row.object) fail(`working file differs from the git index: ${row.path}`)
    return {
      path: row.path,
      mode: row.mode,
      bytes: content.byteLength,
      sha256: sha256Bytes(content),
    }
  })

  return entries
}

export function assertRequiredReleaseFiles(entries, required = REQUIRED_RELEASE_FILES) {
  const paths = new Set(entries.map((entry) => entry.path))
  const missing = required.filter((path) => !paths.has(path))
  if (missing.length > 0) fail(`release is missing required files: ${missing.join(', ')}`)
}

const SYNTHETIC_HOME_NAMES = new Set(['angel', 'probe', 'proof', 'u', 'user', 'x'])
const TEXT_RELEASE_PATH =
  /(?:\.(?:gitignore|json|lock|md|mjs|patch|py|rs|sh|toml|txt|ya?ml)|Dockerfile[^/]*)$/u
const LIVE_TAILNET_DNS = /\b[A-Za-z0-9.-]+\.tail[0-9a-f]{4,}\.ts\.net\b/iu
const HARDWARE_HOSTNAME = new RegExp(`\\b${['d', 'g', 'x'].join('')}-[a-z0-9-]+\\b`, 'iu')
const HOST_MOUNT_COORDINATE = /\/mnt\/[A-Za-z0-9._-]+/u

function isTestFixturePath(path) {
  return /(?:^|\/)(?:fixtures?|tests?)(?:\/|\.|$)/u.test(path) || /\.test\.[cm]?js$/u.test(path)
}

function isTextReleasePath(path) {
  return path === 'LICENSE' || path === 'bin/angelX' || TEXT_RELEASE_PATH.test(path)
}

function isCgnatHost(host) {
  const octets = host.split('.').map(Number)
  return (
    octets.length === 4 &&
    octets.every((octet) => Number.isInteger(octet) && octet >= 0 && octet <= 255) &&
    octets[0] === 100 &&
    octets[1] >= 64 &&
    octets[1] <= 127
  )
}

/**
 * Reject operator-local material even if a future edit widens RELEASE_PATHS.
 *
 * Synthetic values are allowed only in test/fixture sources so private-network
 * behavior remains testable. Production source and documentation may not carry
 * any CGNAT literal or absolute per-user home path.
 */
export function assertPublicReleaseEntries(repoRoot, entries) {
  const embedded = new Set(REQUIRED_COCKPIT_EMBEDDED_FILES)
  for (const entry of entries) {
    const { path } = entry
    if (
      path === 'scripts/heads/heads.json' ||
      path.startsWith('scripts/heads/') ||
      (path.startsWith('cockpit/skills/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/personas/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/graphs/') &&
        ![
          'cockpit/graphs/council.toml',
          'cockpit/graphs/research-pool.toml',
          'cockpit/graphs/research-write-review.toml',
          'cockpit/graphs/polyglot-departments.toml',
        ].includes(path)) ||
      path.startsWith('benchmarks/') ||
      (path.startsWith('docs/') &&
        !['docs/release-evidence.md', 'docs/telemetry/trace-schema-v1.json'].includes(path) &&
        !PUBLIC_RELEASE_DOCS.includes(path) &&
        !embedded.has(path)) ||
      (path.startsWith('cockpit/docs/') &&
        ![
          'cockpit/docs/COMPETITION_RUNNER.md',
          'cockpit/docs/ENV.md',
          'cockpit/docs/JEV.md',
        ].includes(path)) ||
      (path.startsWith('cockpit/fixtures/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/modules/') && !embedded.has(path)) ||
      (path.startsWith('cockpit/research/') &&
        ![
          'cockpit/research/sloptomizer/README.md',
          'cockpit/research/sloptomizer/publish_runtime.py',
          'cockpit/research/labyrinth/README.md',
          'cockpit/research/labyrinth/schema.md',
          'cockpit/research/labyrinth/UPSTREAM.json',
        ].includes(path) &&
        !embedded.has(path)) ||
      path.startsWith('experimental/')
    ) {
      fail(`operator-only path cannot enter the public release: ${path}`)
    }

    if (!isTextReleasePath(path)) continue
    const source = readFileSync(join(repoRoot, path), 'utf8')
    // A root home folder only: a repository folder named `home` (delve/rules/home/) is not one.
    for (const match of source.matchAll(/(?<![A-Za-z0-9._~-])\/(?:home|Users)\/([A-Za-z0-9._-]+)(?:\/|\b)/gu)) {
      if (!SYNTHETIC_HOME_NAMES.has(match[1])) {
        fail(`machine-specific absolute home path in public release entry: ${path}`)
      }
    }
    for (const match of source.matchAll(/[A-Za-z]:\\Users\\([^\\\s]+)/gu)) {
      if (!SYNTHETIC_HOME_NAMES.has(match[1])) {
        fail(`machine-specific absolute home path in public release entry: ${path}`)
      }
    }
    for (const match of source.matchAll(/\b(?:\d{1,3}\.){3}\d{1,3}\b/gu)) {
      const host = match[0]
      if (isCgnatHost(host) && !isTestFixturePath(path)) {
        fail(`private CGNAT endpoint in public release entry: ${path}`)
      }
    }
    if (LIVE_TAILNET_DNS.test(source)) {
      fail(`live tailnet DNS name in public release entry: ${path}`)
    }
    if (HARDWARE_HOSTNAME.test(source)) {
      fail(`machine-specific hardware hostname in public release entry: ${path}`)
    }
    if (HOST_MOUNT_COORDINATE.test(source)) {
      fail(`machine-specific mount coordinate in public release entry: ${path}`)
    }
  }
}

// Check against the archive inventory, not the development checkout: a private
// helper present locally cannot satisfy an instruction in the distributed docs.
export function assertDocumentedSourcePaths(repoRoot, entries) {
  const paths = new Set(entries.map((entry) => entry.path))
  const present = (path) =>
    paths.has(path) || [...paths].some((entry) => entry.startsWith(`${path.replace(/\/$/u, '')}/`))
  const local = (document, target) =>
    relative(repoRoot, resolve(repoRoot, dirname(document), target))
      .split(sep)
      .join('/')
  for (const path of paths) {
    if (!path.endsWith('.md') || /^(vendor|third-party)\//u.test(path)) continue
    // The book embeds Markdown as model instructions. Its examples describe
    // files in a future user workspace, not dependencies of the source archive.
    // Keep these bytes in the inventory and public-content checks above.
    if (path.startsWith('cockpit/src/agent/harness/book/')) continue
    const source = readFileSync(join(repoRoot, path), 'utf8')
    for (const match of source.matchAll(/\]\(([^\s)]+)(?:\s+"[^"]*")?\)/gu)) {
      const target = match[1].replace(/^<|>$/gu, '')
      if (/^[A-Za-z][A-Za-z0-9+.-]*:|^#/u.test(target)) continue
      const referenced = local(path, decodeURIComponent(target.split('#')[0]))
      if (!present(referenced))
        fail(`documentation target absent from release: ${path} -> ${target}`)
    }
    for (const code of source.matchAll(/`([^`\n]+)`/gu)) {
      for (const match of code[1].matchAll(
        /(?<![\w/])((?:scripts|docs|cockpit|lib|bin|assets)\/[A-Za-z0-9_./-]+\.[A-Za-z0-9]+)/gu,
      )) {
        const target = match[1]
        // These are documented generated build outputs, never source inputs.
        if (target.startsWith('cockpit/target/')) continue
        if (!present(target) && !present(local(path, target))) {
          fail(`documented source absent from release: ${path} -> ${target}`)
        }
      }
    }
  }
}

// Resolve runtime imports against the distributed inventory. A dependency in
// the private checkout cannot make an incomplete public bundle pass.
export function assertRuntimeHelperClosure(repoRoot, entries) {
  const paths = new Set(entries.map((entry) => entry.path))
  for (const path of paths) {
    if (!/^(scripts|lib)\/.+\.(mjs|js)$/u.test(path) || path.includes('.test.')) continue
    const source = readFileSync(join(repoRoot, path), 'utf8')
    const dependencies = []
    for (const match of source.matchAll(
      /(?:from\s*|import\s*(?:\(\s*)?|new URL\s*\()\s*['"](\.\.?\/[^'"\n]+)['"]/gu,
    )) {
      dependencies.push(
        relative(repoRoot, resolve(repoRoot, dirname(path), match[1]))
          .split(sep)
          .join('/'),
      )
    }
    for (const match of source.matchAll(
      /['"`]((?:\$\{(?:root|ROOT)\}\/)?scripts\/[A-Za-z0-9_.-]+\.(?:mjs|py|sh))['"`]/gu,
    ))
      dependencies.push(match[1].replace(/^\$\{(?:root|ROOT)\}\//u, ''))
    for (const dependency of dependencies) {
      if (!paths.has(dependency))
        fail(`runtime helper absent from release: ${path} -> ${dependency}`)
    }
  }
}

export function cockpitSourceSha256(repoRoot, entries) {
  return cockpitSourceIdentityFromRows(
    entries.map((entry) => ({
      path: entry.path,
      content: readFileSync(join(repoRoot, entry.path)),
    })),
  )
}

export function assertReleaseInputsClean(repoRoot, pathspecs = RELEASE_PATHS) {
  const tagged = run('git', ['ls-files', '-v', '-z', '--cached', '--', ...pathspecs], {
    cwd: repoRoot,
  })
  for (const row of String(tagged).split('\0').filter(Boolean)) {
    if (row.length < 3 || row[1] !== ' ') fail(`could not parse git index flag row: ${row}`)
    const tag = row[0]
    const path = assertSafeReleasePath(row.slice(2))
    if (tag === 'S' || tag === 's') {
      fail(`release input has skip-worktree set: ${path}`)
    }
    if (/^[a-z]$/u.test(tag)) {
      fail(`release input has assume-unchanged set: ${path}`)
    }
  }

  const dirty = run(
    'git',
    ['status', '--porcelain=v1', '-z', '--untracked-files=all', '--', ...pathspecs],
    { cwd: repoRoot },
  )
  if (String(dirty).length > 0) {
    fail('release inputs are dirty; commit or remove changes before producing evidence')
  }
}

export function inspectNodeLock(repoRoot) {
  const packageJson = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8'))
  const lockPath = join(repoRoot, 'package-lock.json')
  const lock = JSON.parse(readFileSync(lockPath, 'utf8'))
  if (packageJson.private !== true || packageJson.license !== 'MIT') {
    fail('release policy requires MIT licensing and disabled npm publication (private: true)')
  }
  if (packageJson.engines?.node !== '^20.19.0 || ^22.13.0 || >=24') {
    fail('supported Node policy must match the ESLint 10 runtime floor')
  }
  if (lock.lockfileVersion !== 3 || !lock.packages || typeof lock.packages !== 'object') {
    fail('package-lock.json must use lockfileVersion 3 with a packages graph')
  }
  if (lock.name !== packageJson.name || lock.version !== packageJson.version) {
    fail('package-lock root identity differs from package.json')
  }
  if (lock.packages['']?.license !== packageJson.license) {
    fail('package-lock root license differs from package.json')
  }

  const registry = []
  for (const [path, dependency] of Object.entries(lock.packages)) {
    if (!path) continue
    const resolved = String(dependency.resolved || '')
    if (!resolved.startsWith('https://registry.npmjs.org/')) {
      fail(`non-registry Node dependency is not permitted in the release graph: ${path}`)
    }
    const integrity = String(dependency.integrity || '')
    const integrityMatch = integrity.match(/^sha512-([A-Za-z0-9+/]+={0,2})$/u)
    if (!integrityMatch || Buffer.from(integrityMatch[1], 'base64').byteLength !== 64) {
      fail(`Node dependency lacks sha512 integrity: ${path}`)
    }
    if (!dependency.license || typeof dependency.license !== 'string') {
      fail(`Node dependency lacks license metadata: ${path}`)
    }
    registry.push(path)
  }

  return {
    lockfile_sha256: sha256File(lockPath),
    lockfile_version: lock.lockfileVersion,
    package_count: Object.keys(lock.packages).length,
    registry_package_count: registry.length,
    integrity_policy: 'all registry packages carry sha512 lock integrity',
    license_policy: 'all registry packages carry lockfile license metadata',
    product_license: 'MIT',
  }
}

function parseRustToolchain(repoRoot) {
  const path = join(repoRoot, 'rust-toolchain.toml')
  const source = readFileSync(path, 'utf8')
  const match = source.match(/^channel\s*=\s*"([^"]+)"\s*$/mu)
  if (!match) fail('rust-toolchain.toml must declare an exact channel')
  if (!/^\d+\.\d+\.\d+$/u.test(match[1])) fail('Rust channel must be an exact stable version')
  const componentsMatch = source.match(/^components\s*=\s*\[([^\]]+)\]\s*$/mu)
  const components = componentsMatch
    ? [...componentsMatch[1].matchAll(/"([^"]+)"/gu)].map((component) => component[1]).sort()
    : []
  if (canonicalJson(components) !== canonicalJson(['clippy', 'rustfmt'])) {
    fail('Rust toolchain must pin both clippy and rustfmt components')
  }
  return { version: match[1], components, sha256: sha256File(path) }
}

function assertWithin(root, candidate, label) {
  const answer = relative(root, candidate)
  if (answer === '..' || answer.startsWith(`..${sep}`) || isAbsolute(answer)) {
    fail(`${label} escapes its package root`)
  }
}

function parseCargoLock(repoRoot) {
  const path = join(repoRoot, 'cockpit/Cargo.lock')
  const source = readFileSync(path, 'utf8')
  if (!/^version = 4$/mu.test(source)) fail('Cargo.lock must use lockfile version 4')
  const packages = source
    .split(/^\[\[package\]\]\s*$/mu)
    .slice(1)
    .map((block) => {
      const field = (name) => block.match(new RegExp(`^${name} = "([^"]+)"$`, 'mu'))?.[1] || null
      return {
        name: field('name'),
        version: field('version'),
        source: field('source'),
        checksum: field('checksum'),
      }
    })
  if (packages.some((pkg) => !pkg.name || !pkg.version))
    fail('Cargo.lock contains an invalid package row')
  const rows = new Map()
  let registryPackages = 0
  for (const pkg of packages) {
    const key = `${pkg.name}\0${pkg.version}\0${pkg.source || ''}`
    if (rows.has(key))
      fail(`Cargo.lock contains a duplicate package identity: ${pkg.name}@${pkg.version}`)
    rows.set(key, pkg)
    if (pkg.source) {
      if (!pkg.source.startsWith('registry+')) {
        fail(`Cargo.lock contains a non-registry source: ${pkg.name}@${pkg.version}`)
      }
      if (!pkg.checksum || !/^[0-9a-f]{64}$/u.test(pkg.checksum)) {
        fail(`Cargo.lock registry package lacks a checksum: ${pkg.name}@${pkg.version}`)
      }
      registryPackages += 1
    }
  }
  return { path, packages, rows, registryPackages }
}

// Packaged lockfiles that are frozen candidate input rather than a dependency
// graph of the release. The narrowed archive carries only its three dependency
// locks, so nothing is exempt; the manifest still records the (empty) list.
const NON_DEPENDENCY_FIXTURE_LOCKFILES = new Set()

function advisoryLockfileEcosystem(path) {
  if (NON_DEPENDENCY_FIXTURE_LOCKFILES.has(path)) return null
  const name = basename(path)
  if (name === 'Cargo.lock') return 'crates.io'
  if (name === 'package-lock.json') return 'npm'
  if (
    ['uv.lock', 'Pipfile.lock', 'poetry.lock', 'pdm.lock', 'pylock.toml'].includes(name) ||
    /^requirements(?:-[^.]+)?\.txt$/u.test(name)
  ) {
    return 'PyPI'
  }
  return null
}

function advisoryLockfilePackageCount(repoRoot, path, ecosystem) {
  const source = readFileSync(join(repoRoot, path), 'utf8')
  if (ecosystem === 'crates.io') {
    return [...source.matchAll(/^\[\[package\]\]\s*$/gmu)].length
  }
  if (ecosystem === 'npm') {
    const lock = JSON.parse(source)
    if (!lock.packages || typeof lock.packages !== 'object') {
      fail(`advisory lockfile has no npm packages graph: ${path}`)
    }
    return Object.keys(lock.packages).filter(Boolean).length
  }
  fail(`unsupported advisory lockfile ecosystem: ${ecosystem}`)
}

function inspectSupplyChainPolicy(repoRoot, metadata, toolchain, releaseEntries) {
  const path = join(repoRoot, 'release/supply-chain-policy.json')
  const policy = JSON.parse(readFileSync(path, 'utf8'))
  if (
    policy.schema !== 'angelX-release-supply-chain-policy/v1' ||
    !Array.isArray(policy.prebuilt_artifacts) ||
    policy.prebuilt_artifacts.length !== 1 ||
    !Array.isArray(policy.container_images) ||
    policy.container_images.length !== 1 ||
    !Array.isArray(policy.advisory_scanners) ||
    policy.advisory_scanners.length !== 1 ||
    !policy.advisory_policy ||
    typeof policy.advisory_policy !== 'object'
  ) {
    fail('release supply-chain policy must declare exactly one versioned prebuilt artifact')
  }
  const artifact = policy.prebuilt_artifacts[0]
  const locked = metadata.packages.find(
    (pkg) => pkg.name === artifact.crate && pkg.version === artifact.crate_version,
  )
  if (!locked) fail('prebuilt rusty_v8 policy does not match the supported Cargo graph')
  if (
    artifact.name !== 'rusty_v8' ||
    artifact.target !== SUPPORTED_CARGO_TARGET ||
    artifact.profile !== 'release' ||
    artifact.format !== 'decompressed-static-library' ||
    !Number.isSafeInteger(artifact.bytes) ||
    artifact.bytes <= 0 ||
    !/^[0-9a-f]{64}$/u.test(artifact.sha256) ||
    artifact.upstream_url !==
      `https://github.com/denoland/rusty_v8/releases/download/v${artifact.crate_version}/librusty_v8_release_${SUPPORTED_CARGO_TARGET}.a.gz` ||
    !String(artifact.trust).includes('no upstream signature')
  ) {
    fail('prebuilt rusty_v8 policy is incomplete or inconsistent')
  }
  const image = policy.container_images[0]
  const dockerfilePath = join(repoRoot, 'release/Dockerfile.clean-builder')
  const dockerfile = readFileSync(dockerfilePath, 'utf8')
  if (
    image.name !== 'clean_release_builder' ||
    image.reference !== image.digest ||
    !/^sha256:[0-9a-f]{64}$/u.test(image.digest) ||
    image.local_tag !== 'angelX-clean-builder:rust-1.99.0-bookworm-v1' ||
    image.base_reference !== `docker.io/library/rust@${image.base_digest}` ||
    !/^sha256:[0-9a-f]{64}$/u.test(image.base_digest) ||
    image.dockerfile_sha256 !== sha256File(dockerfilePath) ||
    !dockerfile.startsWith(`FROM ${image.base_reference}\n`) ||
    image.operating_system !== 'linux' ||
    image.architecture !== 'amd64' ||
    image.rust_version !== toolchain.version ||
    !String(image.distribution).includes('bookworm') ||
    !Array.isArray(image.native_packages) ||
    image.native_packages.length !== 9 ||
    !String(image.trust).includes('no signature')
  ) {
    fail('clean release container policy is incomplete or inconsistent')
  }
  const scanner = policy.advisory_scanners[0]
  if (
    scanner.name !== 'osv-scanner' ||
    scanner.version !== '2.3.8' ||
    !/^[0-9a-f]{40}$/u.test(String(scanner.commit || '')) ||
    !/^sha256:[0-9a-f]{64}$/u.test(String(scanner.digest || '')) ||
    scanner.reference !== `ghcr.io/google/osv-scanner@${scanner.digest}` ||
    scanner.operating_system !== 'linux' ||
    scanner.architecture !== 'amd64' ||
    !String(scanner.trust).includes('not yet performed')
  ) {
    fail('advisory scanner policy is incomplete or inconsistent')
  }
  const advisory = policy.advisory_policy
  const expectedLockfiles = new Map([
    ['cockpit/Cargo.lock', 'crates.io'],
    ['crates/Cargo.lock', 'crates.io'],
    ['package-lock.json', 'npm'],
  ])
  if (
    !Array.isArray(advisory.lockfiles) ||
    advisory.lockfiles.length !== expectedLockfiles.size ||
    advisory.python?.status !== 'not-applicable' ||
    !String(advisory.python?.reason).includes('no Python dependency') ||
    !Array.isArray(advisory.exceptions) ||
    advisory.exceptions.length !== 3
  ) {
    fail('release advisory policy scope is incomplete')
  }
  for (const lockfile of advisory.lockfiles) {
    const ecosystem = expectedLockfiles.get(lockfile.path)
    if (
      !ecosystem ||
      lockfile.ecosystem !== ecosystem ||
      lockfile.sha256 !== sha256File(join(repoRoot, lockfile.path)) ||
      lockfile.package_count !==
        advisoryLockfilePackageCount(repoRoot, lockfile.path, lockfile.ecosystem)
    ) {
      fail(`release advisory lockfile policy differs: ${lockfile.path || 'unknown'}`)
    }
    expectedLockfiles.delete(lockfile.path)
  }
  if (expectedLockfiles.size > 0) fail('release advisory policy omitted a dependency lockfile')
  if (releaseEntries) {
    const discovered = releaseEntries
      .map((entry) => ({ path: entry.path, ecosystem: advisoryLockfileEcosystem(entry.path) }))
      .filter((entry) => entry.ecosystem)
      .sort((left, right) => left.path.localeCompare(right.path))
    const declared = [...advisory.lockfiles]
      .map(({ path, ecosystem }) => ({ path, ecosystem }))
      .sort((left, right) => left.path.localeCompare(right.path))
    if (canonicalJson(discovered) !== canonicalJson(declared)) {
      fail('release advisory policy does not cover every packaged dependency lockfile')
    }
    if (
      discovered.some((entry) => entry.ecosystem === 'PyPI') !==
      (advisory.python.status !== 'not-applicable')
    ) {
      fail('release advisory Python applicability differs from packaged inputs')
    }
  }
  const exceptionKeys = new Set()
  for (const exception of advisory.exceptions) {
    if (
      exception.lockfile !== 'cockpit/Cargo.lock' ||
      !exception.package ||
      !exception.version ||
      !Array.isArray(exception.ids) ||
      exception.ids.length === 0 ||
      canonicalJson(exception.ids) !== canonicalJson([...new Set(exception.ids)].sort()) ||
      !['unmaintained', 'unsound'].includes(exception.kind) ||
      !/^\d{4}-\d{2}-\d{2}$/u.test(String(exception.expires || '')) ||
      !exception.reason ||
      exception.reason.length < 32
    ) {
      fail('release advisory exception is incomplete or inconsistent')
    }
    const key = `${exception.lockfile}\0${exception.package}\0${exception.version}\0${exception.ids.join(',')}`
    if (exceptionKeys.has(key)) fail('release advisory policy contains duplicate exceptions')
    exceptionKeys.add(key)
  }
  return {
    policy_file_sha256: sha256File(path),
    artifacts: [artifact],
    container_images: [image],
    advisory_scanners: [scanner],
    advisory_policy: advisory,
  }
}

export function inspectCargoGraph(repoRoot, releaseEntries = undefined) {
  const toolchain = parseRustToolchain(repoRoot)
  const lock = parseCargoLock(repoRoot)
  const metadataText = run(
    'cargo',
    [
      'metadata',
      '--manifest-path',
      'cockpit/Cargo.toml',
      '--locked',
      '--offline',
      '--filter-platform',
      SUPPORTED_CARGO_TARGET,
      '--format-version',
      '1',
    ],
    { cwd: repoRoot },
  )
  const metadata = JSON.parse(metadataText)
  const supplyChain = inspectSupplyChainPolicy(repoRoot, metadata, toolchain, releaseEntries)
  const rootId = metadata.resolve?.root
  const rootPackage = metadata.packages.find((pkg) => pkg.id === rootId)
  if (!rootPackage || rootPackage.name !== 'angelX-cockpit') {
    fail('Cargo metadata did not resolve angelX-cockpit as the workspace root')
  }

  let registryPackages = 0
  let pathPackages = 0
  const licenseFiles = []
  for (const pkg of metadata.packages) {
    const manifestPath = realpathSync(pkg.manifest_path)
    const packageRoot = dirname(manifestPath)
    if (pkg.source === null) {
      pathPackages += 1
      assertWithin(repoRoot, manifestPath, `path dependency ${pkg.name}`)
      if (pkg.id === rootId) {
        if (pkg.license !== 'MIT') {
          fail('angelX-cockpit must declare the MIT license')
        }
      } else if (!pkg.license && !pkg.license_file) {
        fail(`path dependency lacks license metadata: ${pkg.name}@${pkg.version}`)
      }
    } else {
      registryPackages += 1
      if (!String(pkg.source).startsWith('registry+')) {
        fail(`non-registry Cargo source is not permitted: ${pkg.name}@${pkg.version}`)
      }
      const locked = lock.rows.get(`${pkg.name}\0${pkg.version}\0${pkg.source}`)
      if (!locked) {
        fail(`resolved Cargo package is absent from Cargo.lock: ${pkg.name}@${pkg.version}`)
      }
      if (!pkg.license && !pkg.license_file) {
        fail(
          `Cargo registry package lacks license metadata or a license file: ${pkg.name}@${pkg.version}`,
        )
      }
    }

    if (pkg.license_file) {
      const licensePath = resolve(packageRoot, pkg.license_file)
      assertWithin(packageRoot, licensePath, `license file for ${pkg.name}@${pkg.version}`)
      if (!existsSync(licensePath) || !statSync(licensePath).isFile()) {
        fail(`declared license file is unavailable: ${pkg.name}@${pkg.version}`)
      }
      const realPackageRoot = realpathSync(packageRoot)
      const realLicensePath = realpathSync(licensePath)
      assertWithin(realPackageRoot, realLicensePath, `license file for ${pkg.name}@${pkg.version}`)
      licenseFiles.push({
        package: `${pkg.name}@${pkg.version}`,
        declared_path: pkg.license_file,
        sha256: sha256File(realLicensePath),
      })
    }
  }
  licenseFiles.sort((a, b) => (a.package < b.package ? -1 : a.package > b.package ? 1 : 0))

  const rustc = String(run('rustc', ['-Vv'], { cwd: repoRoot })).trim()
  const release = rustc.match(/^release:\s*(\S+)$/mu)?.[1]
  if (release !== toolchain.version) {
    fail(`active rustc ${release || 'unknown'} differs from pinned ${toolchain.version}`)
  }

  return {
    lockfile_sha256: sha256File(lock.path),
    locked_package_count: lock.packages.length,
    locked_registry_package_count: lock.registryPackages,
    metadata_target: SUPPORTED_CARGO_TARGET,
    resolved_package_count: metadata.packages.length,
    registry_package_count: registryPackages,
    path_package_count: pathPackages,
    registry_policy: 'registry-only dependencies with Cargo checksums',
    license_policy: 'SPDX metadata or a present hashed license file; product root is MIT',
    license_files: licenseFiles,
    prebuilt_artifacts: supplyChain.artifacts,
    clean_build_container_images: supplyChain.container_images,
    advisory_scanners: supplyChain.advisory_scanners,
    advisory_policy: supplyChain.advisory_policy,
    supply_chain_policy_file_sha256: supplyChain.policy_file_sha256,
    toolchain: {
      pinned_version: toolchain.version,
      pinned_components: toolchain.components,
      toolchain_file_sha256: toolchain.sha256,
      rustc_verbose: rustc.split('\n'),
      cargo: String(run('cargo', ['-V'], { cwd: repoRoot })).trim(),
      clippy: String(run('cargo', ['clippy', '--version'], { cwd: repoRoot })).trim(),
      rustfmt: String(run('cargo', ['fmt', '--version'], { cwd: repoRoot })).trim(),
    },
  }
}

function sameEntries(left, right) {
  return canonicalJson(left) === canonicalJson(right)
}

function atomicWrite(path, value, mode = 0o644) {
  const temporary = `${path}.tmp-${process.pid}`
  writeFileSync(temporary, value, { mode })
  chmodSync(temporary, mode)
  renameSync(temporary, path)
}

export function createSourceArchive(repoRoot, outputPath, entries) {
  mkdirSync(dirname(outputPath), { recursive: true })
  const temporaryArchive = `${outputPath}.tmp-${process.pid}`
  const listPath = `${outputPath}.files-${process.pid}`
  try {
    writeFileSync(listPath, Buffer.from(`${entries.map((entry) => entry.path).join('\0')}\0`))
    run(
      'tar',
      [
        '--format=ustar',
        '--sort=name',
        '--mtime=@0',
        '--owner=0',
        '--group=0',
        '--numeric-owner',
        '--mode=a=rwX,go-w',
        '--no-recursion',
        '--null',
        '--verbatim-files-from',
        '--files-from',
        listPath,
        '-cf',
        temporaryArchive,
      ],
      { cwd: repoRoot, encoding: null },
    )
    const listed = String(run('tar', ['-tf', temporaryArchive], { cwd: repoRoot }))
      .split('\n')
      .filter(Boolean)
    const expected = entries.map((entry) => entry.path)
    if (canonicalJson(listed) !== canonicalJson(expected)) {
      fail('archive entry list differs from the release inventory')
    }
    renameSync(temporaryArchive, outputPath)
  } finally {
    rmSync(listPath, { force: true })
    rmSync(temporaryArchive, { force: true })
  }
  return {
    name: basename(outputPath),
    media_type: 'application/x-tar',
    bytes: statSync(outputPath).size,
    sha256: sha256File(outputPath),
  }
}

function toolVersions(repoRoot) {
  const [nodeMajor, nodeMinor] = process.versions.node.split('.').map(Number)
  const nodeSupported =
    (nodeMajor === 20 && nodeMinor >= 19) ||
    (nodeMajor === 22 && nodeMinor >= 13) ||
    nodeMajor >= 24
  if (!nodeSupported) fail('release gate requires Node 20.19+, 22.13+, or 24+')
  const tar = String(run('tar', ['--version'], { cwd: repoRoot }))
    .split('\n')[0]
    .trim()
  if (!tar.startsWith('tar (GNU tar) ')) fail('release gate requires GNU tar')
  return {
    node: process.version,
    git: String(run('git', ['--version'], { cwd: repoRoot })).trim(),
    tar,
  }
}

export function assertSafeOutputDirectory(repoRoot, outputRoot) {
  const outputRelative = relative(repoRoot, outputRoot)
  const outputInsideRepository =
    outputRelative !== '..' && !outputRelative.startsWith(`..${sep}`) && !isAbsolute(outputRelative)
  if (
    outputInsideRepository &&
    outputRelative !== '.angel' &&
    !outputRelative.startsWith(`.angel${sep}`)
  ) {
    fail('release output inside the repository must stay under .angel/')
  }
  return outputRoot
}

export function buildManifest({
  version,
  commit,
  tree,
  entries,
  archive,
  nodeDependencies,
  rustDependencies,
  tools,
  cockpitSource,
}) {
  const unsigned = {
    schema: RELEASE_SCHEMA,
    scope: RELEASE_SCOPE,
    version,
    source: {
      commit,
      tree,
      cockpit_source_sha256: cockpitSource,
      release_inputs_dirty: false,
    },
    support: {
      operating_system: 'Linux',
      architecture: 'x86_64',
      cargo_target: SUPPORTED_CARGO_TARGET,
      node: '^20.19.0 || ^22.13.0 || >=24',
      rust: rustDependencies.toolchain.pinned_version,
    },
    policy: {
      excluded_namespace: `${FORBIDDEN_NAMESPACE}/`,
      source_inputs: 'tracked, committed, regular files from the explicit allowlist',
      non_dependency_fixture_lockfiles: [...NON_DEPENDENCY_FIXTURE_LOCKFILES].sort(),
      network_during_gate: 'disabled for Cargo metadata; no vulnerability feed queried',
      clean_host_proven: false,
    },
    dependencies: { node: nodeDependencies, rust: rustDependencies },
    tools,
    entries_manifest_sha256: sha256Bytes(canonicalJson(entries)),
    entries,
    artifact: archive,
  }
  return {
    ...unsigned,
    manifest_sha256: sha256Bytes(canonicalJson(unsigned)),
  }
}

export function runReleaseGate({ cwd = process.cwd(), outputDirectory } = {}) {
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    fail('the supported release gate currently runs only on Linux x86_64')
  }
  const repoRoot = repositoryRoot(cwd)
  assertReleaseInputsClean(repoRoot)
  const entries = inventoryReleaseFiles(repoRoot)
  assertRequiredReleaseFiles(entries)
  assertPublicReleaseEntries(repoRoot, entries)
  assertDocumentedSourcePaths(repoRoot, entries)
  assertRuntimeHelperClosure(repoRoot, entries)
  const packageJson = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8'))
  const commit = String(run('git', ['rev-parse', 'HEAD'], { cwd: repoRoot })).trim()
  const tree = String(run('git', ['rev-parse', 'HEAD^{tree}'], { cwd: repoRoot })).trim()
  const shortCommit = commit.slice(0, 12)
  const outputRoot = resolve(repoRoot, outputDirectory || '.angel/release')
  assertSafeOutputDirectory(repoRoot, outputRoot)
  const stem = `angelX-source-${packageJson.version}-${shortCommit}`
  const archivePath = join(outputRoot, `${stem}.tar`)

  const nodeDependencies = inspectNodeLock(repoRoot)
  const rustDependencies = inspectCargoGraph(repoRoot, entries)
  const tools = toolVersions(repoRoot)
  const cockpitSource = cockpitSourceSha256(repoRoot, entries)
  const archive = createSourceArchive(repoRoot, archivePath, entries)

  assertReleaseInputsClean(repoRoot)
  const finalEntries = inventoryReleaseFiles(repoRoot)
  assertPublicReleaseEntries(repoRoot, finalEntries)
  assertDocumentedSourcePaths(repoRoot, finalEntries)
  assertRuntimeHelperClosure(repoRoot, finalEntries)
  if (!sameEntries(entries, finalEntries))
    fail('release inputs changed while the archive was built')

  const manifest = buildManifest({
    version: packageJson.version,
    commit,
    tree,
    entries,
    archive,
    nodeDependencies,
    rustDependencies,
    tools,
    cockpitSource,
  })
  const manifestPath = join(outputRoot, `${stem}.manifest.json`)
  atomicWrite(manifestPath, canonicalJson(manifest))
  atomicWrite(`${archivePath}.sha256`, `${archive.sha256}  ${archive.name}\n`)
  atomicWrite(`${manifestPath}.sha256`, `${sha256File(manifestPath)}  ${basename(manifestPath)}\n`)

  return { repoRoot, archivePath, manifestPath, manifest }
}

function parseArgs(argv) {
  const options = {}
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--out') {
      index += 1
      if (!argv[index]) fail('--out requires a directory')
      options.outputDirectory = argv[index]
    } else {
      fail(`unknown argument: ${argv[index]}`)
    }
  }
  return options
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  try {
    const result = runReleaseGate(parseArgs(process.argv.slice(2)))
    process.stdout.write(
      `${canonicalJson({
        schema: result.manifest.schema,
        artifact: result.manifest.artifact,
        manifest: relative(result.repoRoot, result.manifestPath),
        manifest_sha256: result.manifest.manifest_sha256,
      })}`,
    )
  } catch (error) {
    process.stderr.write(`release gate failed: ${error.message}\n`)
    process.exitCode = 1
  }
}
