// Authoritative Delve rules. These override the host's saved tuning while
// this generation is active, for local and joined knights alike. Removing a
// name restores its normal tuning; a bad edit keeps the previous generation.
// /dungeon tune lists every available name and its allowed range.
module.exports = {
  apiVersion: 1,
  render() {
    return { knobs: {
      // hero_speed: 16, fire_rate: 150, foe_pace: 1.2, foe_hit: 125,
      // hit_ward_s: 0.5, hitstop_heavy: 3,
    } }
  },
}
