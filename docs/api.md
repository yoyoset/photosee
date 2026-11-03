# PhotoSee HTTP API v1

The only interface between `web/` and the Rust side, used by the desktop shell
(embedded server on 127.0.0.1) and by the Docker server. Changing anything here
is a contract change: the orchestrator does it in its own commit, bumps
`API_VERSION` in `core/src/types.rs`, and updates `contracts/*.json`.

## Conventions

- Base path `/api/v1`. UTF-8 JSON, `snake_case` keys.
- **Logical paths** (`p`, and every `path` in responses) are opaque strings to
  the web UI. The UI never builds or splits a path; it only sends back paths
  the server gave it.
  - Desktop (`mode: "desktop"`): the native absolute path, e.g. `C:\Photos\a.jpg`, `\\nas\share\b.jpg`.
  - Docker (`mode: "server"`): `/<root>/<relative>`, e.g. `/photos/2024/a.jpg`.
- Query values are percent-encoded.
- Auth: the desktop shell opens `/?t=<token>` once. The server answers
  `303` to `/` and sets the token as an HttpOnly, SameSite=Strict `ps_token`
  cookie, so it never stays in the URL or reaches page script. After that,
  every request (UI files included) must carry the cookie. Docker with `PHOTOSEE_PASSWORD` uses the same
  cookie, set by `POST /api/v1/login` (M2). Requests with a `Host` header not
  on the allow-list get 403 (DNS-rebinding defence).

## Errors

Any non-2xx response has this body:

```json
{ "code": "missing_codec", "codec": "hevc_ext", "message": "WIC: component not found (0x88982F50)" }
```

| code | HTTP | meaning |
|---|---|---|
| `not_found` | 404 | path does not exist |
| `forbidden` | 403 | outside roots, bad token, bad Host |
| `unsupported` | 415 | format not supported |
| `missing_codec` | 415 | `codec` is one of `heif_ext`, `hevc_ext`, `libheif` |
| `corrupt` | 422 | file is truncated or malformed |
| `too_large` | 413 | over the decode pixel limit (200 MP) |
| `timeout` | 504 | the filesystem (usually a NAS) did not answer in time |
| `io` | 502 | any other filesystem failure |

`message` is technical detail for the "details" line. The UI maps `code` (and
`codec`) to a localized sentence and an action. The server never sends UI text.

## Endpoints

### GET /session
Start-up facts. Golden file: `contracts/session.json`.
- `start` is the file or folder the app was opened with, or null.
- `roots` is empty on desktop.

### GET /list?p=DIR
Images and subfolders of a directory. Both lists are naturally sorted
(`img2` < `img10`), and hidden entries and NAS litter are removed. `fmt` is
guessed from the file extension; `/img` confirms it from magic bytes.
`parent` is null at a root. Golden file: `contracts/list.json`.

The server caches listings and rescans only when the directory's mtime
changes. On a cold NAS this call can take seconds, so the UI must not wait
for it before showing the first image.

### GET /img?p=FILE[&w=W&h=H][&prio=current|prefetch]
Image bytes for display.
- `w`/`h` are the box to fill, in **device pixels**: fit size × devicePixelRatio.
- Omit `w`/`h` for the original pixels (zoom to 100% or more).
- `prio` defaults to `current`.

| Situation | Server action |
|---|---|
| Format is native to the client AND (animated OR pixels ≤ 1.5 × w×h) | Send the file unchanged. `Range` is supported. The webview decodes and orients it. |
| RAW with a Normal-oriented embedded JPEG that is ≤ 1.5 × w×h | Send that byte range unchanged |
| Anything else | Decode, apply orientation once, convert to sRGB, resize to fit (w,h) without upscaling, encode JPEG (PNG if there is alpha). The output carries no Orientation tag. |

"Native" = jpeg png gif bmp ico svg, plus whatever the request's `Accept`
header lists (`image/webp`, `image/avif`, `image/jxl`, `image/heic`).

Headers:
- `Content-Type`
- `ETag`: hash of path + size + mtime + w + h
- `Cache-Control: private, max-age=60`
- `X-Photosee-Delivery: passthrough | range | transcode`, for debugging and benchmarks

The UI reads the original size from `/meta`, not from these headers, because
`<img>` cannot read response headers.

### GET /thumb?p=FILE[&s=256]
Filmstrip thumbnail. Returns a JPEG whose longest edge is `s` (64..=512,
default 256). The EXIF embedded thumbnail is used when it exists; otherwise
the server generates one and caches it on disk. Animated images give their
first frame. Same caching headers as `/img`.

### GET /meta?p=FILE
Facts for fitting and for the info overlay, without decoding pixels.
`width`/`height` are the **display** size, after orientation. Golden file:
`contracts/meta.json`.

### GET /nav?p=DIR&root=ROOT&d=next|prev
Folder to visit next. Rules:
1. Siblings in natural order.
2. When siblings run out, climb one level and continue depth-first.
3. Skip folders with no images.
4. Never leave `root`.

Returns `{"dir": PATH}`, or `{"dir": null}` at the end. Golden file:
`contracts/nav.json`.

### GET /config, PUT /config
The six user settings. PUT takes the full object and returns the normalized
object (for example `interval_s` clamped to 1..=60). Golden file:
`contracts/config.json`.

## Golden-file fixture tree

Server contract tests build this tree in `MemFs` under `Policy::Roots` with
root `photos` → `/photos`. The golden files are the exact expected bodies.

```
/photos/a10.jpg    (bytes "x")
/photos/a2.jpg     (bytes "x")
/photos/.hidden.jpg
/photos/notes.txt
/photos/2023/b.png
/photos/2024/        (empty)
/photos/2025/c.jpg
```
