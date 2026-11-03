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
import type { ViewState } from './state';

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
  /** Allow fit to exceed 100%. Off for real images (small ones stay 1:1). */
  upscale: boolean;
}

export const FIT: Zoom = { mode: 'fit', scale: 1, x: 0, y: 0 };

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

/** Scale that fits `img` in `view` (CSS px); capped at 1 unless `upscale`.
 * A panorama fits its short axis instead, so it fills the view across and
 * pans along its length. */
export function fitScale(img: Box, view: Box, dpr: number, upscale: boolean): number {
  if (!valid(img) || !valid(view) || !(dpr > 0)) return 1;
  const sx = (view.w * dpr) / img.w;
  const sy = (view.h * dpr) / img.h;
  // Not max(sx, sy): a 3.5:1 image in a 4:1 window would then overflow
  // vertically although it fits whole. The short axis is the image's own.
  const s = isPanorama(img) ? (img.w > img.h ? sy : sx) : Math.min(sx, sy);
  return upscale ? s : Math.min(s, 1);
}

/** Device-px box for /img. `img` null = size not known yet: then the screen
 * box is right, because the server fits inside it and never upscales. A
 * panorama's long axis is grown so that its short axis fills the view. */
export function requestBox(img: Box | null, view: Box, dpr: number): Box {
  const base = { w: ceilPx(view.w * dpr), h: ceilPx(view.h * dpr) };
  if (!img || !isPanorama(img)) return base;
  const s = fitScale(img, view, dpr, true);
  return { w: Math.max(base.w, ceilPx(img.w * s)), h: Math.max(base.h, ceilPx(img.h * s)) };
}

/** The scale actually on screen for a zoom state. */
export function effectiveScale(z: Zoom, g: Geometry): number {
  if (z.mode === 'fit') return fitScale(g.img, g.view, g.dpr, g.upscale);
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
export function zoomAt(z: Zoom, g: Geometry, factor: number, anchor?: Point): Zoom {
  const fit = fitScale(g.img, g.view, g.dpr, g.upscale);
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

/** 100%, with the image point under `anchor` moved to the view center (the
 * brief's "centered on the anchor"). No anchor keeps the current center. */
export function zoomActual(z: Zoom, g: Geometry, anchor?: Point): Zoom {
  const s0 = effectiveScale(z, g);
  const a = centered(g, anchor);
  return clampPan({ mode: 'actual', scale: 1, x: -(a.x - z.x) / s0, y: -(a.y - z.y) / s0 }, g);
}

/** Double-click / `zoom_toggle`: fit -> 100% at the anchor; anything else -> fit. */
export function toggleZoom(z: Zoom, g: Geometry, anchor?: Point): Zoom {
  return z.mode === 'fit' ? zoomActual(z, g, anchor) : clampPan(FIT, g);
}

/** Drag / `pan_by`, clamped. */
export function panBy(z: Zoom, g: Geometry, dx: number, dy: number): Zoom {
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

/** Whether the needed request box grew past the one already asked for. 2%
 * slack: below that the blur is invisible, and boxes guessed from a
 * transcode's natural size jitter by a pixel, which must not loop. */
export function grew(need: Box, have: Box): boolean {
  return need.w > have.w * 1.02 || need.h > have.h * 1.02;
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
