# Model setup

Use `/connect` for provider setup, `/model` to choose a route, and `/think` to
choose a supported reasoning level. `/status` shows the current session.
Provider adapters live in `cockpit/src/agent/club`; your credentials and preferences
belong in your environment or account login files.

For interactive launches, put settings in the checkout's ignored `.angel.env`
or `~/.config/host_env/system.env`, then restart Angel. For headless runs, export
settings in the caller or set `ANGEL_RUNNER_ENV_FILE=/path/to/your.env`.

| Connection | Setup | Route |
|---|---|---|
| Grok account | Run `grok login --oauth` with the Grok CLI. Angel reads `~/.grok/auth.json`; `ANGEL_GROK_OAUTH_FILE` overrides the path. | `grok` |
| Grok API | Set `XAI_API_KEY` and optionally `ANGEL_GROK_API_MODEL` / `ANGEL_GROK_API_URL`. | `grok-api` |
| ChatGPT account | Run `codex login`. Angel reads `~/.codex/auth.json` and the local model catalog; `CODEX_HOME` overrides the directory. | `openai` |
| OpenAI API | Set `OPENAI_API_KEY` and `ANGEL_OPENAI_API_MODEL`. Optional endpoint: `ANGEL_OPENAI_API_URL`. | `openai-api` |
| GLM | Set `ANGEL_GLM_KEY` or the endpoint's matching key, plus `ANGEL_GLM_URL` and `ANGEL_GLM_MODEL` when overriding defaults. | `glm` |
| DeepSeek | Set `DEEPSEEK_API_KEY`. Optional settings: `ANGEL_DEEPSEEK_URL`, `ANGEL_DEEPSEEK_MODEL`. | `deepseek` |
| OpenRouter | Set `OPENROUTER_API_KEY` and `ANGEL_OPENROUTER_MODEL`. Optional endpoint: `ANGEL_OPENROUTER_URL`. | `openrouter` |
| Local server | Set `ANGEL_LOCAL_URL` to an OpenAI-compatible base URL. Set `ANGEL_LOCAL_MODEL`, or let `/models` discovery identify the served model. | `local` |

Select a startup route with `ANGEL_DRIVER`, for example:

```sh
ANGEL_DRIVER=local ANGEL_LOCAL_URL=http://127.0.0.1:8080/v1 ./bin/angel0
```

OAuth and API routes have separate labels and credentials. An API key does not
replace the account login. Grok's optional `grok_research` tool uses the CLI
OAuth session; the `grok-api` driver uses native HTTP tool calls.

Configured API providers are available by default. To restrict them, set
`ANGEL_API_CLUBS`: `glm,grok` allows those API providers; `none` or an explicitly
empty value disables API routes; unset, `*`, or `all` allows configured providers.
This setting does not disable account OAuth or local routes. `/connect` shows
the current policy. Availability in the model list is not a completed live test.

Consultation and delegation use configured models by default. `/solo` restricts
remote consultation; `ANGEL_ALLOW_SOTA_CONSULT=0` or
`ANGEL_ALLOW_SOTA_DELEGATE=0` restricts the corresponding tools. Autonomous loops
prefer the selected model; `ANGEL_LOOP_SOTA_CLUB` pins an escalation route and
`ANGEL_LOOP_SOTA=0` disables that escalation. Configure budgets and run limits
for your workload in the [environment reference](../cockpit/docs/ENV.md).

Other providers, endpoint aliases, fleet discovery, timeouts and model-specific
controls are listed in the [environment reference](../cockpit/docs/ENV.md).
