/* ANGELX — the ascent.
   The page is inverted on the vertical: it opens at the BOTTOM (the entrance)
   and is read upward. Picking up the Axcalibur sends a guiding star from the
   sword tip; scrolling up follows it through the content to the summit. */
(() => {
  'use strict';

  const doc = document.documentElement;
  const body = document.body;
  const entrance = document.getElementById('entrance');
  const pick = document.getElementById('pick');
  const guide = document.getElementById('star-guide');
  const hudPct = document.getElementById('hud-pct');
  const hudFill = document.getElementById('hud-fill');
  const finale = document.getElementById('finale');
  /* the real intro (canvas) when it's live; the wordmark otherwise */
  const swordEl = () => (window.Excalibur && window.Excalibur.visible()
    ? document.getElementById('excalibur') : document.querySelector('.logo-wrap'));
  const returnLink = document.getElementById('return-link');
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  let ascended = false;
  let maxScroll = 1;
  let ticking = false;
  let streak = null;

  const measure = () => {
    maxScroll = Math.max(1, doc.scrollHeight - window.innerHeight);
  };
  const toEntrance = () => window.scrollTo(0, maxScroll);

  /* ── begin at the gate: pin to the bottom of the document ── */
  history.scrollRestoration = 'manual';
  measure();
  toEntrance();
  window.addEventListener('load', () => { measure(); if (!ascended) toEntrance(); });
  window.addEventListener('resize', () => { measure(); if (!ascended) toEntrance(); });
  if (document.fonts && document.fonts.ready) {
    document.fonts.ready.then(() => { measure(); if (!ascended) toEntrance(); });
  }

  /* deep link (#points etc.) skips the ritual and ascends directly */
  if (location.hash) {
    const target = document.querySelector(location.hash);
    if (target) {
      ascended = true;
      body.classList.add('ascended');
      entrance.classList.add('lifted');
      if (window.Excalibur && window.Excalibur.stop) window.Excalibur.stop();
      window.scrollTo(0, target.offsetTop);
      update();
    }
  }

  /* reveal blocks as they enter the viewport (arriving from above) */
  /* a block taller than the viewport (the stacked bench plots) can never show
     15% of itself at once, so it reveals as soon as any of it is on screen */
  const io = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      const tall = e.boundingClientRect.height > window.innerHeight * 0.6;
      if (tall || e.intersectionRatio >= 0.15) e.target.classList.add('in-view');
    }
  }, { threshold: [0, 0.15] });
  document.querySelectorAll('.reveal').forEach((el) => io.observe(el));

  /* ── a tiny star ping breathes at the sword tip while the blade is raised ── */
  let ping = null, pingTimer = 0;
  function startPing() {
    if (reduced || ping) return;
    const el = swordEl();
    if (!el) return;
    const make = () => {
      if (ping) return;
      const el = swordEl();
      if (!el || !window.Excalibur || !window.Excalibur.visible()) return;
      const { x, y } = window.Excalibur.tip(el);
      ping = document.createElement('div');
      ping.className = 'tip-ping';
      ping.setAttribute('aria-hidden', 'true');
      ping.innerHTML = '<svg viewBox="0 0 46 46"><use href="#star4"/></svg>';
      ping.style.left = x + 'px';
      ping.style.top = y + 'px';
      document.body.appendChild(ping);
      ping.addEventListener('animationend', () => { ping && ping.remove(); ping = null; });
    };
    make();
    pingTimer = setInterval(make, 3400);
  }
  function stopPing() {
    clearInterval(pingTimer); pingTimer = 0;
    if (ping) { ping.remove(); ping = null; }
  }

  /* ── the guiding star rises from behind the sword tip ── */
  function spawnStar() {
    stopPing();
    if (reduced) return;
    const el = swordEl();
    if (!el) return;
    const { x, y } = window.Excalibur && el.id === 'excalibur'
      ? window.Excalibur.tip(el)
      : (() => { const r = el.getBoundingClientRect(); return { x: r.left + r.width * 0.5, y: r.top + r.height * 0.08 }; })();
    const s = document.createElement('div');
    s.className = 'rising-star';
    s.setAttribute('aria-hidden', 'true');
    s.innerHTML = '<svg viewBox="0 0 46 46"><use href="#star4"/></svg>';
    s.style.left = x + 'px';
    s.style.top = y + 'px';
    document.body.appendChild(s);
    requestAnimationFrame(() => requestAnimationFrame(() => s.classList.add('rise')));
    s.addEventListener('animationend', () => s.remove());
  }

  function ascend(opts = {}) {
    if (ascended) return;
    ascended = true;
    body.classList.add('ascended');
    entrance.classList.add('lifted');
    if (window.Excalibur && window.Excalibur.stop) window.Excalibur.stop();
    spawnStar();
    update();
    if (!opts.manual) {
      /* glide one viewport up, into the intro */
      const target = Math.max(0, entrance.offsetTop - window.innerHeight);
      setTimeout(() => {
        window.scrollTo({ top: target, behavior: reduced ? 'auto' : 'smooth' });
      }, reduced ? 0 : 650);
    }
  }

  entrance.addEventListener('click', () => ascend());
  pick.addEventListener('click', (e) => { e.stopPropagation(); ascend(); });

  /* scrolling UP past the gate by hand also begins the ascent —
     direction-aware so reset()'s downward glide doesn't re-trigger it */
  let lastY = window.scrollY;
  window.addEventListener('scroll', () => {
    const y = window.scrollY;
    const goingUp = lastY - y > 4;
    const goingDown = y - lastY > 4;
    lastY = y;
    if (!ascended && goingUp && y < maxScroll - 40) ascend({ manual: true });
    /* scrolling back DOWN to the gate re-arms the ritual: the mark and the
       blade light up again and the star waits to be picked up once more */
    if (ascended && goingDown && y >= maxScroll - window.innerHeight * 0.4) rearm();
    if (!ticking) { ticking = true; requestAnimationFrame(update); }
  }, { passive: true });

  function update() {
    ticking = false;
    if (!ascended) return;
    const p = Math.min(1, Math.max(0, 1 - window.scrollY / maxScroll));
    // the light streak: a scroll-linked beam the star rides up the page
    if (!streak) {
      streak = document.createElement('div');
      streak.className = 'light-streak';
      streak.setAttribute('aria-hidden', 'true');
      document.body.appendChild(streak);
    }
    streak.style.setProperty('--p', p.toFixed(4));
    // the beam is the climb itself: it burns out as the summit arrives, so the
    // very top of the page has no trail left on it
    const bu = Math.min(1, Math.max(0, (p - 0.94) / 0.05));
    streak.style.opacity = (1 - bu * bu * (3 - 2 * bu)).toFixed(3);
    // the guide star sits just ahead of the beam's head — leading the ascent
    const streakHeadY = window.innerHeight * (0.94 - 0.88 * p);
    guide.style.top = Math.max(10, streakHeadY - 34) + 'px';
    hudFill.style.width = (p * 100).toFixed(1) + '%';
    hudPct.textContent = String(Math.round(p * 100)).padStart(3, '0') + '%';
    const crowned = p > 0.985;
    // crowned, the star stays — alone now, resting at the very top above the
    // summit — instead of dying with the beam
    guide.style.opacity = crowned ? '1' : String((0.35 + 0.65 * p).toFixed(3));
    finale.classList.toggle('crowned', crowned);
    if (window.Mountain) {
      window.Mountain.progress(p);   // the chart climbs with you
      if (crowned) window.Mountain.walk(); // the summit: the wizard walks in
    }
  }

  /* ── return to the gate and re-arm the ritual ── */
  function rearm() {
    ascended = false;
    if (streak) { streak.remove(); streak = null; }
    body.classList.remove('ascended');
    entrance.classList.remove('lifted');
    finale.classList.remove('crowned');
    if (window.Mountain) window.Mountain.reset();
    guide.style.opacity = '0';
    document.querySelectorAll('.in-view').forEach((el) => el.classList.remove('in-view'));
    startPing();
  }
  /* the button: re-arm, then glide home to the gate */
  function reset() {
    rearm();
    window.scrollTo({ top: maxScroll, behavior: reduced ? 'auto' : 'smooth' });
  }
  /* the ping begins once the blade is raised and settled (~4.5s of intro) */
  setTimeout(() => { if (!ascended) startPing(); }, 4500);
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) stopPing();
    else if (!ascended && window.Excalibur && window.Excalibur.visible()) startPing();
  });

  /* ── the install command: one click, on the clipboard ────────────────
     The summit's copy button reads the command out of #install-cmd. The fixed
     bottom-left box is a plain GitHub link, so it needs no script. */
  const copyTargets = Array.from(document.querySelectorAll('[data-copy]'));
  const writeClipboard = async (text) => {
    if (!text) return false;
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (err) {
      /* non-secure contexts and older browsers: fall back to a scratch node */
    }
    const scratch = document.createElement('textarea');
    scratch.value = text;
    scratch.setAttribute('readonly', '');
    scratch.style.position = 'fixed';
    scratch.style.top = '-1000px';
    document.body.appendChild(scratch);
    scratch.select();
    let ok = false;
    try { ok = document.execCommand('copy'); } catch (e) { ok = false; }
    scratch.remove();
    return ok;
  };
  for (const el of copyTargets) {
    el.addEventListener('click', async () => {
      const source = el.dataset.copy ? document.getElementById(el.dataset.copy) : null;
      const text = (source || el.querySelector('code') || el).textContent.trim();
      const ok = await writeClipboard(text);
      el.classList.toggle('copied', ok);
      el.setAttribute('aria-label', ok ? 'Install command copied' : 'Copy the install command');
      clearTimeout(el.copiedTimer);
      el.copiedTimer = setTimeout(() => {
        el.classList.remove('copied');
        el.setAttribute('aria-label', 'Copy the one-command Linux install');
      }, 1800);
    });
  }

  returnLink.addEventListener('click', reset);
})();
