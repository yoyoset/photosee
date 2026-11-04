# BR: the browser — folder tree, thumbnail grid, details list (web)

Read `AGENTS.md` first. Owned paths: see `[BR]` in `docs/ownership.toml`.

## Why (user feedback, 2026-11-04)
- The app opened straight into the single-image view. Without an argument it
  showed a black stage with no way to choose anything.
- The user asked for selection and browsing at the level of professional
  viewers (XnView MP, FastStone, ACDSee): "专业一点，不要搞得跟个玩具一样"
  ("be professional, not like a toy").
- The user also wants a layout that can be customised: both a tree and a
  grid, adjustable.

The viewer stays exactly as it is: black stage, fast, minimal. The browser is
the second mode.

## Modes and flow
- **Browser mode** is the default when there is no argument or the argument is
  a folder.
- **Viewer mode** opens directly when the argument is a file. This is the
  fast path; keep the provisional-listing first paint.
- Switching modes:
  - Viewer → browser: Esc. This replaces the `window` layer of the Esc stack;
    closing the window stays on the title-bar button and Ctrl+Q.
  - Browser → viewer: Enter or double-click on an image. Return
    to the same folder with the same image selected and scrolled into view.
- The two modes share one store and listing, so switching never reloads the
  folder.

## Layout (browser mode)
```
┌ toolbar: [◧ tree] [选择文件夹] ‹ › ↑  路径面包屑 / 可编辑地址栏 │ 视图 ▾  排序 ▾  缩略图 ━━●━━ │
├ tree (resizable) ┬ content: grid or details ────────────────────────────────────────────────┤
│ 最近              │                                                                          │
│ 图片 / 桌面        │                                                                          │
│ 此电脑 C: D: Z:    │                                                                          │
│ 网络位置 \\nas\x   │                                                                          │
├──────────────────┴──────────────────────────────────────────────────────────────────────────┤
└ status bar: 412 张图片 · 3 个文件夹 │ 选中: IMG_0012.jpg · 6000×4000 · 8.2 MB · 2024-05-01 13:22 ┘
```

**Toolbar**
- Toggle the tree.
- 选择文件夹 ("choose folder") calls `host.pickFolder()`; it is hidden when
  the host lacks it (browser build).
- Back and forward through visited folders, and up one level.
- Address bar: breadcrumbs, each segment clickable. A click on empty space
  or Ctrl+L turns it into a text input that accepts any path, UNC included.
  Enter navigates; an error shows inline, never as a modal. A UNC path that
  opens successfully is added to 网络位置 ("network places").
- 视图 ("view"): grid or details.
- 排序 ("sort"): name (natural), date modified, size, or type, plus a
  separate ascending/descending toggle.
- Thumbnail size slider, 64–320 px; Ctrl+wheel over the grid changes it
  too.
- Toggle for showing file names under thumbnails.

**Tree**
- Sections:
  - 最近 ("recent"): the last 10 opened folders, newest first, each
    removable.
  - Places from `session.roots`. The `folder` kind is labelled through i18n
    (Pictures → 图片, Desktop → 桌面); drives use `C:`.
  - 网络位置 ("network places"): user-added UNC paths, each removable.
- Nodes expand lazily, one `/list` per expansion; only `dirs` are used.
- The current folder is highlighted, and its ancestors are expanded when
  reachable.
- Keyboard: ↑ ↓ to move, ← and → to collapse and expand, Enter to open.
- The tree's width can be dragged between 160 and 480 px.

**Grid**
- Virtualized: only visible rows plus overscan are in the DOM, and it must
  stay smooth at 10 000 items. Test the DOM count.
- Square cells sized by the slider. Thumbnails are fitted (`object-fit:
  contain`) on a slightly lighter cell background. `ctx.src.thumbUrl(path,
  edge)` is requested at `cellPx × dpr`, clamped to 64..512 with the step
  quantized to a few sizes so the cache hits.
- Folders come first as cells with a folder glyph and their name. There is
  no cover image, because that would cost NAS IO per folder.
- Selection:
  - A single selection with a clear focus ring.
  - Arrows, Home/End and PgUp/PgDn move it.
  - Typing letters jumps to the next name with that prefix (type-ahead,
    reset after 1 s).
  - Enter opens the selection: a folder navigates, an image enters the
    viewer.
  - Backspace or Alt+↑ goes up one level.
- Shows thumbnails only, so it never decodes full images.

**Details list**: also virtualized.
- Columns: name with a small icon, date modified, type (extension upper
  case), size, and dimensions.
- Dimensions are filled lazily from `/meta` for visible rows only, cached
  per path, with no request storm while scrolling: debounce 150 ms.
- Clicking a header sorts by that column; a second click reverses the
  order.

**Status bar**
- Counts for the folder.
- For the selection: name, size and date from the listing, and dimensions
  when known.

**Persisted** through `api.uiState()` / `api.saveUiState()`, debounced 500 ms,
as one object with a `version: 1` field:
- tree visible and width;
- view mode, sort key and direction;
- thumbnail size and the show-names flag;
- recent folders and network places;
- the last folder.

A missing or invalid state falls back to defaults: tree visible, 220 px,
grid, name ascending, 160 px, names shown. Unknown fields are kept as they
are.

**Start**
- No argument: the last folder if it still lists, else Pictures, else the
  first place.
- A folder argument: that folder.

## Look (professional, not a toy)
- Dark UI that matches the viewer: surfaces in very dark greys (#121212 to
  #1e1e1e), not pure black; the image stage stays pure black.
- One accent colour, used for selection and focus only.
- System font, 13 px; truncate names in the middle so the extension stays
  visible.
- Thin 1 px dividers, no shadows or rounded "cards". Icons are inline SVG at
  16 px, monochrome.
- Hover, selection and focus states are visible and distinct, and pass WCAG
  AA contrast.
- No animation beyond 120 ms; none under `prefers-reduced-motion`.
- Mouse: a single click selects and a double-click opens, as in Explorer.
  Right-click does nothing; there are no file operations (non-goals).

## Integration
Integration goes through the existing controller and boot files, which you
own for this package.
- Add a `mode: 'browser' | 'viewer'` field to the store. `state.ts` is a
  contract file, but you may add this one field and its initial value; say
  so in the report.
- Keys: in browser mode the viewer's key map is inactive, and the browser
  handles its own keys. The overlay and filmstrip exist only in viewer mode.
- Strings: add the keys you need to `web/src/i18n.ts`, zh and en. You may
  edit it for this package, adding keys only and changing none.

## Tests (vitest + jsdom)
- Pure modules: sort comparators (natural name, date, size, type, both
  directions, folders first), grid geometry (columns for a width, visible
  range, the index under keyboard moves including PgUp/PgDn), type-ahead,
  ui-state parse with defaults, unknown fields preserved, history
  back/forward/up.
- DOM:
  - 10 000 items keep fewer than 200 cell nodes;
  - Enter on an image enters the viewer at that index;
  - Esc in the viewer returns to the browser with the same selection;
  - the address bar accepts a typed path;
  - a failed path shows inline;
  - the tree expands lazily, one `/list` per expansion;
  - the pick-folder button is hidden without `host.pickFolder`.
- Boot:
  - no start → last folder from ui-state;
  - a file start → viewer;
  - a folder start → browser.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership BR main` pass. The bundle stays
≤ 60 KB gzip; report it.

## Not in scope
- File operations of any kind (copy, move, delete, rename).
- Multi-select.
- Ratings and tags.
- Search.
- Folder cover images.
- A recursive "all subfolders" view (`docs/non-goals.md`).
