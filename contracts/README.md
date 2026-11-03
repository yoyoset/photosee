# Golden contract files

Exact expected response bodies for `docs/api.md`. Server tests assert equality
(`meta.json` is matched for shape and types, since it needs a real camera
file); web tests use them as fetch mocks. Only the orchestrator edits these.

`nav.json` encodes the folder rules on the fixture tree:
- `2024` is empty, so it is skipped.
- Order is pre-order depth-first, and the root itself counts as a folder when
  it has images: previous from `/photos/2023` is `/photos`.
- Previous from the root is `null`, and so is next from the last folder.
