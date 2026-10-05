# Model setup

Run `angelX setup` to connect a model. It lists what this machine already has
(a model server on localhost such as Ollama, LM Studio, llama.cpp or vLLM, an
API key in your environment, a ChatGPT or Grok login), lets you paste a key or
type a server address, and saves the choice to `~/.angelX/angel.env`. With
nothing configured anywhere, the first interactive launch runs it by itself;
`ANGEL_SETUP=0` turns that off.

In the cockpit, use `/connect` for provider setup, `/model` to choose a route,
and `/think` to choose a supported reasoning level. `/status` shows the current
session. Provider adapters live in `cockpit/src/agent/club`; your credentials
and preferences belong in your environment or account login files.

For interactive launches, settings load from `~/.angelX/angel.env`, then from
the checkout's ignored `.angel.auto.env` and `.angel.env` (the checkout wins);
restart Angel after a change. The launcher also reads `~/.config/host_env/*`,
`~/.env` and `~/.openrouter.env`, but only when `ANGEL_HOST_ENV=1` is exported
or set in one of those files. For headless runs, export settings in the caller
or set `ANGEL_RUNNER_ENV_FILE=/path/to/your.env`.

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
ANGEL_DRIVER=local ANGEL_LOCAL_URL=http://127.0.0.1:8080/v1 ./bin/angelX
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

## Treebeard compactor

In the Treebeard lane (the default), a local model can work beside a paid
driver as its rolling compactor. When the lane parks a large tool result under
a handle, the compactor reads it and writes the driver a short digest in the
receipt, so the details stay local. It also writes the background compaction
summaries. Without one, receipts stay bare and compaction stays in-hand.

Recommended model:
[unsloth/Qwen3.6-35B-A3B-GGUF](https://huggingface.co/unsloth/Qwen3.6-35B-A3B-GGUF),
file `Qwen3.6-35B-A3B-UD-Q5_K_XL.gguf` (26.6 GB, Apache-2.0). It is a
mixture-of-experts model with about 3B active parameters, fast enough to digest
inside a hop. Serve it with llama.cpp b9743 or newer; older builds lack its
`qwen35moe` architecture.

```sh
hf download unsloth/Qwen3.6-35B-A3B-GGUF Qwen3.6-35B-A3B-UD-Q5_K_XL.gguf --local-dir ~/models/treebeard
llama-server -m ~/models/treebeard/Qwen3.6-35B-A3B-UD-Q5_K_XL.gguf \
  --host 127.0.0.1 --port 8001 -ngl 99 -c 131072 -np 4 --kv-unified \
  -fa on --jinja --reasoning-budget 0 -a treebeard
```

Then, in `.angel.env`:

```sh
ANGEL_COMPACT_URL=http://127.0.0.1:8001/v1
ANGEL_COMPACT_MODEL=treebeard
ANGEL_COMPACT_REASONING_DIALECT=qwen
ANGEL_COMPACT_REASONING_EFFORT=none
ANGEL_COMPACT_BG_TIMEOUT_SECS=120
```

`ANGEL_TREEBEARD_DIGEST=0` turns the digests off; see the
[environment reference](../cockpit/docs/ENV.md). Nothing cuts a digest off: the
hop waits for every one it started.

Other providers, endpoint aliases, fleet discovery, timeouts and model-specific
controls are listed in the [environment reference](../cockpit/docs/ENV.md).
