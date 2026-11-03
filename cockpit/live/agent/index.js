// Operator-owned guidance for the next interactive turn. An active turn keeps
// its original history snapshot. Empty means no additional guidance.
module.exports = {
  apiVersion: 1,
  render() {
    return { turn_note: '' }
  },
}
