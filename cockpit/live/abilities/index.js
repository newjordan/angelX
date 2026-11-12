const book = {
  definitions: resource('ability-code', 'pack').value.abilities,
  banks: resource('ability-sounds', 'pack').value.banks,
  effects: resource('ability-effects', 'pack').value.banks,
  effect_description: resource('ability-effects', 'pack').value.description
}
const art = resource('characters', 'pack').value
const motionResource = resource.optional('character-motion', 'pack', 'character-animation-pack/1')
const motion = motionResource?.value ?? { options: {} }
const graphResource = resource.optional('character-graphs', 'pack', 'character-animation-graph-pack/1')
const graphOptions = [...Object.keys(graphResource?.value.options ?? {}), 'off'].join('|')
const motionOptions = [...Object.keys(motion.options), 'off'].join('|')
// Delve supports six player seats; ability slots remain four per player.
const MAX_PLAYERS = 6
const reference = (module, option) => ({ module, resource: 'pack', option })
// Bounded exact integers survive JSON/V8/native transport without rounding.
const nextGraphIntent = (state, player) => {
  const previous = state.graphIntents?.[player] ?? 0
  return Number.isSafeInteger(previous) && previous >= 0 && previous < Number.MAX_SAFE_INTEGER ? previous + 1 : null
}
const initial = () => ({ choices: {}, cancelled: {}, soundBanks: {}, effectBanks: {}, artChoices: {}, animationChoices: {} })
module.exports = {
  apiVersion: 2,
  initialize: initial,
  migrate: previous => ({ ...initial(), ...previous.state, artChoices: previous.state.artChoices ?? art.legacy_choices ?? {} }),
  reduce(state, event) {
    if (event.type !== 'command') return { state }
    if (event.name === 'character' && /^graph(?:\s|$)/.test(event.arguments.trim())) {
      const [, option, seat = '1'] = event.arguments.trim().split(/\s+/)
      const player = Number(seat)
      if (!Number.isInteger(player) || player < 1 || player > MAX_PLAYERS || (option !== 'off' && (!Object.hasOwn(graphResource?.value.options ?? {}, option) || !state.artChoices[player] || state.artChoices[player] === 'off'))) return { state, output: `Select a character first; usage: /character graph <${graphOptions}> [player]` }
      const intent = nextGraphIntent(state, player)
      if (intent === null) return { state, output: 'Character selection refused: graph intent revision exhausted or invalid.' }
      return { state: { ...state, graphIntents: { ...(state.graphIntents ?? {}), [player]: intent }, graphChoices: { ...(state.graphChoices ?? {}), [player]: option === 'off' ? null : option }, animationChoices: { ...(state.animationChoices ?? {}), [player]: null } }, output: `graph ${option} selected for player ${player}` }
    }
    if (event.name === 'character' && /^animation(?:\s|$)/.test(event.arguments.trim())) {
      const [, option, seat = '1'] = event.arguments.trim().split(/\s+/)
      const player = Number(seat)
      if (!Number.isInteger(player) || player < 1 || player > MAX_PLAYERS || (option !== 'off' && (!Object.hasOwn(motion.options, option) || !state.artChoices[player] || state.artChoices[player] === 'off'))) return { state, output: `Select a character first; usage: /character animation <${motionOptions}> [player]` }
      const intent = nextGraphIntent(state, player)
      if (intent === null) return { state, output: 'Character selection refused: graph intent revision exhausted or invalid.' }
      return { state: { ...state, graphIntents: { ...(state.graphIntents ?? {}), [player]: intent }, animationChoices: { ...(state.animationChoices ?? {}), [player]: option === 'off' ? null : option }, graphChoices: { ...(state.graphChoices ?? {}), [player]: null } }, output: `animation ${option} selected for player ${player}` }
    }
    if (event.name === 'character') {
      const [option = 'status', seat = '1'] = event.arguments.trim().split(/\s+/).filter(Boolean)
      if (option === 'status') return { state, output: `character choices: ${JSON.stringify(state.artChoices)} · options: ${Object.keys(art.options).join(', ')} · /open live.characters` }
      const player = Number(seat)
      if (!Number.isInteger(player) || player < 1 || player > MAX_PLAYERS || !Object.hasOwn(art.options, option)) return { state, output: `usage: /character <${Object.keys(art.options).join('|')}> [player 1..${MAX_PLAYERS}]` }
      const intent = nextGraphIntent(state, player)
      if (intent === null) return { state, output: 'Character selection refused: graph intent revision exhausted or invalid.' }
      return { state: { ...state, graphIntents: { ...(state.graphIntents ?? {}), [player]: intent }, artChoices: { ...state.artChoices, [player]: option } }, output: `character ${option} selected for player ${player}` }
    }
    if (event.name === 'loadout') return { state, output: `Character loadouts: ${JSON.stringify(state)} · /open live.abilities · /open live.characters` }
    const [name = 'status', seat = '1', key = '1'] = event.arguments.trim().split(/\s+/).filter(Boolean)
    if (name === 'effect') {
      const player = Number(key)
      if (!Number.isInteger(player) || player < 1 || player > MAX_PLAYERS || !Object.hasOwn(book.effects, seat)) return { state, output: `usage: /ability effect <${Object.keys(book.effects).join('|')}> [player 1..${MAX_PLAYERS}]` }
      return { state: { ...state, effectBanks: { ...(state.effectBanks ?? {}), [player]: seat } }, output: `effect bank ${seat} selected for player ${player}` }
    }
    if (name === 'sound') {
      const player = Number(key)
      if (!Number.isInteger(player) || player < 1 || player > MAX_PLAYERS || !Object.hasOwn(book.banks, seat)) return { state, output: `usage: /ability sound <${Object.keys(book.banks).join('|')}> [player 1..${MAX_PLAYERS}]` }
      return { state: { ...state, soundBanks: { ...(state.soundBanks ?? {}), [player]: seat } }, output: `sound bank ${seat} selected for player ${player}` }
    }
    if (name === 'status') return { state, output: `abilities: ${JSON.stringify(state.choices)} · ${Object.keys(book.definitions).join(', ')} · /open live.abilities · /reload status shows active casts` }
    const player = Number(seat), slot = Number(key)
    if (!Number.isInteger(player) || player < 1 || player > MAX_PLAYERS) return { state, output: `player must be 1..${MAX_PLAYERS}` }
    if (name === 'cancel') return { state: { ...state, cancelled: { ...state.cancelled, [player]: (state.cancelled[player] ?? 0) + 1 } }, output: `cancelled module abilities for player ${player}` }
    if (!Number.isInteger(slot) || slot < 1 || slot > 4 || (name !== 'off' && !Object.hasOwn(book.definitions, name))) return { state, output: `usage: /ability <${Object.keys(book.definitions).join('|')}|off> [player 1..${MAX_PLAYERS}] [slot 1..4]` }
    const slots = [...(state.choices[player] ?? [null, null, null, null])]
    slots[slot - 1] = name === 'off' ? null : name
    return { state: { ...state, choices: { ...state.choices, [player]: slots } }, output: `ability ${name} selected for player ${player} slot ${slot}` }
  },
  view(state) {
    return { abilities: { title: 'Executable abilities', lines: [
      ...Object.entries(book.definitions).map(([id, spell]) => `/ability ${id} [player] [slot] — ${spell.name}`),
      '/ability off [player] [slot] restores the ordinary spell.',
      '/ability cancel [player] cancels pending/running module casts.',
      `/ability sound <${Object.keys(book.banks).join('|')}> [player] selects a sound bank.`,
      `/ability effect <${Object.keys(book.effects).join('|')}> [player] selects an effect bank.`,
      book.effect_description,
      ...Object.keys({ ...state.choices, ...(state.soundBanks ?? {}), ...(state.effectBanks ?? {}) }).map(player => `Player ${player}: ${(state.choices[player] ?? [null, null, null, null]).map(id => id ?? 'ordinary').join(', ')} · sounds ${state.soundBanks?.[player] ?? 'crystal'} · effects ${state.effectBanks?.[player] ?? 'crystal'}`),
      'Edit ability-code/fairies.bend for the field, spells.js for other abilities, or an independent art/sound/effect pack. Active casts retain their selected versions.'
    ] }, characters: { title: 'Characters', lines: [
      ...Object.keys(art.options).map(option => `/character ${option} [player 1..${MAX_PLAYERS}]`),
      ...Object.entries(state.artChoices).map(([player, option]) => `Player ${player}: ${option}`),
      ...Object.entries(state.animationChoices ?? {}).filter(([, option]) => option).map(([player, option]) => `Player ${player}: animation ${option}${motionResource ? '' : state.artChoices[player] && state.artChoices[player] !== 'off' ? ' unavailable; showing art defaults until its pack returns' : ' unavailable; choice retained until its pack returns'}`),
      `/character animation <${motionOptions}> [player] selects playback or restores art defaults.`,
      `/character graph <${graphOptions}> [player] selects authored gameplay transitions or art defaults.`,
      ...Object.entries(state.graphChoices ?? {}).filter(([, option]) => option).map(([player, option]) => `Player ${player}: graph ${option}${graphResource ? '' : ' unavailable; choice retained, showing art defaults'}`),
      'Art, animation, executable abilities, sound and effects have independent providers.'
    ] } }
  },
  resources(state) {
    const players = new Set([...Object.keys(state.choices), ...Object.keys(state.cancelled), ...Object.keys(state.soundBanks), ...Object.keys(state.effectBanks), ...Object.keys(state.artChoices)])
    return Object.fromEntries([...players].map(player => [`character-seat-${player}`, {
      player: Number(player),
      slots: (state.choices[player] ?? [null, null, null, null]).map(id => id === null ? null : reference('ability-code', id)),
      appearance: state.artChoices[player] ? reference('characters', state.artChoices[player]) : null,
      animation: state.artChoices[player] && state.artChoices[player] !== 'off' && state.animationChoices?.[player] && motionResource ? reference('character-motion', state.animationChoices[player]) : null,
      graph_intent: state.graphIntents?.[player] ?? 0,
      graph: state.artChoices[player] && state.artChoices[player] !== 'off' && state.graphChoices?.[player] && graphResource ? reference('character-graphs', state.graphChoices[player]) : null,
      sounds: reference('ability-sounds', state.soundBanks[player] ?? 'crystal'),
      effects: reference('ability-effects', state.effectBanks[player] ?? 'crystal'),
      cancel: state.cancelled[player] ?? 0
    }]))
  }
}
