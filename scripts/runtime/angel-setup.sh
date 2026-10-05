#!/usr/bin/env bash
# angel-setup.sh — `angelX setup`: connect a model.
#
# Finds what this machine already has (model servers running on localhost, API
# keys in the environment, ChatGPT or Grok plan logins), lets the operator pick
# one or add a key or a server, and saves the choice to ~/.angelX/angel.env.
# bin/angelX loads that file on every interactive launch, and runs this script
# by itself once (--first-run) when nothing at all is configured.
#
# Plain bash 3.2 and curl, no python: it has to run on a stock Mac.
set -euo pipefail

first_run=0
[ "${1:-}" = --first-run ] && first_run=1
settings="$HOME/.angelX/angel.env"
settings_shown="~/.angelX/angel.env"

say() { printf '%s\n' "$*"; }

# ask PROMPT [DEFAULT] → REPLY. End of input ends setup.
ask() {
  if [ -n "${2:-}" ]; then printf '%s [%s]: ' "$1" "$2"; else printf '%s: ' "$1"; fi
  IFS= read -r REPLY || { printf '\n'; exit 1; }
  [ -n "$REPLY" ] || REPLY="${2:-}"
}

ask_secret() {
  printf '%s: ' "$1"
  IFS= read -rs REPLY || { printf '\n'; exit 1; }
  printf '\n'
  REPLY=$(printf '%s' "$REPLY" | tr -d '[:space:]')
}

have_curl() { command -v curl >/dev/null 2>&1; }

# http_get URL [KEY] → HTTP_CODE and HTTP_BODY. The key travels to curl on
# stdin, never on the command line where other users could read it.
http_get() {
  local body code
  body=$(mktemp)
  if [ -n "${2:-}" ]; then
    code=$(printf 'header = "Authorization: Bearer %s"\n' "$2" \
      | curl -s -K - -o "$body" -w '%{http_code}' --connect-timeout 5 --max-time 20 "$1" 2>/dev/null) || true
  else
    code=$(curl -s -o "$body" -w '%{http_code}' --connect-timeout 2 --max-time 10 "$1" 2>/dev/null) || true
  fi
  HTTP_CODE="${code:-000}"
  HTTP_BODY=$(cat "$body")
  rm -f "$body"
}

# Model ids of an OpenAI-style /models answer, in order.
model_ids() { tr ',{}[]' '\n\n\n\n\n' | sed -n -E 's/^ *"id" *: *"([^"]*)" *$/\1/p'; }

# envval NAME: a variable's value by name (names come from the fixed tables here).
envval() { eval "printf '%s' \"\${$1:-}\""; }

# --- saving ----------------------------------------------------------------
sq() { printf '%s' "$1" | sed "s/'/'\\\\''/g"; }

# save NAME VALUE [NAME VALUE …]: replace those names in angel.env, keep the rest.
save() {
  local tmp names="" i
  mkdir -p "$HOME/.angelX"
  tmp=$(mktemp "$HOME/.angelX/.angel.env.XXXXXX")
  chmod 600 "$tmp"
  i=1
  while [ $i -le $# ]; do
    names="$names${names:+|}${!i}"
    i=$((i + 2))
  done
  if [ -f "$settings" ]; then
    grep -v -E "^[[:space:]]*(export[[:space:]]+)?($names)=" "$settings" > "$tmp" || true
  else
    printf '%s\n' "# angelX settings. \`angelX setup\` writes this file; edit it freely." \
      "# bin/angelX loads it on every interactive launch." > "$tmp"
  fi
  while [ $# -ge 2 ]; do
    printf "%s='%s'\n" "$1" "$(sq "$2")" >> "$tmp"
    shift 2
  done
  mv -f "$tmp" "$settings"
}

saved() {
  say ""
  say "Connected: $1. Saved to $settings_shown; run \`angelX setup\` to change it."
}

# pick_model IDS DEFAULT_NUMBER → MODEL. One model is picked without asking.
pick_model() {
  local ids="$1" count n=0 id
  count=$(printf '%s\n' "$ids" | grep -c . || true)
  if [ "$count" -le 1 ]; then
    MODEL=$(printf '%s\n' "$ids" | head -n 1)
    return
  fi
  say ""
  say "Models:"
  while IFS= read -r id; do
    n=$((n + 1))
    [ $n -le 30 ] || { say "  … and $((count - 30)) more (type a name)"; break; }
    printf '  %2d  %s\n' "$n" "$id"
  done <<< "$ids"
  ask "Model (number or name)" 1
  case "$REPLY" in
    *[!0-9]*) MODEL="$REPLY" ;;
    *) MODEL=$(printf '%s\n' "$ids" | sed -n "${REPLY}p"); [ -n "$MODEL" ] || MODEL=$(printf '%s\n' "$ids" | head -n 1) ;;
  esac
}

# --- what this machine has -------------------------------------------------
# Default ports of the common local servers (ANGEL_SETUP_PORTS replaces the
# list; empty probes none). The answer's owned_by names the server when it can;
# otherwise the port's usual owner does.
local_ports="${ANGEL_SETUP_PORTS-11434 1234 8080 8000 30000 1337 5000 5001}"
port_owner() {
  case "$1" in
    11434) echo Ollama ;; 1234) echo "LM Studio" ;; 8080) echo llama.cpp ;;
    8000) echo vLLM ;; 30000) echo SGLang ;; 1337) echo Jan ;; *) echo "model server" ;;
  esac
}
served_by() {
  case "$1" in
    library) echo Ollama ;; organization_owner) echo "LM Studio" ;; llamacpp) echo llama.cpp ;;
    vllm) echo vLLM ;; sglang) echo SGLang ;; *) echo "" ;;
  esac
}

LOCAL_NAMES=()
LOCAL_URLS=()
LOCAL_MODELS=()
detect_local() {
  have_curl || return 0
  local dir port ids owner name
  dir=$(mktemp -d)
  for port in $local_ports; do
    curl -s --connect-timeout 0.5 --max-time 3 -o "$dir/$port" "http://127.0.0.1:$port/v1/models" 2>/dev/null &
  done
  wait
  for port in $local_ports; do
    [ -s "$dir/$port" ] || continue
    ids=$(model_ids < "$dir/$port")
    [ -n "$ids" ] || continue
    owner=$(tr ',{}' '\n\n\n' < "$dir/$port" | sed -n -E 's/^ *"owned_by" *: *"([^"]*)" *$/\1/p' | head -n 1)
    name=$(served_by "$owner")
    [ -n "$name" ] || name=$(port_owner "$port")
    LOCAL_NAMES+=("$name")
    LOCAL_URLS+=("http://127.0.0.1:$port/v1")
    LOCAL_MODELS+=("$ids")
  done
  rm -rf "$dir"
}

# driver|name|key variables the cockpit reads|model variables it needs (the first is saved)
seats="deepseek|DeepSeek|ANGEL_DEEPSEEK_KEY DEEPSEEK_API_KEY|
openai-api|OpenAI API|ANGEL_OPENAI_KEY OPENAI_API_KEY|ANGEL_OPENAI_API_MODEL OPENAI_MODEL
grok-api|xAI Grok API|ANGEL_XAI_KEY XAI_API_KEY GROK_API_KEY|
glm|Z.ai GLM|ANGEL_GLM_KEY GLM_API_KEY ZAI_API_KEY ZHIPU_API_KEY BIGMODEL_API_KEY|
kimi|Kimi|ANGEL_KIMI_KEY KIMI_API_KEY MOONSHOT_API_KEY|
openrouter|OpenRouter|ANGEL_OPENROUTER_KEY OPENROUTER_API_KEY|ANGEL_OPENROUTER_MODEL OPENROUTER_MODEL
cerebras|Cerebras|ANGEL_CEREBRAS_KEY CEREBRAS_API_KEY|ANGEL_CEREBRAS_MODEL CEREBRAS_MODEL
meta|Meta Muse|ANGEL_META_KEY META_API_KEY|
qwen|Qwen Coding Plan|ANGEL_QWEN_KEY QWEN_API_KEY DASHSCOPE_API_KEY|
longcat|LongCat|ANGEL_LONGCAT_KEY LONGCAT_API_KEY|"

ENV_SEATS=()
detect_env_keys() {
  local driver name vars model var
  while IFS='|' read -r driver name vars model; do
    for var in $vars; do
      if [ -n "$(envval "$var")" ]; then
        ENV_SEATS+=("$driver|$name|$var|$model")
        break
      fi
    done
  done <<< "$seats"
}

codex_auth="${CODEX_HOME:-$HOME/.codex}/auth.json"
grok_auth="${GROK_HOME:-$HOME/.grok}/auth.json"

# --- choices ---------------------------------------------------------------
use_local() { # URL MODEL_IDS NAME
  pick_model "$2"
  save ANGEL_LOCAL_URL "$1" ANGEL_LOCAL_MODEL "$MODEL" ANGEL_DRIVER local
  saved "$3, $MODEL"
  if [ "$3" = Ollama ]; then
    say "Ollama starts models with a short context. For coding, start it with"
    say "OLLAMA_CONTEXT_LENGTH=32768 (or more) so long files fit."
  fi
}

# A seat that must be told which model to use: list what the key can reach.
seat_model() { # DRIVER URL KEY → MODEL ("" when the seat has a default)
  MODEL=""
  case "$1" in
    openai-api | cerebras)
      http_get "$2/models" "$3"
      local ids
      # Chat models only; the list also carries embedding, speech and image models.
      ids=$(printf '%s' "$HTTP_BODY" | model_ids \
        | grep -v -E 'embed|tts|whisper|dall-e|image|audio|realtime|moderation|transcribe|search' | sort || true)
      if [ -n "$ids" ]; then pick_model "$ids"; else ask "Model name"; MODEL="$REPLY"; fi
      ;;
    openrouter)
      ask "OpenRouter model" "${ANGEL_OPENROUTER_MODEL:-tencent/hy3:free}"
      MODEL="$REPLY"
      ;;
  esac
}

use_env_seat() { # "driver|name|var|modelvars"
  local driver name var modelvars mv have=""
  IFS='|' read -r driver name var modelvars <<< "$1"
  for mv in $modelvars; do [ -z "$(envval "$mv")" ] || have=1; done
  if [ -n "$modelvars" ] && [ -z "$have" ]; then
    seat_model "$driver" "$(seat_url "$driver")" "$(envval "$var")"
    if [ -n "$MODEL" ]; then
      save ANGEL_DRIVER "$driver" "${modelvars%% *}" "$MODEL"
      saved "$name, $MODEL (key from \$$var)"
      return
    fi
  fi
  save ANGEL_DRIVER "$driver"
  saved "$name (key from \$$var)"
}

seat_url() {
  case "$1" in
    deepseek) echo https://api.deepseek.com/v1 ;;
    openai-api) echo "${OPENAI_BASE_URL:-https://api.openai.com/v1}" ;;
    grok-api) echo https://api.x.ai/v1 ;;
    glm) echo https://api.z.ai/api/coding/paas/v4 ;;
    kimi) echo https://api.moonshot.ai/v1 ;;
    kimi-code) echo https://api.kimi.com/coding/v1 ;;
    openrouter) echo https://openrouter.ai/api/v1 ;;
    cerebras) echo https://api.cerebras.ai/v1 ;;
    meta) echo https://api.meta.ai/v1 ;;
  esac
}

paste_key() {
  local choice driver name var console url key extra=()
  say ""
  say "Which provider?"
  say "  1  DeepSeek          platform.deepseek.com"
  say "  2  OpenAI            platform.openai.com"
  say "  3  xAI Grok          console.x.ai"
  say "  4  Z.ai GLM          z.ai"
  say "  5  Kimi              platform.moonshot.ai"
  say "  6  Kimi Code plan    kimi.com/code"
  say "  7  OpenRouter        openrouter.ai"
  say "  8  Cerebras          cloud.cerebras.ai"
  ask "Provider"
  choice="$REPLY"
  case "$choice" in
    1) driver=deepseek name=DeepSeek var=DEEPSEEK_API_KEY ;;
    2) driver=openai-api name=OpenAI var=OPENAI_API_KEY ;;
    3) driver=grok-api name="xAI Grok" var=XAI_API_KEY ;;
    4) driver=glm name="Z.ai GLM" var=ZAI_API_KEY ;;
    5) driver=kimi name=Kimi var=MOONSHOT_API_KEY ;;
    6) driver=kimi-code name="Kimi Code plan" var=KIMI_API_KEY ;;
    7) driver=openrouter name=OpenRouter var=OPENROUTER_API_KEY ;;
    8) driver=cerebras name=Cerebras var=CEREBRAS_API_KEY ;;
    *) say "No provider $choice."; return 1 ;;
  esac
  url=$(seat_url "$driver")
  while :; do
    ask_secret "Paste your $name key (hidden)"
    key="$REPLY"
    [ -n "$key" ] || return 1
    case "$key" in *[!A-Za-z0-9._~+/=:-]*) say "That key has characters no API key uses; check the paste."; continue ;; esac
    have_curl || break
    # OpenRouter lists models for anyone; its key endpoint is the one that checks.
    if [ "$driver" = openrouter ]; then http_get "$url/key" "$key"; else http_get "$url/models" "$key"; fi
    case "$HTTP_CODE" in
      2??) say "$name accepted the key."; break ;;
      401 | 403)
        say "$name rejected the key (HTTP $HTTP_CODE)."
        ask "Try again? (y/n)" y
        case "$REPLY" in [Nn]*) return 1 ;; esac
        ;;
      *) say "Could not check the key (HTTP $HTTP_CODE); saving it anyway."; break ;;
    esac
  done
  if [ "$driver" = kimi-code ]; then
    save KIMI_API_KEY "$key" KIMI_API_URL "$url" KIMI_MODEL k3 ANGEL_DRIVER kimi
    saved "Kimi Code plan, k3"
    return
  fi
  seat_model "$driver" "$url" "$key"
  case "$driver" in
    openai-api) extra=(ANGEL_OPENAI_API_MODEL "$MODEL") ;;
    openrouter) extra=(ANGEL_OPENROUTER_MODEL "$MODEL") ;;
    cerebras) extra=(ANGEL_CEREBRAS_MODEL "$MODEL") ;;
  esac
  save "$var" "$key" ANGEL_DRIVER "$driver" ${extra[@]+"${extra[@]}"}
  saved "$name${MODEL:+, $MODEL}"
}

sign_in() {
  local tool login auth driver name install
  say ""
  say "Sign in with a plan you already pay for:"
  say "  1  ChatGPT (Plus, Pro, Business) through the Codex CLI"
  say "  2  Grok (SuperGrok) through the Grok CLI"
  ask "Plan"
  case "$REPLY" in
    1) tool=codex login="codex login" auth="$codex_auth" driver=openai name="ChatGPT plan" install="npm install -g @openai/codex" ;;
    2) tool=grok login="grok login --oauth" auth="$grok_auth" driver=grok name="Grok plan" install="npm install -g @xai-official/grok" ;;
    *) say "No plan $REPLY."; return 1 ;;
  esac
  if ! command -v "$tool" >/dev/null 2>&1; then
    say "This needs the $tool command. Install it, then run \`angelX setup\` again:"
    say "  $install"
    return 1
  fi
  $login || true
  if [ ! -f "$auth" ]; then
    say "No login found at $auth; nothing saved."
    return 1
  fi
  save ANGEL_DRIVER "$driver"
  saved "$name"
}

server_address() {
  local url base bases key=""
  say ""
  say "The address of an OpenAI-compatible server, for example http://192.168.1.20:8000/v1"
  ask "Address"
  url="${REPLY%/}"
  [ -n "$url" ] || return 1
  case "$url" in http://* | https://*) ;; *) url="http://$url" ;; esac
  have_curl || { say "Checking a server needs curl."; return 1; }
  # OpenAI-compatible servers answer under /v1; try that first unless it was typed.
  case "$url" in */v1) bases="$url" ;; *) bases="$url/v1 $url" ;; esac
  for base in $bases; do
    http_get "$base/models"
    if [ "$HTTP_CODE" = 401 ] || [ "$HTTP_CODE" = 403 ]; then
      ask_secret "The server wants a key (hidden)"
      key="$REPLY"
      http_get "$base/models" "$key"
    fi
    case "$HTTP_CODE" in 2??) url="$base"; break ;; esac
  done
  case "$HTTP_CODE" in
    2??) ;;
    *) say "No model list at $url (HTTP $HTTP_CODE); nothing saved."; return 1 ;;
  esac
  pick_model "$(printf '%s' "$HTTP_BODY" | model_ids)"
  [ -n "$MODEL" ] || { ask "Model name"; MODEL="$REPLY"; }
  if [ -n "$key" ]; then
    save ANGEL_LOCAL_URL "$url" ANGEL_LOCAL_MODEL "$MODEL" ANGEL_BRAIN_KEY "$key" ANGEL_DRIVER local
  else
    save ANGEL_LOCAL_URL "$url" ANGEL_LOCAL_MODEL "$MODEL" ANGEL_DRIVER local
  fi
  saved "$url, $MODEL"
}

skip() {
  [ -f "$settings" ] || save ANGEL_SETUP_SKIPPED 1
  say ""
  say "Skipped. angelX starts in practice mode (no model) until you run \`angelX setup\`."
}

# --- the menu --------------------------------------------------------------
say ""
if [ "$first_run" = 1 ]; then
  say "angelX: no model is connected yet. Let's connect one."
else
  say "angelX setup: connect a model"
fi
say ""
detect_local
detect_env_keys

found=()
labels=()
for i in ${LOCAL_NAMES[@]+"${!LOCAL_NAMES[@]}"}; do
  first=$(printf '%s\n' "${LOCAL_MODELS[$i]}" | head -n 1)
  more=$(printf '%s\n' "${LOCAL_MODELS[$i]}" | grep -c . || true)
  extra=""
  [ "$more" -le 1 ] || extra=" and $((more - 1)) more"
  found+=("local|$i")
  labels+=("${LOCAL_NAMES[$i]} on ${LOCAL_URLS[$i]#http://}: $first$extra")
done
if [ -f "$codex_auth" ]; then found+=("plan|openai|ChatGPT plan"); labels+=("ChatGPT plan, signed in with codex"); fi
if [ -f "$grok_auth" ]; then found+=("plan|grok|Grok plan"); labels+=("Grok plan, signed in with grok"); fi
for seat in ${ENV_SEATS[@]+"${ENV_SEATS[@]}"}; do
  IFS='|' read -r driver name var model <<< "$seat"
  found+=("env|$seat")
  labels+=("$name, key in \$$var")
done

if [ ${#found[@]} -gt 0 ]; then
  say "Found on this machine:"
  for i in "${!labels[@]}"; do printf '  %d  %s\n' "$((i + 1))" "${labels[$i]}"; done
  say ""
  say "Or add one:"
else
  say "No models found on this machine yet."
  say "For a free local model, install Ollama (ollama.com), pull a coding model,"
  say "and run \`angelX setup\` again. Or add one now:"
fi
say "  k  paste an API key (DeepSeek, OpenAI, xAI, Z.ai, Kimi, OpenRouter, Cerebras)"
say "  p  sign in with a plan (ChatGPT or Grok)"
say "  a  a model server at another address"
say "  s  skip for now"
say ""

while :; do
  if [ ${#found[@]} -gt 0 ]; then ask "Choose" 1; else ask "Choose"; fi
  case "$REPLY" in
    k | K) paste_key && break ;;
    p | P) sign_in && break ;;
    a | A) server_address && break ;;
    s | S) skip; break ;;
    *[!0-9]* | "") say "Choose a number or a letter from the list." ;;
    *)
      n="$REPLY"
      if [ "$n" -lt 1 ] || [ "$n" -gt ${#found[@]} ]; then
        say "Choose a number or a letter from the list."
        continue
      fi
      pick="${found[$((n - 1))]}"
      case "$pick" in
        local\|*)
          i="${pick#local|}"
          use_local "${LOCAL_URLS[$i]}" "${LOCAL_MODELS[$i]}" "${LOCAL_NAMES[$i]}"
          ;;
        plan\|*)
          IFS='|' read -r _ driver name <<< "$pick"
          save ANGEL_DRIVER "$driver"
          saved "$name"
          ;;
        env\|*) use_env_seat "${pick#env|}" ;;
      esac
      break
      ;;
  esac
done
[ "$first_run" = 1 ] || { say ""; say "Start angelX inside a project: cd your-project && angelX"; }
