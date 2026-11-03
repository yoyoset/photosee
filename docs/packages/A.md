# A: scan, natural sort, folder navigation (core)

Read `AGENTS.md` first. Owned paths: see `[A]` in `docs/ownership.toml`.

## Why
Listing and "next folder" run on every folder open and at every slideshow
end. On a NAS a folder of 2000 files must list in ≤2 s, so this code does no
per-file IO beyond what `FsBackend::read_dir` already returned.

## Implement
1. `core/src/sort.rs` `natural_cmp`: use `alphanumeric_sort::compare_str` on
   the case-folded names first, then exact `str::cmp` as the tiebreak, so the
   order is total (`"a.jpg"` and `"A.jpg"` never compare Equal). Test
   img2 < img10, case ties, unicode (中文 names), leading zeros
   (`img007` vs `img7`: whatever the crate does, it must be stable and total),
   and that sorting a shuffled list twice gives the same result.
2. `core/src/scan.rs` `list(fs, dir)`: call `fs.read_dir(dir)` once, then:
   - drop names starting with `.`;
   - drop litter, compared case-insensitively: `Thumbs.db`, `desktop.ini`,
     `@eaDir`, `#recycle`, `#snapshot`, `$RECYCLE.BIN`,
     `System Volume Information`, `.DS_Store` (already caught by the `.` rule);
   - split into images (`EntryKind::Image`) and dirs; non-image files
     (`read_dir` already returns only images and dirs, but don't rely on it:
     drop anything else);
   - sort each with `natural_cmp` on `name`.
3. `core/src/nav.rs` `next_folder` / `prev_folder`: the rules in the module
   doc and `docs/ux-spec.md` §Folders. Define the sequence
   S = pre-order DFS over `root` (children in natural order), keeping only
   folders that contain at least one image (after `scan::list` filtering;
   the root counts). `next(current)` = the first folder after `current` in S;
   `prev` = the last before it. `current` itself need not be in S (it may be
   an empty folder): position it where it would sit in the pre-order walk.
   Return `Ok(None)` past either end. If `current` is not inside `root`,
   return `Error::new(ErrorCode::Forbidden, ..)`.
   - **Do not walk the whole tree** when a neighbour suffices: a NAS root may
     hold 10k folders. For `next`: descend into `current`'s children first,
     then the following siblings, then climb. For `prev`: previous sibling's
     deepest last descendant first, then the parent. Each visited folder
     costs one `read_dir`; test that with a counting `FsBackend` wrapper (e.g.
     `next` from a leaf with one following sibling costs ≤3 `read_dir` calls).
   - Paths: use `Path::join` / `Path::parent` / `starts_with` only. Never
     format separators by hand.

## Tests
- `core/tests/nav_golden.rs`: build the `docs/api.md` golden tree in
  `photosee_core::testfs::MemFs` (core's dev-dependencies enable the
  `testing` feature, so integration tests see it). Load `contracts/nav.json` with `serde_json` via
  `include_str!` and assert every case.
- Deeper trees: a 3-level tree with empty intermediate folders, a root with
  no images, `current` == an empty folder, `current` outside root.
- scan: hidden, litter, mixed case, dirs and images both sorted.

## Acceptance
`powershell -File scripts/check.ps1 -Rust` and
`node scripts/gates.mjs --ownership A main` both pass. Report the
`read_dir` counts your tests measured.

## Not in scope
Caching (package G's `ListingCache`), path policy (package H), anything in
`server/` or `web/`.
