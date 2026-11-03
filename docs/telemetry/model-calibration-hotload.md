# Runtime model calibration

The cockpit reads its embedded measured model defaults and overlays
`~/.angelX/model-calibration.toml`. To update the overlay while the cockpit is
running, write a complete TOML file to a temporary file in the same directory,
then atomically rename it over `model-calibration.toml`. Each request pins one
validated calibration generation across body construction, retries, and identity
recording; subsequent requests can pick up a newly published generation.
Provider selection, explicit effort precedence, and idle timeout rules are
unchanged.

The file must be a regular UTF-8 TOML file no larger than 1 MiB. Invalid files,
files that disappear, and changes to an already accepted file in place retain the
last accepted generation. A rejected file must be replaced by a fresh atomic
rename, not repaired in place. At startup, if no valid runtime file is available,
the embedded defaults are used. To clear all runtime overrides, atomically
publish an empty TOML table (for example, a file containing `date = "2026-01-01"`);
deleting the file retains the last accepted generation until process restart.

The runtime overlay uses the same format as `model-calibration.toml` in this
directory: exact model IDs, a date, and a receipt and reason for each entry.
