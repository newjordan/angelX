#!/usr/bin/env bash
# Sourced after launcher configuration. By default, configured API providers
# remain available. An operator can restrict them with ANGEL_API_CLUBS.
# Credential files and OAuth stores are never modified here.

_angel_api_clubs_normalized="$(printf '%s' "${ANGEL_API_CLUBS:-}" | tr '[:upper:]' '[:lower:]')"

_angel_api_club_enabled() {
  local wanted="$1" entry
  local -a entries
  [[ "${ANGEL_API_CLUBS+x}" != x ]] && return 0
  IFS=, read -r -a entries <<< "$_angel_api_clubs_normalized"
  for entry in "${entries[@]+"${entries[@]}"}"; do
    entry="${entry//[[:space:]]/}"
    if [[ "$entry" == "$wanted" || "$entry" == "*" || "$entry" == "all" ]]; then
      return 0
    fi
  done
  return 1
}

if ! _angel_api_club_enabled openai; then
  unset ANGEL_OPENAI_KEY OPENAI_API_KEY
fi
if ! _angel_api_club_enabled grok; then
  unset ANGEL_GROK_KEY ANGEL_XAI_KEY GROK_API_KEY XAI_API_KEY GPU_COMP_GROK_KEY
fi
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
if ! _angel_api_club_enabled meta; then
  unset ANGEL_META_KEY META_API_KEY
fi
if ! _angel_api_club_enabled cerebras; then
  unset ANGEL_CEREBRAS_KEY CEREBRAS_API_KEY
fi
if ! _angel_api_club_enabled openrouter; then
  unset ANGEL_OPENROUTER_KEY OPENROUTER_API_KEY
fi
unset -f _angel_api_club_enabled
unset _angel_api_clubs_normalized
