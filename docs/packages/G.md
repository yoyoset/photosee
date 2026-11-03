# G: HTTP routes, IO priority queue, Range, listing cache (server)

Read `AGENTS.md` first. Owned paths: see `[G]` in `docs/ownership.toml`.
Core packages A (scan/nav), C (pipeline) and F (thumb/meta) and server
package H (paths, guard) are merged. Use them; don't reimplement them.

## Why
This is the single API for the desktop shell and the Docker build. The NAS
is the bottleneck, so the server's job is to make sure the image the user
is waiting for never queues behind 30 thumbnail reads.

## Implement
1. `queue.rs` `IoQueue::run(prio, job)`:
   - At most `limit` jobs execute at once.
   - Waiting jobs start in `Priority` order, FIFO within a priority. Use a
     `Mutex<BinaryHeap>` of waiters woken through `tokio::sync::oneshot` or
     `Notify`; no busy wait.
   - A cancelled caller (the HTTP request dropped because the user paged on)
     must free its place in the queue. If it has not started yet, it never
     runs.
   - Replace the stub's `expect` on a panicked job with an `Io` error.
     Changing `run` to return `Result<T>` is allowed; the signature is yours
     beyond the stub.
2. `range.rs` `parse(header, len)`: single `bytes=a-b`, `bytes=a-`,
   `bytes=-n`. Anything invalid, multi-range or unsatisfiable → `None`.
   Satisfiable means `a < len`. This is pure, so write a table test.
3. `cache.rs` `ListingCache`:
   - `Mutex<HashMap>` keyed by directory, valid while the directory mtime
     matches.
   - Bounded: 512 entries; evict the oldest inserted entry.
4. `routes.rs` (split into `routes/*.rs` by endpoint if it grows):
   - Every endpoint in `docs/api.md` under `/api/v1`. The guard layer
     covers everything, including static files. Static files come from
     `web_dir` via `tower_http::services::ServeDir`, with `index.html` as
     the fallback.
   - Errors go through `crate::error::ApiError`.
   - Blocking filesystem and decode work goes through
     `state.queue.run(prio, ..)`. Use:
     - `Current` for `/list`, `/meta`, `/nav`, and `/img?prio=current`;
     - `Prefetch` for `/img?prio=prefetch`;
     - `Thumb` for `/thumb`.
   - Wrap each queued job in `tokio::time::timeout(5 s)` → `Timeout`. The
     UI's NAS banner shows "超时 5s". Put the 5 s in one named constant.
   - `/session`:
     - `mode`: `desktop` for `Policy::AnyLocal`, `server` for `Roots`.
     - `platform` from `cfg!(target_os)`.
     - `start` from `state.start`: a file gives `{dir: parent, file}`; a
       directory gives `{dir, file: null}`; `stat` decides which.
     - `roots`: `[{name, path: "/name"}]`, empty on desktop.
   - `/list`:
     - Stat the dir, try the cache, then `scan::list` and put the result.
     - Build `path` for each entry with `policy.to_logical(dir.join(name))`.
     - `parent` is null at a root (Roots: the logical path is `/name`;
       desktop: `Path::parent()` is None).
   - `/img`:
     - Parse `w`/`h` together; one without the other → 400 with the
       `unsupported` body.
     - Native formats come from `Accept` (`image/webp`, `image/avif`,
       `image/jxl`, `image/heic`).
     - Call `pipeline::render`.
     - Passthrough: read the file, or the preview range for RAW, through
       `fs.open` and honour `Range` (206 with `Content-Range`; 416 when
       unsatisfiable).
     - Response headers: `Content-Type`, `ETag` (stable hash of logical
       path + size + mtime + w + h), `Cache-Control: private, max-age=60`,
       `X-Photosee-Delivery`.
     - `If-None-Match` hit → 304 with no work done. Check it **before**
       queueing.
   - `/thumb`:
     - `s` clamped to 64..=512, default 256.
     - `ThumbCache::key` → `get`, else `thumb::make` → `put`.
     - JPEG, same caching headers as `/img`.
   - `/meta`: `pipeline::info` + `meta::exif_summary` + `stat`. Width and
     height are the **display** size. Shape per `contracts/meta.json`.
   - `/nav`: resolve both `p` and `root`, `nav::next_folder` /
     `prev_folder`, then map the result back to logical.
   - `/config`: GET → `config::load(settings_path)`. PUT → parse, normalize,
     `config::save`, return the normalized object. A bad JSON body → 400
     `unsupported`.
5. `server/src/main.rs` stays as it is (M2).

## Tests (`server/tests/api_*.rs`, tower `oneshot`, MemFs)
- The `docs/api.md` golden tree under `Policy::Roots` (root `photos`, dir
  `/photos` in MemFs). Assert exact equality with `contracts/list.json`,
  `nav.json` (all cases), `config.json` and `session.json`. Substitute the
  runtime `platform` before comparing.
- `/meta` shape and types against `meta.json`, using a real small JPEG with
  an EXIF orientation of 6 built in the test.
- `/img`:
  - passthrough with Range;
  - 304 on `If-None-Match`;
  - transcode for a large image;
  - `X-Photosee-Delivery` values;
  - error bodies (`not_found`, `forbidden` traversal).
- Queue:
  - with limit 1 and a blocking job in flight, enqueue thumb, prefetch,
    current → they run current, prefetch, thumb;
  - a dropped waiter never runs;
  - a panicking job → `Io`, and the slot is freed.
- Timeout: a MemFs wrapper that sleeps 6 s → 504 `timeout`. Use tokio's
  paused clock if it applies to `spawn_blocking`; otherwise make the
  timeout configurable for tests.

## Acceptance
`powershell -File scripts/check.ps1 -Rust` and
`node scripts/gates.mjs --ownership G main` pass. Report the requests per
second for `/thumb` cache hits and the p50 for `/img` passthrough of a
3 MB file (loopback, release build).

## Not in scope
- Path policy and the guard (H).
- `main.rs` and login (M2).
- Thumbnail generation itself (F).
