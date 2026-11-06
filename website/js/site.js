document.querySelectorAll('[data-copy]').forEach((button) => {
  button.addEventListener('click', async () => {
    const text = button.getAttribute('data-copy') || ''
    try {
      await navigator.clipboard.writeText(text)
      const previous = button.textContent
      button.dataset.state = 'copied'
      button.textContent = 'Copied'
      setTimeout(() => {
        button.textContent = previous
        delete button.dataset.state
      }, 1600)
    } catch {
      button.textContent = 'Select the command'
    }
  })
})
