'use strict';
// The page's windows: small builders for the elements boot.js lays over
// the picture, and the sheet (a game's own state, as plain text) set as a
// framed screen. Nothing here reads the game: it is handed the text.

/* exported el, keyHint, drawSheet */

// An element of `tag` with `cls` and, when given, its text.
function el(tag, cls, words) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (words !== undefined && words !== null) e.textContent = words;
  return e;
}

// "[Enter] what it does": a key cap and its words.
function keyHint(key, words) {
  const s = el('span', 'hint');
  s.append(el('kbd', '', key), document.createTextNode(' ' + words));
  return s;
}

// A count out of a few (a wish's ladder, 0/3) as pips; out of many (a
// gauge, 40/100) as a bar. `null` for anything else.
function tally(cell) {
  const m = /^(\d+)\/(\d+)$/.exec(cell);
  if (!m) return null;
  const n = Number(m[1]), of = Number(m[2]);
  if (of <= 0) return null;
  if (of <= 8) {
    const pips = el('span', 'pips');
    pips.title = cell;
    for (let i = 0; i < of; i++) pips.append(el('i', i < n ? 'lit' : ''));
    return pips;
  }
  return bar(n, of, 'gauge');
}

// A meter: the frame, the fill as far as `n` of `of`, the count beside it.
function bar(n, of, cls) {
  const wrap = el('span', 'meter ' + cls);
  const track = el('span', 'track');
  const fill = el('i');
  fill.style.width = `${Math.max(0, Math.min(100, (100 * n) / of))}%`;
  track.append(fill);
  wrap.append(track, el('span', 'count', `${n}/${of}`));
  return wrap;
}

// One cell of a sheet's row: a tally, a locked step, a next step, or words.
function cell(words) {
  const t = tally(words);
  if (t) return t;
  if (words.startsWith('LOCKED: ')) {
    const c = el('span', 'cell locked');
    c.append(el('b', '', 'Locked'), document.createTextNode(' ' + words.slice(8)));
    return c;
  }
  if (words.startsWith('next: ')) {
    const c = el('span', 'cell');
    c.append(el('b', '', 'Next'), document.createTextNode(' ' + words.slice(6)));
    return c;
  }
  return el('span', 'cell', words);
}

// Sections that are long ledgers go in the sheet's second column.
const LEDGERS = new Set(['TEACHERS']);

// The sheet's text as a screen. Its shape (crates/delve/src/client.rs,
// `sheet`): a first line "name   health n/m", a few lines about the knight,
// then sections apart by blank lines, each headed by a line in capitals
// (its title, then notes after two spaces) over rows indented by two,
// whose cells are apart by two spaces or more. A line of " · " parts is a
// list of totals. Any other text is set as it comes, line by line.
function drawSheet(page, text) {
  // Where the page and each column were scrolled, kept over the redraw.
  const keep = [page, ...page.querySelectorAll('.column')].map(e => e.scrollTop);
  page.replaceChildren();
  const lines = (text || '').split('\n');
  if (!text) {
    page.append(el('p', 'note', 'Nothing to show yet.'));
    return;
  }
  const left = el('div', 'column'), right = el('div', 'column');
  // The head: name and health, then the lines about the knight.
  let i = 0;
  const first = /^(.*?)\s{2,}health (\d+)\/(\d+)$/.exec(lines[0] || '');
  const head = el('header', 'who');
  if (first) {
    head.append(el('h2', '', first[1]), bar(Number(first[2]), Number(first[3]), 'health'));
    i = 1;
  }
  const about = el('div', 'about');
  for (; i < lines.length && lines[i].trim(); i++) {
    const parts = lines[i].split(/\s{2,}/);
    const row = el('div', 'line');
    if (parts.length > 1) row.append(el('b', '', parts[0]), el('span', '', parts.slice(1).join('  ')));
    else row.append(el('span', '', parts[0]));
    about.append(row);
  }
  head.append(about);
  left.append(head);
  // The sections.
  let section = null;
  for (; i < lines.length; i++) {
    const line = lines[i];
    if (!line.trim()) {
      section = null;
      continue;
    }
    if (!line.startsWith(' ')) {
      const parts = line.split(/\s{2,}/);
      const title = parts[0];
      section = el('section');
      const h = el('h3');
      h.append(el('span', 'title', title));
      section.append(h);
      const notes = parts.slice(1);
      for (const note of notes) {
        if (note.includes(' · ')) {
          const list = el('div', 'totals');
          for (const part of note.split(' · ')) list.append(el('span', 'total', part));
          section.append(list);
        } else {
          h.append(tally(note) || el('span', 'note', note));
        }
      }
      section.dataset.title = title;
      (LEDGERS.has(title) ? right : left).append(section);
      continue;
    }
    if (!section) {
      section = el('section');
      left.append(section);
    }
    const cells = line.trim().split(/\s{2,}/);
    // A line of its own words (a note, "none yet: ..."), unless it is a
    // name alone in a ledger (a held card that adds nothing).
    const named = cells.length === 1 && !cells[0].includes(':') && /^[A-Z0-9]/.test(cells[0]);
    if (cells.length === 1 && !named) {
      section.append(el('p', 'note', cells[0]));
      continue;
    }
    // A ledger of rows: the first cell names the row.
    let rows = section.querySelector('.rows');
    if (!rows) {
      rows = el('div', 'rows');
      section.append(rows);
    }
    rows.dataset.cols = Math.max(Number(rows.dataset.cols || 0), cells.length);
    const row = el('div', 'row');
    row.append(el('span', 'name', cells[0]));
    for (const c of cells.slice(1)) row.append(cell(c));
    rows.append(row);
  }
  // The wish ledger (the longest WISHES section) goes beside the knight
  // too, where there is room for two columns.
  const wishes = [...left.querySelectorAll('section')].filter(s => s.dataset.title === 'WISHES');
  const ledger = wishes.reduce((a, b) => (b.querySelectorAll('.row').length > (a ? a.querySelectorAll('.row').length : 0) ? b : a), null);
  if (ledger && ledger.querySelectorAll('.row').length > 4) right.prepend(ledger);
  page.append(left, right);
  for (const rows of [...left.querySelectorAll('.rows'), ...right.querySelectorAll('.rows')]) {
    rows.classList.add('cols-' + Math.min(4, Number(rows.dataset.cols)));
  }
  [page, left, right].forEach((e, i) => { e.scrollTop = keep[i] || 0; });
}
