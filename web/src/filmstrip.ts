// Virtualized thumbnail strip, mounted inside the overlay's bottom bar.
// Package J.
//
// Why a transform and not native scrolling: the current item must always sit
// in the centre, so the strip is positioned from the index alone. That makes
// "which items are on screen" pure arithmetic (`visibleRange`) and keeps a
// 10 000-image folder at a few dozen DOM nodes. There is deliberately no
// easing: image changes are instant everywhere else in the viewer.

import { FILMSTRIP_THUMB_PX, THUMB_EDGE } from './constants';
import type { Ctx } from './ctx';
import type { ViewState } from './state';

export interface Range { start: number; end: number }

/** Gap between cells, CSS px. Layout only, not a UX threshold. */
const GAP_PX = 4;
export const STRIDE_PX = FILMSTRIP_THUMB_PX + GAP_PX;
/** Cells rendered beyond each edge so a held key never outruns the DOM. */
const OVERSCAN = 6;

/**
 * Half-open range [start, end) of items intersecting the viewport, widened by
 * `overscan` on each side and clamped to [0, count]. `scrollLeft` may be
 * negative: the first item is centred, so the strip starts with blank space.
 */
export function visibleRange(
  scrollLeft: number, viewportW: number, itemW: number, count: number, overscan: number,
): Range {
  if (count <= 0 || itemW <= 0 || viewportW <= 0) return { start: 0, end: 0 };
  const first = Math.floor(scrollLeft / itemW) - overscan;
  const last = Math.ceil((scrollLeft + viewportW) / itemW) + overscan;
  const start = Math.min(Math.max(0, first), count);
  const end = Math.max(start, Math.min(count, last));
  return { start, end };
}

/** Scroll offset that puts the middle of cell `index` in the middle of the viewport. */
export function centerOffset(index: number, viewportW: number): number {
  return index * STRIDE_PX + FILMSTRIP_THUMB_PX / 2 - viewportW / 2;
}

function makeCell(ctx: Ctx, i: number, path: string): HTMLElement {
  const cell = document.createElement('div');
  cell.className = 'fs-cell';
  cell.dataset['i'] = String(i);
  cell.style.left = `${i * STRIDE_PX}px`;
  const img = document.createElement('img');
  // alt is empty on purpose: the strip is decoration for sighted users; the
  // #live region announces the position and name.
  img.alt = '';
  img.setAttribute('loading', 'lazy');
  img.setAttribute('decoding', 'async');
  img.draggable = false;
  img.src = ctx.src.thumbUrl(path, THUMB_EDGE);
  cell.append(img);
  return cell;
}

class Strip {
  readonly el = document.createElement('div');
  private readonly track = document.createElement('div');
  private readonly cells = new Map<number, HTMLElement>();
  private cur: HTMLElement | null = null;
  private lastListing: ViewState['listing'] = null;

  constructor(private readonly ctx: Ctx) {
    this.el.className = 'fs';
    this.el.setAttribute('aria-hidden', 'true');
    // The CSS sizes the cells from the same constant that places them.
    this.el.style.setProperty('--thumb', `${FILMSTRIP_THUMB_PX}px`);
    this.track.className = 'fs-track';
    this.el.append(this.track);
    this.track.addEventListener('click', (e) => {
      const cell = (e.target as Element).closest<HTMLElement>('.fs-cell');
      if (cell) ctx.dispatch({ type: 'goto', index: Number(cell.dataset['i']) });
    });
    // 'error' does not bubble, so listen in the capture phase once instead of
    // one handler per thumbnail. A broken thumbnail keeps its dark cell.
    this.track.addEventListener('error', (e) => (e.target as Element).closest('.fs-cell')?.classList.add('bad'), true);
  }

  /** Cheap enough to run on every index change: touches only the cells that enter or leave. */
  render = (): void => {
    const { listing, index } = this.ctx.store.get();
    if (listing !== this.lastListing) {
      this.lastListing = listing;
      this.track.replaceChildren();
      this.cells.clear();
      this.cur = null;
    }
    const images = listing?.images ?? [];
    const w = this.el.clientWidth || window.innerWidth;
    const off = centerOffset(index, w);
    this.track.style.width = `${images.length * STRIDE_PX}px`;
    this.track.style.transform = `translateX(${-off}px)`;
    const { start, end } = visibleRange(off, w, STRIDE_PX, images.length, OVERSCAN);
    this.reconcile(start, end);
    this.cur?.classList.remove('cur');
    this.cur = this.cells.get(index) ?? null;
    this.cur?.classList.add('cur');
  };

  private reconcile(start: number, end: number): void {
    for (const [i, el] of this.cells) {
      if (i >= start && i < end) continue;
      el.remove();
      this.cells.delete(i);
    }
    const images = this.ctx.store.get().listing?.images ?? [];
    for (let i = start; i < end; i++) {
      const item = images[i];
      if (!item || this.cells.has(i)) continue;
      const cell = makeCell(this.ctx, i, item.path);
      this.cells.set(i, cell);
      this.track.append(cell);
    }
  }
}

/**
 * Mounts the strip into `host` and keeps it in sync with the store. Returns
 * the unmount function. Width falls back to the window when the host has no
 * layout yet (first paint, jsdom).
 */
export function mountFilmstrip(host: HTMLElement, ctx: Ctx): () => void {
  const strip = new Strip(ctx);
  host.append(strip.el);
  window.addEventListener('resize', strip.render);
  // The browser's single view changes the strip's width without a window resize
  // (tree toggled or dragged), and the centring depends on that width.
  const ro = typeof ResizeObserver === 'function' ? new ResizeObserver(() => strip.render()) : null;
  ro?.observe(strip.el);
  const unsub = ctx.store.subscribe((s, prev) => {
    if (s.listing !== prev.listing || s.index !== prev.index) strip.render();
  });
  strip.render();
  return () => {
    unsub();
    ro?.disconnect();
    window.removeEventListener('resize', strip.render);
    strip.el.remove();
  };
}
