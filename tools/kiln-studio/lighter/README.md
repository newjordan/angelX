# Pixel Art Lighter

Vendored copy of [Pixel Art Lighter](https://www.moonfell-rpg.com/) v3.5.22 by Red Hat Sprite (MIT; the notice is in the HTML). It previews a pixel-art albedo under a movable lamp and writes a normal map and a light mask. Developed for Moonfell RPG. The Cloudflare Pages analytics beacon is omitted so a local open does not call out.

Open it from the maker:

```sh
kiln-studio lighter          # http://127.0.0.1:8771/
kiln-studio serve            # review page; the header link is /lighter
```

**Normals.** Its Up direction is positive green, which is the green-up map the game already takes (`docs/WORLD-TIERS.md`). Export `name-Normal.png` and rename it to `<file>_n.png` beside the painting. Do not invert channels: the file's own TODO is for engines that want the other way, and Delve does not. A painting with no map still gets one made at its grain.

Light-mask exports are for the preview. The game has no light-mask slot.
