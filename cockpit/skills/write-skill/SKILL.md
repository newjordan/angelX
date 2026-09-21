---
name: write-skill
description: Author a new skill for this cockpit — where it lives, the SKILL.md format, and what separates a playbook the agent actually uses from a dead essay.
---

# Write a skill

A skill is a playbook loaded on demand: its one-line description rides in every
system prompt; its body loads only when `skill(name)` is called. Write both for
that economy.

1. **Where it lives** (later sources override earlier by name):
   - Bundled with the cockpit: `cockpit/skills/<name>/SKILL.md` — ships to
     every checkout; reserve for skills worth versioning with the code.
   - Per-repo: `<repo>/.agents/skills/<name>/SKILL.md` — conventions for one
     project.
   - Personal: `~/.angelX/skills/<name>/SKILL.md` (or flat `<name>.md`;
     `ANGEL_SKILLS_DIR` overrides) — the user's own overrides, wins over all.
2. **Format:** `---` frontmatter with `name:` and `description:`, then the
   body. The description is the trigger — it must say *when to reach for
   this* in one line, because that line is all the model sees until it loads
   the skill. "Bring up a local inference server and hand back a measured
   endpoint" triggers; "Notes about LLM serving" never will.
3. **Body = numbered playbook, not an essay.** Each step names the concrete
   tool or command (`proc_run`, `gpu_stat`, `llama-quantize …`). Include the
   failure branches — what OOM means here, what to do when the readiness line
   never appears — because that's where a generic agent actually derails.
   30–50 lines; past that, split the skill.
4. **Encode postures explicitly.** If the domain has hard rules (read-only
   fleet recon; never destroy a paid instance; verify quant quality before
   shipping), state them as rules in the skill — a skill is exactly the place
   standing guidance lives.
5. **Test the loop:** confirm the skill appears in `/skills`, load it with
   `skill(<name>)`, and run one real task with it. A skill that hasn't guided
   one task is a draft.
