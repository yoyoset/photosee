// What the single-image view needs that is not DOM: the box to request, the
// display size, wheel stepping, and a small cache of loaded/prefetched images.

import type { ImageSource } from './api';
import { SINGLE_AHEAD, SINGLE_BEHIND, SINGLE_BOX_STEP_PX, SINGLE_CACHE } from './browser-consts';
import { WHEEL_COOLDOWN_MS, WHEEL_PAGE_DELTA } from './constants';

export interface Box { w: number; h: number }
export type LoadImage = (url: string) => Promise<HTMLImageElement>;

const up = (px: number): number => Math.max(SINGLE_BOX_STEP_PX, Math.ceil(px / SINGLE_BOX_STEP_PX) * SINGLE_BOX_STEP_PX);

/** Device-pixel box for a pane of `w` x `h` CSS px, rounded up to a step. */
export const paneBox = (w: number, h: number, dpr: number): Box => ({
  w: up(w * Math.max(1, dpr)), h: up(h * Math.max(1, dpr)),
});

/** CSS size to show an image of `nat` device px in a `pane` of CSS px: fit,
 * but never more than one device pixel per image pixel (no upscaling). */
export function displaySize(nat: Box, dpr: number, pane: Box): Box {
  if (nat.w <= 0 || nat.h <= 0) return { w: 0, h: 0 };
  const s = Math.min(1 / Math.max(1, dpr), pane.w / nat.w, pane.h / nat.h);
  return { w: nat.w * s, h: nat.h * s };
}

/** Wheel notches -> one image step per WHEEL_PAGE_DELTA, then a cool-down
 * (same feel as the viewer: a trackpad flick turns one page, not ten). */
export class WheelStepper {
  private acc = 0;
  private until = 0;

  push(dy: number, now: number): -1 | 0 | 1 {
    if (now < this.until) return 0;
    this.acc += dy;
    if (Math.abs(this.acc) < WHEEL_PAGE_DELTA) return 0;
    const d = this.acc > 0 ? 1 : -1;
    this.acc = 0;
    this.until = now + WHEEL_COOLDOWN_MS;
    return d;
  }
}

/** Indexes worth prefetching around `i` among `count` items: next 2, previous 1. */
export function neighbours(i: number, count: number): number[] {
  const out: number[] = [];
  for (let k = 1; k <= SINGLE_AHEAD; k++) out.push(i + k);
  for (let k = 1; k <= SINGLE_BEHIND; k++) out.push(i - k);
  return out.filter((j) => j >= 0 && j < count);
}

export const loadImage: LoadImage = (url) => new Promise((resolve, reject) => {
  const im = new Image();
  im.onload = () => resolve(im);
  im.onerror = () => reject(new Error('image failed'));
  im.decoding = 'async';
  im.src = url;
});

/** Loads through a tiny cache. A neighbour is requested with prio 'prefetch'
 * and, when it becomes the current image, that same URL is reused: a second,
 * differently-spelled URL would download it again. */
export class SingleLoader {
  private readonly cache = new Map<string, Promise<HTMLImageElement>>();

  constructor(private readonly src: ImageSource, private readonly load: LoadImage = loadImage) {}

  urlFor(path: string, box: Box): string {
    const pre = this.src.imageUrl(path, box, 'prefetch');
    return this.cache.has(pre) ? pre : this.src.imageUrl(path, box);
  }

  fetch(url: string): Promise<HTMLImageElement> {
    let p = this.cache.get(url);
    if (!p) {
      p = this.load(url);
      const mine = p;
      p.catch(() => { if (this.cache.get(url) === mine) this.cache.delete(url); }); // a failure is retried next time
      this.cache.set(url, p);
      for (const k of this.cache.keys()) {
        if (this.cache.size <= SINGLE_CACHE) break;
        this.cache.delete(k); // oldest first
      }
    }
    return p;
  }

  prefetch(paths: readonly string[], box: Box): void {
    for (const path of paths) {
      const url = this.src.imageUrl(path, box, 'prefetch');
      if (!this.cache.has(url)) void this.fetch(url).catch(() => {});
    }
  }

  clear(): void { this.cache.clear(); }
}
