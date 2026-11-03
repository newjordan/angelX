// Edit these values with AngelX open. The host applies a validated replacement
// on the next UI tick; world state, sessions and work remain native-owned.
module.exports = {
  apiVersion: 1,
  render() {
    return {
      scale: 1,
      notice_ticks: 72,
      sparkle_ticks: 30,
      sleeping: 'a giant sleeps by the keep',
      watching: 'the giant stirs — a knight has fallen silent',
      resurrecting: 'the giant kneels by the fallen knight',
      recovered: 'the knight breathes again — the giant sleeps',
      accounted: 'all knights accounted for — the giant sleeps',
      absent: 'the giant has left the realm',
    }
  },
}
