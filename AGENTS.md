# Working on PhotoSee (read this first: humans, Claude sub-agents, Pi workers)

> **Taking over the project? Read [HANDOFF.md](HANDOFF.md) first**: current state, exact
> build/test/package/release commands, pitfalls hit so far, open issues, what not to do.

PhotoSee is a minimal, very fast image viewer: a Rust core, an axum HTTP API,
and a framework-free TypeScript UI shipped in Tauri 2 and Docker. The plan
lives with the orchestrator. Everything a worker needs is in this repo:

| File | What |
|---|---|
| `docs/api.md` + `contracts/*.json` | HTTP contract and golden bodies |
| `docs/ux-spec.md` | every interaction and number (numbers live in `web/src/constants.ts`) |
| `docs/non-goals.md` | what we refuse to build |
| `docs/deps.md` | every dependency and why |
| `docs/ownership.toml` | which files your package may touch |
| `docs/packages/<WP>.md` | your brief: goal, files, acceptance, references |

## Rules
1. **Touch only your package's paths** (`docs/ownership.toml`). Contract files
   are orchestrator-only. If a contract seems wrong or insufficient, stop.
   Write the problem into `QUESTIONS.md` at the repo root of your worktree.
   Do not invent a workaround.
2. **No new dependencies.** Everything is pre-declared and pinned. If you
   truly need one, explain why in your final report. Spikes and tools that
   have their own `Cargo.toml` with an empty `[workspace]` table may pin
   their own dependencies.
3. **Size limits are hard**: ≤300 lines per file, ≤50 lines per function.
   Split by responsibility; do not raise limits or add `allow` attributes to
   dodge them.
4. **No panics on bad input.** Every byte from a file is hostile. Bounds-check
   offsets, cap allocations, and return `Error::corrupt(..)` instead.
   `unwrap`/`expect` are allowed only in tests and for impossible states (with
   a comment saying why).
5. **Comments say why, not what.** Note pitfalls you hit; they are the most
   valuable lines in the file.
6. **Tests ship with the code**: self-contained, no network, no real NAS.
   Private samples are read only when `PHOTOSEE_PRIVATE_FIXTURES` points at
   a folder; skip cleanly otherwise.
7. **Done means green**: `powershell -File scripts/check.ps1 -Rust` (or
   `-Web`) passes, and so does
   `node scripts/gates.mjs --ownership <WP> main`.
8. **Stop only what you started, by PID.** Several workers share this
   machine. `taskkill /IM node.exe` or `Stop-Process -Name cargo` kills
   everyone's builds and test runs (it happened once).
9. **Report honestly**: what works, what you measured (numbers), what you
   could not verify, and what you left out.

## Build notes (Windows)
- Rust 1.92 (pinned by `rust-toolchain.toml`). MSVC is found automatically
  from either PowerShell or Git Bash.
- Build only what you need: `cargo test -p photosee-core` / `-p photosee-server`.
  Dependencies build at opt-level 2 even in dev, so the first build is slow
  and later ones are fast.
- Web: `cd web && npm ci` once, then `npx vitest run`, `npx eslint .`, `npx tsc --noEmit`.
- clippy's `allow-unwrap-in-tests` only covers `#[test]` functions. Helper
  functions in `*/tests/*.rs` files need a file-level
  `#![allow(clippy::unwrap_used, reason = "...")]`; that is the one allowed
  `allow`.
- Server errors: return `photosee_server::error::ApiError` (or call
  `error_response`) so every non-2xx body matches docs/api.md §Errors.
- Paths: core is OS-agnostic; never hard-code `\` or `/` outside tests.
