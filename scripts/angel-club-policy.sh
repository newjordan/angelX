#!/usr/bin/env bash
# Sourced after launcher configuration. Credentials remain untouched on disk;
# disabled provider credentials do not enter this cockpit (including an older
# binary launched with ANGEL_NO_BUILD=1). OAuth tokens live in their own stores.

_angel_api_clubs_normalized="$(printf '%s' "${ANGEL_API_CLUBS:-}" | tr '[:upper:]' '[:lower:]')"

_angel_api_club_enabled() {
  local wanted="$1" entry
  local -a entries
  IFS=, read -r -a entries <<< "$_angel_api_clubs_normalized"
  for entry in "${entries[@]+"${entries[@]}"}"; do
    entry="${entry//[[:space:]]/}"
    if [[ "$entry" == "$wanted" ]]; then
      return 0
    fi
  done
  return 1
}

# Removed routes, never alternatives to the operator's OAuth selection.
unset ANGEL_META_KEY META_API_KEY ANGEL_META_URL META_API_URL \
  ANGEL_META_MODEL META_MODEL ANGEL_LUNA_KEY LUNA_API_KEY CHATGPT_LUNA_KEY \
  OPENAI_API_KEY ANGEL_LUNA_URL LUNA_API_URL CHATGPT_LUNA_URL \
  ANGEL_LUNA_MODEL LUNA_MODEL CHATGPT_LUNA_MODEL

if ! _angel_api_club_enabled kimi; then
  unset ANGEL_KIMI_KEY KIMI_API_KEY MOONSHOT_API_KEY
fi
if ! _angel_api_club_enabled deepseek; then
  unset ANGEL_DEEPSEEK_KEY DEEPSEEK_API_KEY
fi
if ! _angel_api_club_enabled glm; then
  unset ANGEL_GLM_KEY GLM_API_KEY ZAI_API_KEY ZHIPU_API_KEY BIGMODEL_API_KEY
fi
if ! _angel_api_club_enabled qwen; then
  unset ANGEL_QWEN_KEY QWEN_API_KEY DASHSCOPE_API_KEY
fi
if ! _angel_api_club_enabled longcat; then
  unset ANGEL_LONGCAT_KEY LONGCAT_API_KEY
fi
if ! _angel_api_club_enabled cerebras; then
  unset ANGEL_CEREBRAS_KEY CEREBRAS_API_KEY
fi
if ! _angel_api_club_enabled openrouter; then
  unset ANGEL_OPENROUTER_KEY OPENROUTER_API_KEY
fi
unset -f _angel_api_club_enabled
unset _angel_api_clubs_normalized
