# Realm avatars

Every figure here was chosen by the model itself. Each model was asked,
through its own angelX route (`angel --ask`), how it wants to be drawn in this
world: a dark-fantasy realm at dusk whose heart is a Round Table, with a
mission to spread goodness, light and knowledge, where strange and
otherworldly natures are welcome as friends. They chose on six visual scales
(smooth–spiky, soft–blocky, calm–aggressive, light–heavy, plain–ornate,
muted–vivid), one model at a time, each seeing the figures already chosen.

- `round-table/`: one seat per model family, a knight. These are the agent
  portraits. Six models chose a knight outright; the other nine drew the
  knight that carries their chosen figure to the table.
- `pools/<place>/`: the realm's inhabitants, every other figure the models
  drew, each living at the place whose real activity it reflects:
  - `scriptorium`: librarians and archivists (study)
  - `rookery`: chroniclers and scribes (chronicle)
  - `observatory`: star-readers and cartographers (research)
  - `smithy`: artificers and weavers (forge)
  - `chapel`: keepers and augurs (memory)
  - `gatehouse`: wayfarers and couriers (dispatch)
  - `keep`: wardens and sentinels (errands)
- `roster.json`: every seat and inhabitant with its model, its own words
  (figure, title, look, why), where it came from, and file hashes.

Each figure has a `render/` (Qwen-Image 2.1, transparent, 512px) and a
`sprite/` (96px, 24 colours, hard alpha, drawn at 2× in a 192px frame). The
darker twins from the first poll are in `../doppelgangers/`. Nothing in the
cockpit reads this directory yet: the Round Table seats become eight-pose
helm sheets first, and the pools become inhabitants of the overworld.

Qwen-Image 2.1 ran locally in ComfyUI. Qwen is licensed under the Qwen RESEARCH
LICENSE AGREEMENT, Copyright (c) 2026 Hangzhou Tongyi Laboratory Technology
Co., Ltd. All Rights Reserved.
