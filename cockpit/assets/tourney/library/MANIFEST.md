# Tourney Theatrical — asset library for angel0 miniviz

**Purpose:** Mini sequences and cut-in elements that stage a knight’s tournament
theatrically in angel0 miniviz / tourney ceremony windows.

**Style contract:** Stylized theatrical medieval · bold silhouettes · festival gold/crimson · no text on props · sequences at 12 fps.

**Two deliveries:**

| Tree | Role |
|------|------|
| `assets/tourney-theatrical/` (this pack) | Full-color sources + sequenced frames + Angel-palette miniviz derivatives |
| `angel0/cockpit/assets/tourney/library/` | Cockpit-installed subset for miniviz use |
| `angel0/cockpit/assets/tourney/plates/` | 480×240 Angel-DMD plates |
| `angel0/cockpit/assets/tourney/poses/` | Legacy poses + new hero/challenger/maiden/herald |

---

## Locations (stills)

| File | Beat |
|------|------|
| `locations/loc_arena_empty_dawn.jpg` | Empty lists at dawn — curtain up |
| `locations/loc_arena_dmd_plate.jpg` | Flat Angel-palette arena plate source |
| `locations/loc_castle_gate_festival.jpg` | Arrival at the festival gate |
| `locations/loc_royal_pavilion.jpg` | Royal tent / patronage |
| `locations/loc_grandstands_cheer.jpg` | Crowd energy |
| `locations/loc_lists_dusk.jpg` | Aftermath / quiet field |
| `locations/scene_victory_lap.jpg` | Victory with petals |

**Miniviz plates (480×240, 8-color Angel palette):** `miniviz/plates/*.png`

---

## Characters

| File | Beat |
|------|------|
| `characters/char_knight_hero_charge.jpg` | Silver hero (magenta keyable) |
| `characters/char_knight_challenger.jpg` | Dark challenger (magenta keyable) |
| `characters/char_maiden_favor.jpg` | Happy maiden + silk favor |
| `characters/char_maidens_cheer.jpg` | Group of cheering maidens |
| `characters/char_noble_lady_box.jpg` | Lady in the box bestowing favor |
| `characters/char_herald_trumpet.jpg` | Herald / fanfare |

**Miniviz poses (256×256, keyed + quantized):** `miniviz/poses/*.png`

---

## Props & FX

| File | Beat |
|------|------|
| `props/prop_flag_raise.jpg` | Flag climbing the pole |
| `props/prop_pennants_set.jpg` | Three pennants (magenta keyable) |
| `fx/fx_lance_shield_impact.jpg` | Lance → shield impact wide |
| `fx/fx_shield_crack_close.jpg` | Shield close-up after hit |

---

## Mini sequences (play in order)

All under `sequences/<name>/` with `name_00.png…`, `name_sheet.png`, `name_loop_meta.json`, `index.html`.

| Sequence | Frames | FPS | Mode | Notes |
|----------|--------|-----|------|-------|
| `flag_raise` | 14 | 12 | scene loop (`--no-key`) | Ceremony open |
| `lance_impact` | 12 | 12 | **one-shot** mid clip | Close-up hit — play once, don’t infinite-loop |
| `maiden_wave` | 16 | 12 | scene loop | Favor / crowd cut-in |
| `knight_charge` | 10 | 12 | keyed sprite loop | Magenta-keyed gallop |

Also see prior pack: `assets/joust/` (12-frame gallop sheet) for extra charge material.

### Playback tips for miniviz

1. **Locations** as background plates (swap by ceremony phase).
2. **Sequences** as short cut-ins (1–2s) over the plate or full-frame.
3. **Poses** overlay like existing `poses/idle.png` … `victory.png`.
4. **lance_impact** is theatrical punctuation — trigger on impact frame of joust, play forward once.
5. HTML previews: `python3 -m http.server 8765 -d sequences/<name>` → `http://127.0.0.1:8765/`

### Suggested theatrical arc (miniviz storyboard)

```
1. castle_gate_festival     → arrival
2. flag_raise sequence      → open the lists
3. arena_empty_dawn plate   → field set
4. herald_trumpet still     → fanfare beat
5. knight_charge sequence   → challenge
6. lance_impact sequence    → strike (once)
7. maidens_cheer / maiden_wave → reaction
8. victory_lap plate        → crown the loop
9. lists_dusk               → clear the field
```

Map onto Angel ceremony kinds (`lifecycle_viz`):

| Ceremony | Suggested assets |
|----------|------------------|
| LoopStart / GoalSet | flag_raise + castle_gate |
| LoopEscalate / joust | knight_charge + arena plate |
| impact moment | lance_impact (once) |
| LoopDone / win | victory_lap + maiden_wave |
| LoopFailed | shield_crack + lists_dusk |
| LoopStopped | lists_dusk + retreat pose |

---

## Angel palette (miniviz)

Production plates/poses quantized to:

`#000000` · `#56E8FF` · `#5BBEFF` · `#3A6F97` · `#B8E4FF` · `#FFCF5C` · `#FF677E` · `#63F1A9`

Same family as `scripts/process-tourney-assets.py`.

---

## Honest limits

- **Generation volume:** 17 theatrical stills + 4 video sequences; not every costume/angle variant.
- **lance_impact** wrap ratio ~1.4 if forced-looped — treat as one-shot.
- **Scene loops** use `--no-key` (full frames); sprite charge uses magenta key.
- `joust_gallop`/`knight_charge` shipped with residual near-key magenta spill;
  cleaned in-place 2026-07-20 (alpha zeroed within channel-distance 60 of the
  key). Re-exports from staging need the same pass or a wider key tolerance.
- **Wired** into `lifecycle_viz.rs` (2026-07-20): all five sequences play —
  `flag_raise` loops through challenge ceremonies, `knight_charge` opens the
  grand joust, `joust_gallop` owns its approach, `lance_impact` plays once at
  the strike and holds its aftermath, `maiden_wave` crowns victories. Location
  plates back each ceremony (castle_gate / arena_dmd / victory_lap /
  lists_dusk) and the `herald` still opens non-joust entry beats. Retreat
  action stays procedural over the dusk field.
- **Wired** into the ordinary cockpit rider overlay (2026-07-20):
  `knight_charge` and `joust_gallop` join the production approach/canter pair
  in a slow realm-seeded rotation. This overlay deliberately caps playback at
  roughly 3.4 fps instead of the source preview rate to avoid terminal flicker.
- Still unwired: `hero_charge`/`challenger`/`maiden_favor`/`noble_lady` stills
  (candidates for world billboards and interiors, not ceremony beats), the fx
  stills, and the remaining location plates.

---

## Tooling

```bash
# regenerate a sequence loop
bin/sprite-export loop sequences/flag_raise/raw -o sequences/flag_raise \
  --name flag_raise --no-key --fps 12

# MCP (angel0)
sprite__loop / sprite__preview / sprite__key
skill(sprite-export)
```
