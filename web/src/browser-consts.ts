// Numbers for the browser mode (docs/packages/BR.md). They live here, not in
// constants.ts, because that file belongs to the orchestrator; same rule:
// never hard-code them elsewhere.

export const TREE_MIN_PX = 160;
/** The tree may take at most this share of the window (computed at drag time
 * and on resize, so a stored width never overflows a smaller window). */
export const TREE_MAX_FRACTION = 0.5;
/** Only bounds a corrupt ui-state file; the real limit is the window. */
export const TREE_STORED_MAX_PX = 4096;
/** Recent shows this many rows and scrolls beyond that, so the sections
 * below it never move as the list grows. The row height is CSS (`--tr-row`):
 * 24 px for a mouse, taller for a finger. */
export const RECENT_VISIBLE_ROWS = 5;
export const TREE_DEFAULT_PX = 220;
/** At or below this window width the tree becomes a drawer over the files
 * (browser-drawer.ts). Phones held upright are 360–430 px; tablets (768+)
 * keep the column, where a 220 px tree still leaves room for the files. */
export const NARROW_MAX_PX = 640;

export const THUMB_MIN_PX = 64;
export const THUMB_MAX_PX = 640;
/** One extra slider notch past the max means "single image" view. */
export const SINGLE_NOTCH_PX = 8;
/** Cells needing more device px than this ask for a resized image instead of /thumb. */
export const THUMB_BOX_THRESHOLD_PX = 256;
/** Requested box sizes for big cells: coarse steps, so resizing hits the cache. */
export const BOX_STEP_PX = 128;
export const THUMB_DEFAULT_PX = 160;
/** Ctrl+wheel: one notch changes the size by this much. */
export const THUMB_WHEEL_STEP_PX = 16;
/** Requested thumbnail edges. A few fixed steps, so a slider drag does not
 * make every size a cache miss on the server. */
export const THUMB_EDGES: readonly number[] = [64, 96, 128, 192, 256, 384, 512];

/** Grid cell chrome (CSS px): padding around the thumbnail, label strip. */
export const CELL_PAD_PX = 6;
export const CELL_LABEL_PX = 22;
/** Space around the whole grid. */
export const GRID_MARGIN_PX = 8;
/** Rows rendered beyond each edge of the viewport. */
export const OVERSCAN_ROWS = 3;

export const DETAILS_ROW_PX = 24;
/** Under `pointer: coarse`: a 24 px row is half a fingertip (phone review,
 * 2026-11-06). Matches TOUCH_TARGET_PX; browser.css sets --dr-h to it. */
export const DETAILS_ROW_TOUCH_PX = 44;

export const RECENT_MAX = 10;
/** Network places are a small list; this only bounds a corrupt file. */
export const LIST_MAX = 50;
export const SAVE_DEBOUNCE_MS = 500;
/** Dimensions are fetched only after scrolling has been quiet this long. */
export const META_DEBOUNCE_MS = 150;
export const META_CONCURRENCY = 4;
export const TYPEAHEAD_RESET_MS = 1000;

/** Single-image view: the requested box is the pane rounded up to this many
 * device px, so a small window resize keeps the same URL (and cache entry). */
export const SINGLE_BOX_STEP_PX = 64;
/** Images kept decoded ahead/behind in single view: next 2, previous 1. */
export const SINGLE_AHEAD = 2;
export const SINGLE_BEHIND = 1;
/** Decoded neighbours kept (current + prefetched). */
export const SINGLE_CACHE = 6;
