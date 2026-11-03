module.exports = {
  apiVersion: 1,
  render({ native_help }) {
    // Keep the authoritative native verb list, while making its surrounding
    // presentation editable independently of the command dispatcher.
    return native_help
  },
}
