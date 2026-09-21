# angelX website

Black & white, **dotmax / retro-fantasy / high-tech** fusion. The mission: spread
the light of knowledge through research tools.

## The inverted vertical

The page opens at the **bottom** of the document — the entrance — and is read
**upward**. DOM order is therefore the reverse of reading order:

```
document top    #finale                     ← end of the journey
                #charts   04 · telemetry
                #gallery  03 · sightings
                #points   02 · twelve tenets
                #intro    01 · what it is
document bottom #entrance the gate             ← the beginning
```

On load, JS pins the scroll to the bottom (`history.scrollRestoration =
'manual'` + `scrollTo`). Do not reorder the sections without flipping this.

## The ritual

1. Silver/chrome **AngelX** logo renders; the **real Excalibur intro** — the same
   asset the app plays (`cockpit/assets/excalibur/rise.png`, a 60-frame atlas,
   copied to `assets/excalibur/`) — rises above the letters, re-dithered to
   dotmax dots in `js/excalibur.js` at the app's own cadence: 12 fps rise, hold
   at frame 49, then ambient water looping frames 50–59 with the 4-tick
   crossfade seam. A hand-drawn fallback sword shows only if the atlas fails.
   `...pick it up` appears bottom-right in a DOS font (VT323).
2. Click (or scroll up manually): a **guiding star** rises from behind the sword
   tip, the gate seals, and the page glides one viewport up into the content.
3. Scrolling up follows the star: intro → 12 tenets → images → charts → summit.
   The HUD (bottom-left) tracks ASCENT %; at the top the summit star ignites.
4. `▼ RETURN TO THE ENTRANCE` scrolls back down and re-arms the ritual.

## Run it

No build step — static files.

```bash
cd website
python3 -m http.server 8080
# → http://localhost:8080
```

## Structure

```
index.html            all sections + inline SVG art (shared <defs> at top of <body>)
css/style.css         design tokens + all animation/state
js/main.js            inverted scroll, ascend ritual, HUD, reveals
js/excalibur.js       the real startup intro: rise.png atlas → dotmax canvas
assets/excalibur/rise.png   10×6 atlas of 60 frames (copied from the cockpit)
assets/favicon.svg
```

## Filler → real content

- **Images** — `#gallery` figures are inline SVG plates labeled
  `IMAGE PLACEHOLDER`. Swap each `<svg class="art">` for `<img>`/real art.
- **Charts** — `#charts` are hand-drawn placeholder SVGs (`PLACEHOLDER DATA`).
- **Stats** — the intro spec plate rows (`N/A · PLACEHOLDER`).
- **Copy** — tenets in `#points`, paragraphs in `#intro`.

## Design tokens

- Palette: `#050506` bg · `#f4f4f7` ink · grays only (no color anywhere).
- Type: `Cinzel` (retro-fantasy display) · `VT323` (DOS) · system mono (body).
- Texture: fixed dot-field, scanlines, vignette; dot-matrix patterns in SVG.
- `prefers-reduced-motion` collapses animations but keeps the flow.
