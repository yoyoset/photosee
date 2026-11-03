# M0-S: SMB benchmark tool (progate)

## Why
The plan promises the following on a gigabit LAN NAS with a folder of 2000
photos:
- first image ≤600ms;
- listing ≤2s;
- an IO concurrency limit of 4.

The concurrency limit is a guess. This tool measures the real numbers on the
user's NAS so the server's queue settings are based on data.

## Owned paths
- `tools/bench-smb/**`: a standalone Rust binary. Its own `Cargo.toml` has an
  empty `[workspace]` table, it has its own `Cargo.lock`, and it uses the
  **standard library only**.
- `scripts/bench-smb.ps1`: a wrapper that builds it in release mode and runs
  it with the arguments it was given.

## Behaviour
`bench-smb <DIR> [--runs N]` (default N = 3) prints a Markdown report to
stdout. Each phase reports min, median and max over N runs:

1. **List**: `std::fs::read_dir(DIR)` plus `DirEntry::metadata()` for every
   entry. Count files and total time. Run 1 is "cold" (best effort), runs 2
   and later are "warm".
2. **Head reads**: for the first 50 `.jpg`/`.jpeg` files in name order, read
   the first 64 KB of each. Report per-file latency median and p90.
3. **First image**: the full read time of the first JPEG, and its size.
4. **Concurrency sweep**: read 40 whole JPEGs with K threads, K ∈ {1, 2, 4,
   8, 16}, using `std::thread::scope` and a shared atomic work index. Use a
   different file set per K so the OS cache doesn't fake results. If fewer
   than 200 JPEGs exist, reuse files and print a warning. Report total
   seconds and MB/s per K, and mark the smallest K that reaches ≥90% of the
   best MB/s.
5. **Header line**: the start of the report records the date, the DIR (UNC or
   drive), the file count, and the OS.

Errors (a missing path, or access denied on an individual file) are printed
in the report and never panic. The tool only reads; it never writes into DIR.

## Acceptance
- `cargo build --release` in `tools/bench-smb` succeeds with zero
  dependencies.
- Unit tests for the statistics helpers (median, p90 on even and odd counts,
  empty input) and the "smallest K reaching 90%" picker. Run them with
  `cargo test` in that folder.
- A smoke run against a local temp folder of generated files succeeds. The
  test creates them itself.
- Files must be ≤300 lines each and functions ≤50 lines.
- `node scripts/gates.mjs --ownership M0-S main` passes.
- The orchestrator runs it on the user's NAS afterwards; you cannot.
