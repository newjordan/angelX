# Showcase squire activation timing

Run the ignored local measurement from the repository root:

```sh
cargo test --manifest-path cockpit/Cargo.toml -p angelX-cockpit --bin angel app::hotload::tests::measure_showcase_squire_registry_to_open_glass_activation -- --ignored --nocapture
```

The fixture changes the module-owned `showcase-squire/pack.json`, forces the real `angel_live::Registry` to scan, evaluate the module, validate and bind the authored clip, snapshots the published resource, then calls the same App-to-Glass helper used by the live tick. Glass uses its real no-window `Host`; a showcase is open while updates are sent. The test prints nearest-rank p50 and p95 for 51 post-warmup samples, split into `write+Registry.refresh+snapshot`, `App-to-Glass host setter`, and their combined time.

The measurement begins immediately before replacing the pack file and ends when Glass accepts the update into shared host state and queues its wake event. It includes synchronous file write, registry scan and module evaluation, clip preparation/validation, snapshot cloning, App resource selection, host identity comparison/locking, and wake enqueue. It does not include filesystem watcher/debounce delay, editor save time before this write, the remote publisher's later work, showcase picture recomposition, terminal draw, GPU upload, or display presentation. It is a local development diagnostic rather than a CI threshold; machine load and V8 warm state affect the values.
