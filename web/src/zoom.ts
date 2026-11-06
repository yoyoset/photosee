// Pure geometry for fitting, zooming and panning. Package I. No DOM.
//
// Units (docs/packages/I.md), used by every function here:
// - `scale` is device pixels per image pixel; 1 = 100%.
// - `x`/`y` (pan) are the CSS-px offset of the image center from the view
//   center, in screen axes (rotation does not change them).
// - Image boxes are display size (after EXIF orientation) in image px. Pass
//   them through `rotated()` for the display-only quarter turns first.
// - View boxes and anchors are CSS px; anchors are relative to the stage's
//   top-left corner (clientX - stage.left), so a full-window stage takes
//   clientX/clientY unchanged.

import { PANORAMA_ASPECT, ZOOM_MAX, ZOOM_STEP } from './constants';
import type { FitMode, ViewState } from './state';

export interface Box { w: number; h: number }
export interface Point { x: number; y: number }
export type Zoom = ViewState['zoom'];

/** What the zoom helpers need to know; `Viewer.geometry()` returns it. */
export interface Geometry {
  /** Image display size after orientation AND rotation, image px. */
  img: Box;
  /** Stage size, CSS px. */
  view: Box;
  dpr: number;
  /** Which edge "fit" fills (ViewState.fit). */
  fit: FitMode;
}

/** A page that is bigger than the view once fitted (fit width on a tall
 * comic page) opens at its top-left, where reading starts, not its middle:
 * a pan this large is clamped to that corner by clampPan, and to 0 on an
 * axis that fits. Every helper below clamps before it moves from it. */
const START = 1e9;
export const FIT: Zoom = { mode: 'fit', scale: 1, x: START, y: START };

const valid = (b: Box): boolean => b.w > 0 && b.h > 0 && Number.isFinite(b.w) && Number.isFinite(b.h);
const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));
// Floats: 1000 * 1.1 is 1100.0000000000002, and a plain ceil would ask the
// server for one pixel more than the screen has.
const ceilPx = (v: number): number => Math.max(1, Math.ceil(v - 1e-6));

export function isPanorama(img: Box): boolean {
  if (!valid(img)) return false;
  return img.w / img.h > PANORAMA_ASPECT || img.h / img.w > PANORAMA_ASPECT;
}

/** Swap width and height for odd quarter turns. */
export function rotated(img: Box, quarterTurns: number): Box {
  return Math.abs(quarterTurns) % 2 === 1 ? { w: img.h, h: img.w } : img;
}

/** Scale that fits `img` in `view` (CSS px) for `mode`. Small images are
 * enlarged too: the user reads comics on phones, and a 700 px scan shown
 * 1:1 on a 1080 px screen was "only a little bit" of it (2026-11-06).
 * 'contain' shows the whole image, except that a panorama fits its short
 * axis, so it fills the view across and pans along its length. */
export function fitScale(img: Box, view: Box, dpr: number, mode: FitMode = 'contain'): number {
  if (!valid(img) || !valid(view) || !(dpr > 0)) return 1;
  const sx = (view.w * dpr) / img.w;
  const sy = (view.h * dpr) / img.h;
  if (mode === 'width') return sx;
  if (mode === 'height') return sy;
  // Not max(sx, sy): a 3.5:1 image in a 4:1 window would then overflow
  // vertically although it fits whole. The short axis is the image's own.
  return isPanorama(img) ? (img.w > img.h ? sy : sx) : Math.min(sx, sy);
}

/** True when the fitted image is bigger than the view, so a drag or the
 * wheel moves it instead of turning the page (fit width/height, panoramas). */
export function overflows(g: Geometry): boolean {
  const s = fitScale(g.img, g.view, g.dpr, g.fit) / g.dpr;
  return g.img.w * s > g.view.w + 0.5 || g.img.h * s > g.view.h + 0.5;
}

/** Device-px box for /img. The server fits the image inside it and never
 * upscales. Fit width/height reach PANORAMA_ASPECT times past the screen on
 * the free axis, so any page up to that shape comes back sharp from the
 * first request. That box does not depend on the image, which keeps
 * prefetched neighbours (size unknown) and the page on screen on one URL.
 * A longer image (known `img`) grows the box to its fitted size. */
export function requestBox(img: Box | null, view: Box, dpr: number, mode: FitMode = 'contain'): Box {
  const base = { w: ceilPx(view.w * dpr), h: ceilPx(view.h * dpr) };
  if (mode === 'width') base.h = Math.max(base.h, ceilPx(view.w * dpr * PANORAMA_ASPECT));
  if (mode === 'height') base.w = Math.max(base.w, ceilPx(view.h * dpr * PANORAMA_ASPECT));
  if (!img || !valid(img)) return base;
  // Capped at 1: past the original the server only sends the original.
  const s = Math.min(1, fitScale(img, view, dpr, mode));
  return { w: Math.max(base.w, ceilPx(img.w * s)), h: Math.max(base.h, ceilPx(img.h * s)) };
}

/** Fit width/height name screen axes; the request box is in image axes, so
 * a quarter turn swaps them. */
export function rotatedFit(mode: FitMode, quarterTurns: number): FitMode {
  if (mode === 'contain' || Math.abs(quarterTurns) % 2 === 0) return mode;
  return mode === 'width' ? 'height' : 'width';
}

/** The scale actually on screen for a zoom state. */
export function effectiveScale(z: Zoom, g: Geometry): number {
  if (z.mode === 'fit') return fitScale(g.img, g.view, g.dpr, g.fit);
  if (z.mode === 'actual') return 1;
  return z.scale > 0 ? z.scale : 1;
}

/** Keep the image edges outside the view edges; a side that is smaller than
 * the view is centered. Returns a state with `scale` made explicit. */
export function clampPan(z: Zoom, g: Geometry): Zoom {
  const s = effectiveScale(z, g) / g.dpr;
  const maxX = Math.max(0, (g.img.w * s - g.view.w) / 2);
  const maxY = Math.max(0, (g.img.h * s - g.view.h) / 2);
  // `+ 0` turns -0 into 0, so states compare equal and CSS never says -0px.
  const x = clamp(Number.isFinite(z.x) ? z.x : 0, -maxX, maxX) + 0;
  const y = clamp(Number.isFinite(z.y) ? z.y : 0, -maxY, maxY) + 0;
  return { mode: z.mode, scale: effectiveScale(z, g), x, y };
}

/** Anchor relative to the view center; the view center when omitted. */
function centered(g: Geometry, anchor?: Point): Point {
  if (!anchor) return { x: 0, y: 0 };
  return { x: anchor.x - g.view.w / 2, y: anchor.y - g.view.h / 2 };
}

/** Multiply the scale by `factor` around `anchor`: the image point under the
 * anchor stays put. Clamped to [fit, ZOOM_MAX]; landing on fit gives mode
 * 'fit' so later resizes keep fitting. */
export function zoomAt(z0: Zoom, g: Geometry, factor: number, anchor?: Point): Zoom {
  const z = clampPan(z0, g);
  const fit = fitScale(g.img, g.view, g.dpr, g.fit);
  const s0 = effectiveScale(z, g);
  const f = factor > 0 && Number.isFinite(factor) ? factor : 1;
  const s1 = clamp(s0 * f, fit, Math.max(fit, ZOOM_MAX));
  const a = centered(g, anchor);
  const k = s1 / s0;
  const atFit = Math.abs(s1 - fit) <= fit * 1e-9;
  const next: Zoom = { mode: atFit ? 'fit' : 'custom', scale: s1, x: a.x - (a.x - z.x) * k, y: a.y - (a.y - z.y) * k };
  return clampPan(next, g);
}

/** One wheel notch or key press: `ZOOM_STEP` in (+1) or out (-1). */
export function zoomStep(z: Zoom, g: Geometry, dir: 1 | -1, anchor?: Point): Zoom {
  return zoomAt(z, g, dir > 0 ? ZOOM_STEP : 1 / ZOOM_STEP, anchor);
}

/** 100% around `anchor`: the image point under the cursor stays under it,
 * the same rule as every zoom step (docs/ux-spec.md, mouse section). No
 * anchor zooms around the view center. */
export function zoomActual(z0: Zoom, g: Geometry, anchor?: Point): Zoom {
  const z = clampPan(z0, g);
  const s0 = effectiveScale(z, g);
  const a = centered(g, anchor);
  return clampPan({ mode: 'actual', scale: 1, x: a.x - (a.x - z.x) / s0, y: a.y - (a.y - z.y) / s0 }, g);
}

/** Double-click / `zoom_toggle`: fit -> a closer look at the anchor, else
 * back to fit. Closer is 100% for an image fitted below that; an enlarged
 * small image is already past 100%, so it doubles instead of shrinking. */
export function toggleZoom(z: Zoom, g: Geometry, anchor?: Point): Zoom {
  if (z.mode !== 'fit') return clampPan(FIT, g);
  const fit = fitScale(g.img, g.view, g.dpr, g.fit);
  return fit < 1 ? zoomActual(z, g, anchor) : zoomAt(z, g, 2, anchor);
}

/** Drag / `pan_by`, clamped. */
export function panBy(z0: Zoom, g: Geometry, dx: number, dy: number): Zoom {
  const z = clampPan(z0, g);
  return clampPan({ ...z, x: z.x + dx, y: z.y + dy }, g);
}

/** True when the image on screen uses more device px than `loaded` has, so
 * the original is worth fetching. `img` and `loaded` share orientation. */
export function beyondLoaded(scale: number, img: Box, loaded: Box): boolean {
  // 1% slack: rounding in the server's fit must not trigger a 24 MP fetch.
  return scale * img.w > loaded.w * 1.01 || scale * img.h > loaded.h * 1.01;
}

/** The server fits inside the box and never upscales, so a result smaller
 * than its box on both axes is the whole image; so is one at least as big
 * as `/meta` says, or one requested without a box. */
export function isWholeImage(loaded: Box, box: Box | null, meta: Box | null): boolean {
  if (!box || (meta && loaded.w >= meta.w && loaded.h >= meta.h)) return true;
  return loaded.w < box.w - 1 && loaded.h < box.h - 1;
}

export const sameBox = (a: Box | null, b: Box | null): boolean =>
  a === b || (!!a && !!b && a.w === b.w && a.h === b.h);

/** Whether the needed request box grew past the one already asked for. 2%
 * slack: below that the blur is invisible, and boxes guessed from a
 * transcode's natural size jitter by a pixel, which must not loop. */
export function grew(need: Box, have: Box): boolean {
  return need.w > have.w * 1.02 || need.h > have.h * 1.02;
}

/** What a loaded but partial image needs now (`img` and `loaded` unrotated).
 * Fit mode: a bigger box once the stage or DPR grew, never for a shrink.
 * Zoomed: the original, once the screen shows more device px than loaded. */
export function resolutionNeed(
  fit: boolean, scale: number, img: Box, loaded: Box, need: Box, asked: Box,
): Box | 'original' | null {
  if (!fit) return beyondLoaded(scale, img, loaded) ? 'original' : null;
  return grew(need, asked) ? need : null;
}

/** CSS transform for an element whose CSS size is the unrotated image size
 * in image px and whose transform-origin is 0 0 (style.css). Read right to
 * left: center the element on the origin, scale to device px / dpr, rotate,
 * then move to the view center plus pan. */
export function cssTransform(z: Zoom, g: Geometry, quarterTurns: number): string {
  const c = clampPan(z, g);
  const raw = rotated(g.img, quarterTurns);
  const tx = g.view.w / 2 + c.x;
  const ty = g.view.h / 2 + c.y;
  const deg = (((quarterTurns % 4) + 4) % 4) * 90;
  return `translate(${tx}px, ${ty}px) rotate(${deg}deg) scale(${c.scale / g.dpr}) translate(${-raw.w / 2}px, ${-raw.h / 2}px)`;
}
