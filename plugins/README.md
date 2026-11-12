# The plugin registry

[`registry.toml`](registry.toml) lists every published angelX plugin. Each
entry pins a plugin to one reviewed commit and the digest of its files there.

- **Install one:** `/plugins install <id>` in angelX, or `angelX plugin install <id>`.
- **Publish yours:** push it to a public repository, then run
  `angelX plugin submit` in its folder. That opens the pull request that adds
  its entry here.
- **Review a submission:** on the pull request's branch,
  `python3 scripts/plugins/check-registry.py --changed origin/main` fetches
  each changed plugin at its commit, recomputes the digest and checks it
  against its entry. `ANGEL_PLUGIN_REGISTRY=plugins/registry.toml angelX plugin info <id>`
  shows it the way an install will. Then read the code: its setup, its tool
  programs and what its skills tell the agent.

The whole story is in [docs/PLUGINS.md](../docs/PLUGINS.md).
