# Native worn-authoring sample

`odd_helm.png` is a tiny transparent 9×5 item, not a generated doll or a fitted body.
Its asymmetry makes whole-result mirroring easy to inspect. Recreate it from ink
rows with `python samples/native/make_helm.py` from `tools/sprite-rig`.

With a current Rust export and canonical table:

```sh
python -m spriterig.native --native /path/to/native.json --kit realm \
  --worn samples/native/odd_helm.png --slot helm --out out/odd-helm
```

Use the resulting `worn.json` object as the card's `looks.worn` Sprite. Its
`pixel: true` intentionally selects fallback ink art; there is no HD painting.
The canonical table and per-frame native fit, never this sample, own its sizing
and positioning. See ../../README.md for animation strips, slots, and parity tests.
