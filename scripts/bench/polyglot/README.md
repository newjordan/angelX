# bench/polyglot

Checkout-only polyglot measurement: runners, tables, and trace analysis.
Nothing here is in the release inventory.

Run the files in this directory. Dated notes under `docs/telemetry/` still cite
the old `scripts/<name>` paths from before this folder existed; those receipts
were not rewritten.

Sibling loaders stay together. `build_polyglot_bench_data.py` and
`analyze-polyglot-full136.py` import `analyze-polyglot-history.py` by filename
beside themselves. Bench tools outside this folder load these files through
`scripts/bench/polyglot/`, not through `scripts/`.
