# TASKS (orchestrator-owned)

Status: `todo` · `running` · `review` · `merged` · `blocked`

## C0 contracts
| step | status | note |
|---|---|---|
| MSVC toolchain | merged | No install needed: rustc and cc-rs locate VS 18 Insiders via vswhere from both PowerShell and Git Bash (probe crate compiled C and linked). The plan assumed `cl` missing from PATH meant Build Tools were required; that was wrong. |
| repo, contracts, skeleton, gates | merged | `scripts/check.ps1` green. Gates verified to fail on an undocumented dependency and on a 306-line file. |

## M0 spike (gate G0)
| WP | owner | status | branch |
|---|---|---|---|
| M0-T transport + cold start | Claude sub-agent | todo | wp/m0-t |
| M0-R RAW preview extractor | Claude sub-agent | todo | wp/m0-r |
| M0-S SMB bench tool | progate | todo | wp/m0-s |
| M0-F fixture generator | flashgate | todo | wp/m0-f |

Blocked on the user: NAS share path (to run M0-S), private RAW/HEIC samples
(for M0-R's real-file test).

## M1 Wave 1
Briefs are written after G0, because numbers from M0 may change the
budgets and the transport choice.

## Decisions changed during C0 (vs. the approved plan)
- **Slideshow state machine moved from `core/` to `web/src/slideshow.ts`.**
  The timer runs in the UI, and a round trip per tick buys nothing, so the
  pure state machine lives in TS (package B, still pure and tested). Folder
  resolution stays in core (`/nav`).
- **`/list` returns one JSON document, not NDJSON.** Natural sort needs every
  entry before the first line can be emitted, so streaming would buy nothing.
  The first image never waits for the listing anyway.
- **AVIF is passthrough-only.** `image`'s `avif-native` needs a dav1d C build.
  It is recorded in docs/non-goals.md.
