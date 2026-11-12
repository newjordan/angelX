# Plugins

A plugin adds slash commands, skills, tools or book pages to angelX. Anyone can
write one and publish it in the registry; anyone can install it with one
command. Under the hood a plugin is a config-only [cartridge](CARTRIDGES.md): a
folder with a `cartridge.toml`, kept in a public git repository.

## Using plugins

In the cockpit:

```text
/plugins                    the plugins installed here
/plugins search pixel       the registry, or the plugins matching every word
/plugins info vfx           fetch one and show what it carries, before installing
/plugins install vfx        install it; its /command works at once
/plugins update             bring every installed plugin to the registry's version
/plugins remove vfx         remove it
```

The same from a shell, as `angelX plugin <command>` (`angelX plugin help` lists
them all). `install` and `update` take `--no-setup`; `install` and `remove`
take `--force` for a cartridge that was put in place by hand.

An install puts the plugin in `~/.angelX/cartridges/<id>`. Its commands, skills
and pages work as soon as it lands, in a cockpit that is already running. A
tool joins the next session angelX starts, because a session's tool list is
built once.

`/plugins info` shows what a plugin carries: its commands, tools and setup, its
license and authors, and a link to its setup script. A plugin runs with your
permissions, as every angelX tool does, so read what it does first.

### What an install checks

- The registry pins each plugin to one commit and the digest of its files at
  that commit. The install fetches only the plugin's folder at that commit and
  refuses it if the digest differs. Nothing in it has run at that point.
- The plugin must load: `cartridge.toml` parses, every command's skills and
  every tool's program are in the folder, nothing links outside it, and its id
  and version match the registry.
- A plugin can ask for a lowest angelX version (`angelx = "0.3.0"`). An older
  angelX refuses it and says to run `angelX update`.
- Then the setup runs, if the plugin has one (see below). If it fails, the
  files stay in place and `angelX plugin setup <id>` runs it again.

An update replaces the plugin's files and removes the ones the new version
dropped. Anything else in the folder stays: whatever the setup fetched or
built, and any data the plugin keeps. A setup that finds its work done can skip
it.

## Writing a plugin

```sh
angelX plugin new hello        # ./hello: a plugin that works, to grow from
angelX plugin link hello       # angelX loads it from that folder while you edit
angelX plugin check hello      # whether it loads, and whether the registry takes it
```

`new` writes a `cartridge.toml` with one command, one skill and one tool, the
tool's program, a README and an MIT license. It passes `check` before you
change a line. In angelX, `/hello <task>` sends a turn that carries the skill,
and the agent can call `hello_hello`.

What a plugin can carry is everything a config-only cartridge can, which is set
out in [Cartridges](CARTRIDGES.md):

- `[[commands]]`: `/<name> <task>` sends one turn with the named skills from
  `skills/`.
- `[[tools]]`: a program in the folder that reads its arguments as JSON on stdin
  and prints its result. It waits behind `tool_search` unless `core = true`.
- `[[pages]]`: book pages, by route.
- A competition: `boards`, `measures`, `worker`, `fleet_command`.

A plugin cannot carry Rust (`src/mod.rs`). A source cartridge needs a rebuild
of angelX, so it stays a [source cartridge](CARTRIDGES.md#a-source-cartridge).

### The `[plugin]` table

```toml
id = "hello"
label = "Hello"

[plugin]
version = "0.1.0"                 # MAJOR.MINOR.PATCH, optionally -pre
description = "One line on what /hello does."
authors = ["Ada Lovelace"]
license = "MIT"                   # an SPDX expression
repository = "https://github.com/ada/hello"   # optional: else the git remote
homepage = "https://ada.dev/hello"            # optional
keywords = ["greeting"]                       # optional, for search
angelx = "0.3.0"                  # optional: the lowest angelX it works with
setup = "setup.sh"                # optional: run after install and each update
```

The id names the plugin's folder: 2 to 32 lowercase letters, digits and `-`,
starting with a letter. A command cannot use the name of one of angelX's own
commands, because the built-in always answers. `check` says when one does.

### Setup

Leave things out of the repository that should not be in it, such as a
binary, a model or `node_modules`. `setup` names a program in the folder that
fetches or builds them. It runs in the plugin's folder, with that folder as its
first argument and in `ANGEL_CARTRIDGE_DIR`, and with `ANGEL_PLUGIN_ID` set.
Its output shows in the terminal, or in the cockpit's job panel. Esc cancels
it.

A tool program or skill that the setup makes is allowed to be missing from the
repository. `check` notes it, and the setup has to make it. The examples'
`install.sh` scripts work as setups unchanged.

## Publishing

Push the plugin to a public repository. It can be the whole repository or one
folder in it. Then, from its folder:

```sh
angelX plugin submit
```

`submit` fetches the plugin back from the repository at the commit you pushed,
the way an install will, and checks it. It builds the registry entry from what
it fetched, not from your working folder. Then, through the
[GitHub CLI](https://cli.github.com) and as you, it forks
`newjordan/angelX`, puts the entry on a branch and opens the pull request.

- The plugin's folder must have no uncommitted changes, and the commit must be
  pushed. `--commit <sha>` submits another commit.
- `--repository <url>` names the repository when the git remote is not it.
- `--print` prints the entry instead, to paste into a pull request opened on
  the web. `--into <file>` writes it into a registry file of your own.

To publish a new version, bump `version`, push, and `submit` again. The pull
request replaces your entry. An id belongs to the repository that first
published it.

### The registry

[`plugins/registry.toml`](https://github.com/newjordan/angelX/blob/main/plugins/registry.toml) has one table per plugin:

```toml
[hello]
label = "Hello"
version = "0.1.0"
description = "One line on what /hello does."
repository = "https://github.com/ada/hello"
commit = "<the full commit id>"
path = ""                      # the plugin's folder in the repository; "" for its root
sha256 = "<the plugin tree digest at that commit>"
license = "MIT"
authors = ["Ada Lovelace"]
```

The fields from `label` through `authors` must match the plugin's own
`cartridge.toml` at that commit.

### Review

A maintainer checks a registry pull request on its branch:

```sh
python3 scripts/plugins/check-registry.py --changed origin/main
```

For each entry that differs from `main`, it fetches the plugin at its commit,
recomputes the digest, and checks the plugin against its entry and against the
rules above. It also checks that no two plugins claim the same command or tool
name. It never runs what it fetches. `ANGEL_PLUGIN_REGISTRY=plugins/registry.toml
angelX plugin info <id>` then shows the plugin the way an install will.

The maintainer then reads the plugin: its setup, its tool programs, what its
skills tell the agent. The maintainer merges when it does what it says. A
plugin that does something its description does not say is not merged.

### The digest

```text
angelx-plugin-tree/v1\n
<kind> <path>\0<sha256 of the content, hex>\n     one line per file, sorted by path bytes
```

`kind` is `f` for a file, `x` for an executable file and `l` for a symlink,
whose content is its target. Paths are relative and `/`-separated. The SHA-256
of the whole text is the digest. It is read from git objects, so
`.gitattributes` filters cannot change it. `angelX plugin check` prints a
folder's digest.

## Other registries

`ANGEL_PLUGIN_REGISTRY` points angelX at another registry: an https URL, or a
file. A private registry can name `file://` or local repositories.
`angelX plugin submit --into <file>` writes entries into it. `ANGEL_PLUGIN_DIR`
moves where plugins install. That folder must be one the cartridge loader reads
(`ANGEL_CARTRIDGES`), and angelX warns when it is not.

The last registry fetched is cached in `~/.angelX/plugins/registry.toml`.
Offline, `search` reads that copy and says so.
