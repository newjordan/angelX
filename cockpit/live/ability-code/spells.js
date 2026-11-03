// Add an executable ability here without another Rust spell variant. The game
// port accepts projectiles, healing, movement and area statuses. Instances
// own their timeline and retain this file's captured generation until done.
const fairy = require('./fairies.compiled')
const { $: skillTag, parameters: { $: parametersTag, ...fairyParameters }, ...fairyDefinition } = fairy.definition()
exports.definitions = {
  fairies: { ...fairyDefinition, parameters: fairyParameters },
  spark: { name: 'Twin Spark', mana: 8, cooldown: 24, icon: '...P.../..PPP../.PPPPP./...P.../..P....', parameters: { damage: 28, speed: 20, delay: 5 } },
  bloom: { name: 'Bloom', mana: 12, cooldown: 60, icon: '..6.6../...6.../.66666./...6.../..6.6..', parameters: { heal: 6 } }
}
exports.tick = (state, event) => {
  const casts = { ...state.casts }, actions = [], finished = []
  for (const id of event.cancelled ?? []) delete casts[id]
  for (const start of event.starts) casts[start.cast] = { ...start, began: event.tick }
  for (const [id, cast] of Object.entries(casts)) {
    const player = event.players[cast.player]
    if (!player) { finished.push(id); delete casts[id]; continue }
    const age = event.tick - cast.began
    const emit = command => actions.push({ cast: id, command })
    if (cast.ability === 'fairies') {
      const step = fairy.tick(cast.field ?? { $: 'Fresh' }, {
        $: 'Tick', age, x: player.position[0], y: player.position[1], location: JSON.stringify(event.location)
      }, { $: 'Params', ...cast.parameters })
      if (step.$ === 'End') { finished.push(id); delete casts[id]; continue }
      if (step.$ !== 'Keep') throw Error('invalid Bend field transition')
      cast.field = step.field
      if (step.opened) {
        emit({ type: 'sound', cue: 'fairies' })
        emit({ type: 'visual', cue: 'fairies' })
      }
      if (step.area.$ === 'Some') {
        const area = step.area.value
        emit({ type: 'status-area', center: [area.x, area.y], radius: area.radius, status: { type: 'stun', ticks: area.ticks } })
      }
    } else if (cast.ability === 'spark') {
      if (age === 0 || age >= cast.parameters.delay) {
        emit({ type: 'sound', cue: 'spark' })
        emit({ type: 'visual', cue: 'spark' })
        let [x, y] = player.aim
        const length = Math.hypot(x, y) || 1
        x /= length; y /= length
        emit({ type: 'projectile', offset: [x * 0.8, y * 0.8], velocity: [x * cast.parameters.speed, y * cast.parameters.speed], damage: cast.parameters.damage, ttl: 90, kind: 'bolt' })
      }
      if (age >= cast.parameters.delay) { finished.push(id); delete casts[id]; continue }
    } else if (cast.ability === 'bloom') {
      emit({ type: 'sound', cue: 'bloom' })
      emit({ type: 'visual', cue: 'bloom', follow: true })
      emit({ type: 'heal', amount: cast.parameters.heal })
      finished.push(id); delete casts[id]; continue
    } else throw Error(`unknown executable ability: ${cast.ability}`)
    casts[id] = cast
  }
  return { state: { casts }, output: { actions, finished } }
}
