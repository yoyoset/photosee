// Every timing and threshold in the UI. Values and reasons: docs/ux-spec.md.
// Contract file: orchestrator only. Never hard-code these elsewhere.

/** Wait this long before showing a loading indicator (no flicker on SMB). */
export const LOADING_DELAY_MS = 450;
/** Once shown, keep the indicator at least this long. */
export const LOADING_MIN_VISIBLE_MS = 500;
/** Overlay fades after this much inactivity (never while hovered/focused). */
export const OVERLAY_HIDE_MS = 2800;
export const OVERLAY_FADE_MS = 200;
/** Hide the cursor in fullscreen after this much inactivity. */
export const CURSOR_HIDE_MS = 2800;
/** Pointer within this many CSS px of the bottom edge reveals the overlay. */
export const OVERLAY_REVEAL_ZONE_PX = 120;

/** Trackpad/wheel: accumulate |deltaY| to this before one page turn. */
export const WHEEL_PAGE_DELTA = 100;
/** ...then ignore wheel page turns for this long. */
export const WHEEL_COOLDOWN_MS = 250;
/** Multiplicative step per zoom notch / key press. */
export const ZOOM_STEP = 1.25;
export const ZOOM_MAX = 16;
/** Zoom percentage toast. */
export const ZOOM_TOAST_MS = 1000;

/** Touch: a tap in this fraction of the width on either side turns the page
 * (about thirds, like comic readers; the middle shows/hides the bars). */
export const TAP_TURN_FRACTION = 0.3;
/** Pull-down to close. */
export const PULL_CLOSE_PX = 100;
export const PULL_CLOSE_PX_PER_MS = 0.11;
/** Single tap waits this long to rule out a double tap. */
export const DOUBLE_TAP_MS = 300;
/** Mouse edge-click page turn zone, fraction of width on each side. */
export const EDGE_CLICK_FRACTION = 0.15;

/** Holding an arrow key: show thumbnails only until keys are quiet this long. */
export const KEY_REPEAT_SETTLE_MS = 150;

/** Prefetch window around the current index. */
export const PREFETCH_AHEAD = 2;
export const PREFETCH_BEHIND = 1;
/** Decoded-image cache budget (approx. width*height*4 bytes per entry). */
export const CACHE_BUDGET_BYTES_DESKTOP = 512 * 1024 * 1024;
export const CACHE_BUDGET_BYTES_MOBILE = 256 * 1024 * 1024;

/** Aspect ratio above which an image is a panorama: fit width, scroll. */
export const PANORAMA_ASPECT = 3;

export const SLIDESHOW_FADE_MS = 300;
export const FOLDER_TOAST_MS = 1500;
/** A full-sentence hint ("last image · Ctrl+→ next folder") needs longer
 * than a folder name to be read. */
export const HINT_TOAST_MS = 3000;
export const SLIDESHOW_INTERVAL_MIN_S = 1;
export const SLIDESHOW_INTERVAL_MAX_S = 60;

/** Filmstrip thumbnail height (CSS px) and requested edge (device px). */
export const FILMSTRIP_THUMB_PX = 64;
export const THUMB_EDGE = 256;
/** Minimum touch target (CSS px ~= pt). */
export const TOUCH_TARGET_PX = 44;
