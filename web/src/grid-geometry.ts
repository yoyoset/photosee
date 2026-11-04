// Grid arithmetic, kept free of the DOM so it can be tested with numbers.
// The same geometry serves the details list (one column, thin rows).

import {
  BOX_STEP_PX, CELL_LABEL_PX, CELL_PAD_PX, GRID_MARGIN_PX, THUMB_BOX_THRESHOLD_PX, THUMB_EDGES, THUMB_MAX_PX,
  THUMB_MIN_PX,
} from './browser-consts';

export interface Layout {
  cols: number;
  cellW: number;
  cellH: number;
  margin: number;
}

export interface Range { start: number; end: number }

export interface SliderAction { view?: 'grid' | 'single'; thumb?: number }

/** What the size slider means at `px`: the notch past the maximum is the
 * single-image view; anything below it is a thumbnail size, and leaves single
 * view for the grid when that is where it was. */
export function sliderAction(px: number, view: 'grid' | 'details' | 'single'): SliderAction {
  if (px > THUMB_MAX_PX) return { view: 'single' };
  return view === 'single' ? { view: 'grid', thumb: clampThumb(px) } : { thumb: clampThumb(px) };
}

export type MoveKey = 'left' | 'right' | 'up' | 'down' | 'home' | 'end' | 'pageup' | 'pagedown';

export const clampThumb = (px: number): number => Math.min(THUMB_MAX_PX, Math.max(THUMB_MIN_PX, Math.round(px)));

/** Square thumbnail cells; the label strip is extra height, not extra width. */
export function gridLayout(width: number, size: number, names: boolean): Layout {
  const cellW = size + CELL_PAD_PX * 2;
  const cols = Math.max(1, Math.floor((width - GRID_MARGIN_PX * 2) / cellW));
  return { cols, cellW, cellH: cellW + (names ? CELL_LABEL_PX : 0), margin: GRID_MARGIN_PX };
}

export function listLayout(width: number, rowH: number): Layout {
  return { cols: 1, cellW: width, cellH: rowH, margin: 0 };
}

export const rowCount = (count: number, cols: number): number => Math.ceil(count / Math.max(1, cols));

export const totalHeight = (l: Layout, count: number): number =>
  rowCount(count, l.cols) * l.cellH + l.margin * 2;

export function cellPos(l: Layout, i: number): { x: number; y: number } {
  return {
    x: l.margin + (i % l.cols) * l.cellW,
    y: l.margin + Math.floor(i / l.cols) * l.cellH,
  };
}

/** Half-open item range [start, end) intersecting the viewport, widened by
 * `overscan` rows each side. Whole rows only, so a row is never half built. */
export function visibleRange(
  l: Layout, count: number, scrollTop: number, viewH: number, overscan: number,
): Range {
  if (count <= 0 || viewH <= 0 || l.cellH <= 0) return { start: 0, end: 0 };
  const rows = rowCount(count, l.cols);
  const first = Math.floor((scrollTop - l.margin) / l.cellH) - overscan;
  const last = Math.ceil((scrollTop + viewH - l.margin) / l.cellH) + overscan;
  const r0 = Math.min(rows, Math.max(0, first));
  const r1 = Math.max(r0, Math.min(rows, last));
  return { start: Math.min(count, r0 * l.cols), end: Math.min(count, r1 * l.cols) };
}

/** Where the selection goes for a navigation key; the index is kept when the
 * key cannot move it (an edge). `horizontal` is false in the details list,
 * where Left/Right do nothing. */
export function moveIndex(
  index: number, key: MoveKey, cols: number, count: number, pageRows: number, horizontal = true,
): number {
  if (count <= 0) return -1;
  if (index < 0 || index >= count) return key === 'end' || key === 'up' ? count - 1 : 0;
  const last = count - 1;
  switch (key) {
    case 'home': return 0;
    case 'end': return last;
    case 'left': return horizontal ? Math.max(0, index - 1) : index;
    case 'right': return horizontal ? Math.min(last, index + 1) : index;
    case 'up': return index - cols >= 0 ? index - cols : index;
    // Down from a shorter last row lands on the last item, as Explorer does.
    case 'down': return index + cols <= last ? index + cols
      : Math.floor(index / cols) < Math.floor(last / cols) ? last : index;
    case 'pageup': return Math.max(index % cols, index - pageRows * cols);
    case 'pagedown': return Math.min(last, index + pageRows * cols);
  }
}

/** New scrollTop that brings the row `[top, top+h)` fully into view with the
 * least movement; unchanged when it already is. */
export function scrollToReveal(scrollTop: number, viewH: number, top: number, h: number, pad = 0): number {
  if (top - pad < scrollTop) return Math.max(0, top - pad);
  if (top + h + pad > scrollTop + viewH) return top + h + pad - viewH;
  return scrollTop;
}

/** Edge to request for a cell of `cssPx` on a screen with `dpr`: at least the
 * device pixels, rounded up to a step so the cache is hit across slider moves. */
export function thumbEdge(cssPx: number, dpr: number): number {
  const want = cssPx * Math.max(1, dpr);
  const max = THUMB_EDGES[THUMB_EDGES.length - 1] as number;
  return THUMB_EDGES.find((e) => e >= want) ?? max;
}

export type ThumbRequest = { edge: number } | { box: number };

/** How a cell gets its picture. Up to ~256 device px the /thumb endpoint is
 * right (cheap, cached). Beyond that it is not: /thumb is capped at 512 and
 * an EXIF thumbnail inside a photo is only 160 px, so big cells would be
 * blurry; they ask for a resized image instead, in coarse box steps so
 * dragging the slider keeps hitting the browser cache. */
export function thumbRequest(cssPx: number, dpr: number): ThumbRequest {
  const need = cssPx * Math.max(1, dpr);
  if (need <= THUMB_BOX_THRESHOLD_PX) return { edge: thumbEdge(cssPx, dpr) };
  return { box: Math.ceil(need / BOX_STEP_PX) * BOX_STEP_PX };
}
