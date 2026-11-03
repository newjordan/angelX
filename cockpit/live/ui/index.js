// Cockpit composition. Save this file while the host is open: geometry,
// visibility, panel order and composer copy change without losing the chat.
module.exports = {
  apiVersion: 1,
  render() {
    return {
      bands: [
        { min_width: 0, transcript_width: 64, side_width: 48, idle_agent_height: 13 },
        { min_width: 140, transcript_width: 80, side_width: 64, idle_agent_height: 13 },
        { min_width: 150, transcript_width: 80, side_width: 64, idle_agent_height: 14 },
        { min_width: 180, transcript_width: 105, side_width: 80, idle_agent_height: 14 },
      ],
      show_agent: true,
      show_artifacts: true,
      agent_first: true,
      reasoning_percent: 80,
      idle_placeholder: 'Write a message…',
      busy_placeholder: 'Add guidance… Enter queues it',
    }
  },
}
