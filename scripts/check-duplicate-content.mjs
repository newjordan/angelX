#!/usr/bin/env node
// Repo-wide duplicate-content guard.
//
// Every byte-identical pair in a repository is either a deliberate record or an
// accident that will silently drift. This check makes the difference explicit:
// a duplicate group passes only when a named rule states why it exists. A new,
// unexplained duplicate fails the gate instead of accumulating unnoticed.
//
// Offline and dependency-free (stdlib + `git ls-files`). Exit codes:
//   0  every duplicate group is explained by a rule
//   1  at least one duplicate group is unexplained (or --strict and any group exists)
//   2  the check could not run
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const SCRIPT = "check-duplicate-content";

// --- path matching -------------------------------------------------------

/** Compile a `/`-separated glob (`**`, `*`, `?`) into an anchored RegExp. */
export function globToRegExp(glob) {
  let out = "^";
  for (let i = 0; i < glob.length; i += 1) {
    const ch = glob[i];
    if (ch === "*") {
      if (glob[i + 1] === "*") {
        // `**/` may match zero path segments; bare `**` matches everything.
        if (glob[i + 2] === "/") {
          out += "(?:.*/)?";
          i += 2;
        } else {
          out += ".*";
          i += 1;
        }
      } else {
        out += "[^/]*";
      }
    } else if (ch === "?") {
      out += "[^/]";
    } else if ("\\^$.|+()[]{}".includes(ch)) {
      out += `\\${ch}`;
    } else {
      out += ch;
    }
  }
  return new RegExp(`${out}$`, "u");
}

const compiled = new Map();

function asRegExp(glob) {
  if (!compiled.has(glob)) compiled.set(glob, globToRegExp(glob));
  return compiled.get(glob);
}

/** True when `path` (repo-relative, `/`-separated) matches any glob. */
export function matchesAny(path, globs) {
  return globs.some((g) => asRegExp(g).test(path));
}

// --- rules ---------------------------------------------------------------

// Each rule names a reason a duplicate group is allowed to exist. A group is
// explained by the first rule whose constraints all hold.
//
// Constraint fields (all optional, all must hold):
//   all_match      every member matches at least one glob
//   some_match     at least one member matches a glob
//   each_match     every glob has at least one matching member
//   all_basenames  every member basename is in the list
//   max_bytes      largest member size is at most this
//   min_members    group has at least this many members
export const DEFAULT_RULES = [
  {
    id: "python-package-markers",
    reason:
      "Empty or one-line `__init__.py` package markers. Identical by Python convention, not by copy.",
    all_basenames: ["__init__.py"],
    max_bytes: 2048,
  },
  {
    id: "benchmark-sandbox-ignore-markers",
    reason:
      "Per-directory `.gitignore` markers: each benchmark sandbox needs its own so its outputs stay untracked.",
    all_basenames: [".gitignore"],
  },
  {
    id: "empty-files",
    reason:
      "Zero-byte files: empty log captures and store lock markers. They are identical because they carry no content at all, so grouping them is an artifact of hashing the empty string rather than a duplicated record.",
    max_bytes: 0,
    min_members: 2,
  },
  {
    id: "empty-store-lock-markers",
    reason:
      "Empty `*.json.lock` store lock markers; identical because they have no payload.",
    name_suffix: ".json.lock",
  },
  {
    id: "validation-receipts",
    reason:
      "Immutable validation evidence under `notes/validation/`. A capture is written both as a raw `.log` and as its JSON receipt, and re-captured per install candidate.",
    all_match: ["notes/validation/**"],
    min_members: 2,
  },
  {
    id: "benchmark-run-records",
    reason:
      "Per-run benchmark records. A task definition is copied from `benchmarks/realwork/**` into the published `docs/benchmarks/**` record, and repeated baselines differ only in name.",
    all_match: ["docs/benchmarks/**", "benchmarks/**"],
    some_match: ["docs/benchmarks/**"],
    min_members: 2,
  },
  {
    id: "research-embed-archive-pair",
    reason:
      "The embedded runtime copy under `cockpit/research/sloptomizer/` is byte-for-byte identical to its receipted archival source under `experimental/sloptomizer/upstream/`. Both sides are load-bearing: the first is compiled into the cockpit binary by `include_bytes!`, the second is the provenance archive. `scripts/check-research-embed.mjs` asserts the pair stays identical and matches the recorded sha256, so the copy cannot silently fork.",
    all_match: [
      "cockpit/research/sloptomizer/**",
      "experimental/sloptomizer/upstream/**",
    ],
    each_match: [
      "cockpit/research/sloptomizer/**",
      "experimental/sloptomizer/upstream/**",
    ],
    min_members: 2,
  },
  {
    id: "retained-portrait-generation",
    reason:
      "Retired portrait generations kept as original artwork (see cockpit/assets/agents/README.md). The current renderer selects the helms pose sheets, so these legacy PNGs are inert. `codex-active.png` and `codex-neutral.png` are byte-identical, which means one label of that retired generation is a copy rather than a distinct pose; art triage owns that call. Naming the exact pair keeps a future asset duplicate a hard failure.",
    exact_paths: [
      [
        "cockpit/assets/agents/codex-active.png",
        "cockpit/assets/agents/codex-neutral.png",
      ],
    ],
  },
  {
    id: "helms-guide-promotion",
    reason:
      "Each helms agent sheet is the knight guide art it was promoted from, kept under both names and recorded in `cockpit/assets/agents/helms/manifest.json` (`generation: gritty knight sheet, promoted from the guide art`, `original: knight-0N-sprite-sheet-guide.png (this dir, unmodified)`). Both sides are load-bearing: `cockpit/src/ui/helm.rs` loads the agent-named sheet, the guide file is the provenance record for that promotion, and the guide name is what the capture pipeline writes. Pinning the exact pairs keeps a new asset duplicate a hard failure instead of letting it join this set quietly.",
    exact_paths: [
      [
        "cockpit/assets/agents/helms/turbo.png",
        "cockpit/assets/agents/helms/knight-01-sprite-sheet-guide.png",
      ],
      [
        "cockpit/assets/agents/helms/atlas.png",
        "cockpit/assets/agents/helms/knight-02-sprite-sheet-guide.png",
      ],
      [
        "cockpit/assets/agents/helms/sparky.png",
        "cockpit/assets/agents/helms/knight-03-sprite-sheet-guide.png",
      ],
      [
        "cockpit/assets/agents/helms/apollo.png",
        "cockpit/assets/agents/helms/knight-04-sprite-sheet-guide.png",
      ],
      [
        "cockpit/assets/agents/helms/codex.png",
        "cockpit/assets/agents/helms/knight-05-sprite-sheet-guide.png",
      ],
    ],
  },
  {
    id: "website-excalibur-atlas",
    reason:
      "The website ships its own copy of the Excalibur rise atlas because `website/` is the deployed root (Vercel builds that directory alone), so nothing under it can reach `cockpit/` at runtime. `cockpit/assets/excalibur/rise.png` is the atlas the app itself plays and `website/README.md` records the copy, so the pair is a deliberate publication step rather than drift. Pinning the pair means a fork of either side fails the gate.",
    exact_paths: [
      [
        "cockpit/assets/excalibur/rise.png",
        "website/assets/excalibur/rise.png",
      ],
    ],
  },
];

function ruleMatches(rule, group) {
  const paths = group.map((m) => m.path);
  if (rule.min_members && paths.length < rule.min_members) return false;
  if (
    rule.max_bytes != null &&
    Math.max(...group.map((m) => m.size)) > rule.max_bytes
  ) {
    return false;
  }
  if (rule.exact_paths) {
    const sorted = [...paths].sort().join("\n");
    const listed = rule.exact_paths.some(
      (set) => [...set].sort().join("\n") === sorted,
    );
    if (!listed) return false;
  }
  if (rule.all_basenames) {
    const allowed = new Set(rule.all_basenames);
    if (!paths.every((p) => allowed.has(p.split("/").pop()))) return false;
  }
  if (rule.name_suffix && !paths.every((p) => p.endsWith(rule.name_suffix)))
    return false;
  if (
    rule.name_prefix &&
    !paths.every((p) => p.split("/").pop().startsWith(rule.name_prefix))
  ) {
    return false;
  }
  if (rule.all_match && !paths.every((p) => matchesAny(p, rule.all_match)))
    return false;
  if (rule.some_match && !paths.some((p) => matchesAny(p, rule.some_match)))
    return false;
  if (
    rule.each_match &&
    !rule.each_match.every((g) => paths.some((p) => matchesAny(p, [g])))
  ) {
    return false;
  }
  return true;
}

/** First rule that explains the group, or `null` when the duplicate is unexplained. */
export function explainGroup(group, rules = DEFAULT_RULES) {
  return rules.find((rule) => ruleMatches(rule, group)) || null;
}

// --- inventory -----------------------------------------------------------

/** Tracked files, repo-relative and `/`-separated. */
export function listTrackedFiles(root) {
  const out = execFileSync("git", ["-C", root, "ls-files", "-z"], {
    encoding: "buffer",
    maxBuffer: 256 * 1024 * 1024,
  });
  return out
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .map((p) => p.split(sep).join("/"))
    .sort();
}

const sha256 = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

/**
 * Group tracked files by content hash. Only regular, non-symlink files count.
 * Returns groups of two or more members, largest redundant payload first.
 */
export function findDuplicateGroups(root, files = listTrackedFiles(root)) {
  const byHash = new Map();
  for (const rel of files) {
    const abs = resolve(root, rel);
    let stat;
    try {
      stat = statSync(abs);
    } catch {
      continue;
    }
    if (!stat.isFile()) continue;
    const digest = sha256(abs);
    if (!byHash.has(digest)) byHash.set(digest, []);
    byHash.get(digest).push({ path: rel, size: stat.size });
  }
  const groups = [];
  for (const [digest, members] of byHash) {
    if (members.length < 2) continue;
    members.sort((a, b) => a.path.localeCompare(b.path));
    const redundant =
      members.reduce((sum, m) => sum + m.size, 0) - members[0].size;
    groups.push({
      digest,
      size: members[0].size,
      members,
      redundant_bytes: redundant,
    });
  }
  return groups.sort(
    (a, b) =>
      b.redundant_bytes - a.redundant_bytes || a.digest.localeCompare(b.digest),
  );
}

/** Run the full inventory and classify each group. */
export function auditDuplicateContent(
  root,
  { rules = DEFAULT_RULES, files } = {},
) {
  const tracked = files || listTrackedFiles(root);
  const groups = findDuplicateGroups(root, tracked);
  const explained = [];
  const unexplained = [];
  const unusedRuleIds = new Set(rules.map((r) => r.id));
  for (const group of groups) {
    const rule = explainGroup(group.members, rules);
    if (rule) {
      unusedRuleIds.delete(rule.id);
      explained.push({ ...group, rule_id: rule.id, reason: rule.reason });
    } else {
      unexplained.push(group);
    }
  }
  return {
    script: SCRIPT,
    root: relative(process.cwd(), root) || ".",
    tracked_files: tracked.length,
    duplicate_groups: groups.length,
    explained_groups: explained.length,
    unexplained_groups: unexplained.length,
    redundant_bytes: groups.reduce((sum, g) => sum + g.redundant_bytes, 0),
    unused_rules: [...unusedRuleIds].sort(),
    explained,
    unexplained,
    status: unexplained.length ? "fail" : "pass",
  };
}

// --- cli -----------------------------------------------------------------

function parseArgs(argv) {
  const opts = {
    root: process.cwd(),
    json: false,
    strict: false,
    receipt: null,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--root") opts.root = resolve(argv[++i]);
    else if (arg === "--json") opts.json = true;
    else if (arg === "--strict") opts.strict = true;
    else if (arg === "--receipt") opts.receipt = resolve(argv[++i]);
    else if (arg === "--help" || arg === "-h") opts.help = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  return opts;
}

function render(report) {
  const lines = [
    `duplicate content: ${report.status}`,
    `  tracked files    ${report.tracked_files}`,
    `  duplicate groups ${report.duplicate_groups} (${report.explained_groups} explained, ${report.unexplained_groups} unexplained)`,
    `  redundant bytes  ${report.redundant_bytes}`,
  ];
  for (const group of report.explained) {
    lines.push(
      `  ok   ${group.members.length}x ${group.size}B  [${group.rule_id}]`,
    );
    for (const member of group.members) lines.push(`         ${member.path}`);
  }
  for (const group of report.unexplained) {
    lines.push(
      `  FAIL ${group.members.length}x ${group.size}B  unexplained duplicate`,
    );
    for (const member of group.members) lines.push(`         ${member.path}`);
  }
  if (report.unexplained.length) {
    lines.push(
      "",
      "Add a rule to scripts/check-duplicate-content.mjs naming why each group exists,",
    );
    lines.push("or remove the duplicate copy.");
  }
  return lines.join("\n");
}

export function run(argv = process.argv.slice(2)) {
  const opts = parseArgs(argv);
  if (opts.help) {
    console.log(
      "usage: check-duplicate-content.mjs [--root DIR] [--json] [--strict] [--receipt FILE]",
    );
    return 0;
  }
  const report = auditDuplicateContent(opts.root);
  if (opts.strict) {
    report.status = report.duplicate_groups ? "fail_strict" : "pass";
    report.unexplained = report.duplicate_groups
      ? [...report.explained, ...report.unexplained]
      : [];
    report.explained = [];
  }
  if (opts.receipt) {
    mkdirSync(dirname(opts.receipt), { recursive: true });
    writeFileSync(opts.receipt, `${JSON.stringify(report, null, 2)}\n`);
  }
  console.log(opts.json ? JSON.stringify(report, null, 2) : render(report));
  return report.status === "pass" ? 0 : 1;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    process.exitCode = run();
  } catch (error) {
    console.error(`${SCRIPT}: ${error.message}`);
    process.exitCode = 2;
  }
}
