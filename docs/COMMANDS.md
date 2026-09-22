# /commands

Enter commands in the cockpit composer. `/help all` shows built-in help; `Tab`
completes a command. Brackets below mean optional arguments. Commands that launch
model work use the configured routes and can incur provider charges.

## Everyday work

| Command | Behavior |
|---|---|
| `/help [topic\|all]`, `/?` | Show command help. |
| `/connect [grok\|openai\|glm\|deepseek\|openrouter\|local]` | Show model setup instructions and current provider policy. |
| `/model [filter\|exact@effort\|auto]` | Open model selection or choose a route. |
| `/think [filter]` | Choose a supported thinking level; aliases `/thinking`, `/effort`. |
| `/status`, `/usage`, `/context [all]` | Inspect session state, reported usage and retained context. |
| `/mcp`, `/tools` | Inspect available tools and MCP connections. |
| `/skills [check\|search <query>\|<name>[,<name>...] [task]]` | Inspect, find or select playbooks. |
| `/cd <path>`, `/workspace <path>` | Change project and start a fresh model thread. |
| `/mention <file>` | Attach workspace file context. |
| `/diff [all\|staged] [--stat]`, `/review` | Inspect changes or request a review. |
| `/check [args]`, `/test [args]`, `/lint [args]`, `/verify [args]` | Run local checks; `/verify` runs the verification ladder. |
| `/build [args]`, `/run [args]`, `/bench [args]`, `/doc [args]`, `/tree [args]` | Run the corresponding Cargo operation. |
| `/fmt [check\|write]` | Check formatting or apply formatter changes. |
| `/diagnostics <file>`, `/symbols <file>`, `/symbol <query>` | Request LSP diagnostics or symbols; requires an available server. |
| `/definition <file> <symbol>`, `/references <file> <symbol>`, `/hover <file> <symbol>` | Query LSP source relationships or documentation. |
| `/init` | Create project instructions. |
| `/plan`, `/personality <style>` | Set planning or response guidance. |
| `/rate useful\|miss` | Record feedback on the last answer. |
| `/stop` | Interrupt active work; `Esc` / `Ctrl-C` also soft-interrupt. |
| `/ps` | Inspect background work. |

Native Cargo verifiers reject workspace-controlled Cargo/toolchain configuration.
For those projects, use an explicitly pinned command through the shell tool;
the refusal is not a successful check.

## Sessions and context

| Command | Behavior |
|---|---|
| `/new`, `/clear` | Start a new conversation. |
| `/sessions`, `/resume [id]` | List or resume current-project sessions; no ID selects the latest. |
| `/save`, `/rename <name>`, `/fork` | Save, rename or fork the session. |
| `/archive`, `/delete` | Archive or delete the current session. |
| `/history [1-50]`, `/retry`, `/undo`, `/redo` | Inspect or revise conversation history. |
| `/compact` | Compact retained conversation context. |
| `/copy [number\|all\|live\|code [number]]` | Copy a response or code block. |
| `/raw` | Export visible user/assistant conversation to a local file. |
| `/memories [add <text>\|forget <n>\|clear]`, `/recall [query]` | Manage or retrieve project memory. |
| `/dossier`, `/ledger [N]`, `/turns [N]` | Inspect memory state or recorded turn evidence. |
| `/atlas [query]`, `/vault` | Open reviewed project knowledge or its artifact view. |
| `/kg [stats]` | Inspect the workspace's typed knowledge graph. |
| `/atlas observe <path> [symbol]`, `/atlas trace <id>` | Record a source observation or inspect its links. |
| `/atlas add`, `/atlas accept`, `/atlas revise`, `/atlas reject`, `/atlas link`, `/atlas promote`, `/atlas challenge`, `/atlas forget`, `/atlas tend` | Knowledge editing/review verbs; invoke a verb for its argument help. |
| `/refine [status\|add …\|del <id>\|rollback <event>\|seed-light]` | Manage supplemental harness state. |
| `/btw [topic]` | Enter or leave an ephemeral side thread. |
| `/import [<n>\|latest]` | Import a recent local Codex conversation. |
| `/exit`, `/quit` | Save and exit; active work can defer shutdown. |

## Goals and research

| Command | Behavior |
|---|---|
| `/goal [text]` | Inspect or set the durable objective. |
| `/goal go`, `/goal tick`, `/goal rounds <N\|unset>` | Advance work or set its round bound. |
| `/goal criteria <text>`, `/goal cmd <check>`, `/goal note <text>` | Set acceptance criteria, verifier or a note. |
| `/goal pause`, `/goal resume`, `/goal blocked <reason>`, `/goal done`, `/goal clear` | Control objective state. |
| `/loop [iterations=N] <task>` | Start an autonomous loop; `iterations=N` sets an optional bound. `/loop` alone opens its workshop. |
| `/loop status`, `/loop pause`, `/loop resume`, `/loop stop` | Inspect or control the loop. |
| `/loop deli`, `/loop podrace <task>`, `/loop endless [task]` | Select optional loop modes; `endless` removes operator caps. |
| `/handoff-rl [iterations=N] <task>` | Start a competition loop with context clearing and evidence reinjection. |
| `/moa`, `/moa ledger`, `/moa <message>` | Configure a formation, inspect its ledger or use an armed roster. |
| `/graph [list\|run <name> <task>\|status\|stop]` | Inspect or run declared agent graphs. |
| `/campaign [status\|new <objective>\|import-goal\|criterion …\|start\|advance\|review\|pause\|resume\|abandon]` | Manage a project proof contract and review gate. |
| `/rl [run [flags]\|status\|stop]`, `/reinforce` | Open reinforcement views or run a measured campaign. `/rl run --help` lists flags. |
| `/self [goal\|status\|integrate\|discard\|reborn]` | Edit the cockpit in a separate worktree; integration requires approval. |
| `/solo [on\|off\|status]` | Disable paid consultant/delegate paths for self-managed work. |
| `/relentless [on\|off\|status]` | Enable execution nudges for stalled turns. |
| `/research [keep\|table\|smithy\|observatory\|library]` | Navigate the research workspace. |
| `/observatory [campaign <id>\|open <report-id>]` | Inspect campaign/report evidence. |
| `/science [query]`, `/repos [query]` | Start literature or GitHub repository searches. |
| `/learn [topic]`, `/library [topic]`, `/tutor [topic]` | Open the Librarium or start a lesson. |
| `/ask [question]`, `/practice` | Ask the tutor or revisit recall prompts. |
| `/quest [theme\|sample\|lex]`, `/quest gauntlet <question>` | Inspect reasoning maps or explicitly query multiple models. |
| `/cut`, `/habits`, `/conductor`, `/still`, `/village` | Inspect authored changes, improvement state or configured fleet state. |

`benchmark_compare`, `loop_research` and `rl_campaign` are model tools, not
slash commands. See [environment settings](../cockpit/docs/ENV.md).

## Display and execution controls

| Command | Behavior |
|---|---|
| `/show <image\|video\|report>`, `/hide` | Open or close Scryglass media; video requires a video-enabled build. |
| `/see <image>`, `/hear <audio>` | Attach media to the model flow; route capabilities apply. |
| `/open <n\|url\|module>`, `/media` | Open a media card, URL or module; browse media. |
| `/modules`, `/close <module>`, `/layout save\|load <name>` | Inspect modules or save/restore pane layout. |
| `/world [ride\|enter\|leave\|weather\|zoom]`, `/world help` | Navigate Dotmax scenery and room views. |
| `/raytrace`, `/cube` | Display the debug cube. |
| `/tourney calibrate <scene>` | Display a calibration animation; it does not indicate a won run. |
| `/trace`, `/rollout` | Toggle the tool transcript; `/rollout` shows the current session file path. |
| `/redraw` | Repaint the terminal; also `Ctrl-L`. |
| `/keymap [key action]`, `/vim [on\|off]` | Inspect/remap keys or use modal composition. |
| `/statusline <text>`, `/title <text>`, `/pet <name>` | Set the header item, terminal title or header pet glyph. |
| `/sandbox`, `/debug-config`, `/hooks` | Inspect sandbox, configuration or hooks. |
| `/approvals [on\|off\|probe\|selftest]` | Control or exercise the approval gate. |
| `/yolo [on\|off\|status\|smart\|full]`, `/yolos [on\|off\|status]` | Change execution authority. Read [SECURITY.md](../SECURITY.md) first. |
| `/experimental` | Inspect or toggle optional experiments. |

Compatibility names with limited behavior: `/setup-default-sandbox` and
`/sandbox-add-read-dir` only display sandbox status; `/approve` and
`/test-approval` direct you to `/approvals`; `/theme` does not change the palette.
`/logout` gives credential-location guidance. `/ide`, `/app`, `/plugins` and
`/feedback` have no corresponding cockpit subsystem. `/selftest` aliases `/solo`.

Sources: [parser and completion](../cockpit/src/ui/input.rs),
[command dispatch](../cockpit/src/app/control/commands.rs),
[built-in help](../cockpit/src/app/local_command.rs).
