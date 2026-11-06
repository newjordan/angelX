# angelX product website

Static source for [https://omni-space.tech](https://omni-space.tech). Install,
docs, the Glass reel and the repository's published measurements are pages in
this directory. GitHub remains the source and release-download host.

## Preview

```sh
cd website
python3 -m http.server 8080
```

Open `http://127.0.0.1:8080/`. Regenerate HTML after a doc or page edit with
`node website/build.mjs` from the repository root, then
`node website/build.mjs --check`.

## Publish

This directory is deployed on its own as a static site. Merging the repository
does not publish it. The steps, and the separate older hostname that stays as
it is until an approved cutover, are in
[docs/site-rollout.md](../docs/site-rollout.md).
