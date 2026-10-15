(() => {
  const link = document.querySelector('[data-knight-shift]');
  const dialog = document.getElementById('night-dialog');
  const close = document.getElementById('night-close');
  const video = document.getElementById('night-video');
  if (!link || !dialog || !close || !video || !dialog.showModal) return;

  link.addEventListener('click', (event) => {
    event.preventDefault();
    dialog.showModal();
    close.focus();
  });
  close.addEventListener('click', () => dialog.close());
  dialog.addEventListener('click', (event) => {
    if (event.target === dialog) dialog.close();
  });
  dialog.addEventListener('close', () => {
    video.pause();
    video.currentTime = 0;
  });
})();
