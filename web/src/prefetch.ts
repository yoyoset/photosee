// What the viewer loads and when: prefetch window, decoded-image LRU, the
// default loader, loading-bar timing, and the stage environment (DPR and
// size) that triggers re-requests. Package I. The DOM-touching parts are
// injectable (`ImageLoader`, `ViewerEnv`) so tests can drive the viewer.

import { ApiError, type ErrorCode } from './api';
import { LOADING_DELAY_MS, LOADING_MIN_VISIBLE_MS } from './constants';
import type { Box } from './zoom';

/** Indices to prefetch around `index`, nearest first, ahead before behind,
 * clipped to [0, count), never `index` itself. */
export function prefetchWindow(index: number, count: number, ahead: number, behind: number): number[] {
  const out: number[] = [];
  for (let d = 1; d <= Math.max(ahead, behind); d++) {
    if (d <= ahead && index + d < count && index + d >= 0) out.push(index + d);
    if (d <= behind && index - d >= 0 && index - d < count) out.push(index - d);
  }
  return out;
}

/** Decoded images keyed by URL. `bytes` ~ w*h*4. Holding the decoded object
 * is what keeps Chromium's memory-cache entry (and its decoded bitmap) alive
 * for the visible <img> to reuse. */
export class ImageLru<T = HTMLImageElement> {
  // Map iteration order is insertion order: the first key is the least
  // recently used, because `get` and `put` re-insert at the end.
  private readonly map = new Map<string, { v: T; bytes: number }>();
  private total = 0;

  constructor(readonly budgetBytes: number) {}

  get bytes(): number {
    return this.total;
  }

  keys(): string[] {
    return [...this.map.keys()];
  }

  has(key: string): boolean {
    return this.map.has(key);
  }

  get(key: string): T | undefined {
    const e = this.map.get(key);
    if (!e) return undefined;
    this.map.delete(key);
    this.map.set(key, e);
    return e.v;
  }

  put(key: string, v: T, bytes: number): void {
    this.delete(key);
    const b = Number.isFinite(bytes) && bytes > 0 ? bytes : 0;
    // Caching it would evict everything else and then itself.
    if (b > this.budgetBytes) return;
    this.map.set(key, { v, bytes: b });
    this.total += b;
    for (const [k, e] of this.map) {
      if (this.total <= this.budgetBytes) break;
      this.map.delete(k);
      this.total -= e.bytes;
    }
  }

  delete(key: string): void {
    const e = this.map.get(key);
    if (!e) return;
    this.map.delete(key);
    this.total -= e.bytes;
  }
}

/** Loads a URL to a decoded image. Injected so tests can drive the viewer
 * (images never load in jsdom). Rejects when the image cannot be shown. */
export type ImageLoader = (url: string) => Promise<HTMLImageElement>;

/** Default loader: a detached `new Image()` + `decode()`. That is how the
 * M0-T spike prefetched; swapping the visible <img> to the same URL then
 * painted in 31.8 ms median (spikes/transport/REPORT.md §3). */
export async function decodeImage(url: string): Promise<HTMLImageElement> {
  const img = new Image();
  img.decoding = 'async';
  img.src = url;
  await img.decode();
  return img;
}

/** An <img> error carries no body, so ask once more with fetch to read the
 * server's `{code, codec, message}`. */
export async function fetchImageError(url: string): Promise<ApiError> {
  let res: Response;
  try {
    res = await fetch(url, { credentials: 'same-origin' });
  } catch (e) {
    return new ApiError('network', String(e));
  }
  if (res.ok) {
    // The server sent bytes it thought the webview could decode (passthrough).
    void res.body?.cancel();
    return new ApiError('corrupt', `the webview could not decode it (HTTP ${res.status})`);
  }
  const body = (await res.json().catch(() => ({}))) as {
    code?: ErrorCode; message?: string; codec?: ApiError['codec'];
  };
  return new ApiError(body.code ?? 'io', body.message ?? `HTTP ${res.status}`, body.codec ?? null);
}

/** LRU + in-flight dedupe around an `ImageLoader`. */
export class ImageCache {
  private readonly inflight = new Map<string, Promise<HTMLImageElement>>();

  constructor(readonly lru: ImageLru, private readonly loader: ImageLoader) {}

  cached(url: string): HTMLImageElement | undefined {
    return this.lru.get(url);
  }

  pending(url: string): Promise<HTMLImageElement> | undefined {
    return this.inflight.get(url);
  }

  load(url: string): Promise<HTMLImageElement> {
    const hit = this.lru.get(url);
    if (hit) return Promise.resolve(hit);
    const running = this.inflight.get(url);
    if (running) return running;
    const p = this.loader(url).then(
      (img) => {
        this.inflight.delete(url);
        this.lru.put(url, img, img.naturalWidth * img.naturalHeight * 4);
        return img;
      },
      (e: unknown) => {
        this.inflight.delete(url);
        throw e;
      },
    );
    this.inflight.set(url, p);
    return p;
  }

  /** Fire and forget; failures surface when the user gets there. */
  prefetch(url: string): void {
    if (this.lru.has(url) || this.inflight.has(url)) return;
    this.load(url).catch(() => {});
  }
}

/** Loading-bar visibility at `now` for a load that started at `start` and
 * finished at `done` (null = still loading). It appears only after
 * LOADING_DELAY_MS and, once shown, stays LOADING_MIN_VISIBLE_MS. */
export function loadingBarVisible(start: number, done: number | null, now: number): boolean {
  const shownAt = start + LOADING_DELAY_MS;
  if (now < shownAt) return false;
  if (done === null) return true;
  if (done < shownAt) return false;
  return now < Math.max(done, shownAt + LOADING_MIN_VISIBLE_MS);
}

/** When `loadingBarVisible` next changes, or null when it never will. */
export function loadingBarNextChange(start: number, done: number | null, now: number): number | null {
  const shownAt = start + LOADING_DELAY_MS;
  if (now < shownAt) return done === null ? shownAt : null;
  if (done === null) return null;
  if (done < shownAt) return null;
  const hideAt = Math.max(done, shownAt + LOADING_MIN_VISIBLE_MS);
  return now < hideAt ? hideAt : null;
}

/** The thin top bar. `start` when the user begins waiting for an image, `end`
 * when it is on screen or failed. Paging on while still waiting keeps one
 * continuous bar instead of restarting the delay. */
export class LoadingBar {
  private load: { start: number; done: number | null } | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(readonly el: HTMLElement, private readonly now: () => number) {
    el.hidden = true;
  }

  start(): void {
    if (this.load && this.load.done === null) return;
    this.load = { start: this.now(), done: null };
    this.tick();
  }

  end(): void {
    if (!this.load || this.load.done !== null) return;
    this.load.done = this.now();
    this.tick();
  }

  dispose(): void {
    clearTimeout(this.timer);
  }

  private tick(): void {
    clearTimeout(this.timer);
    const l = this.load;
    const now = this.now();
    this.el.hidden = !l || !loadingBarVisible(l.start, l.done, now);
    const next = l ? loadingBarNextChange(l.start, l.done, now) : null;
    if (next !== null) this.timer = setTimeout(() => this.tick(), next - now);
  }
}

export interface ViewerEnv {
  dpr(): number;
  /** Stage size, CSS px. */
  view(): Box;
  now(): number;
  /** Call `changed` on stage resize or DPR change; returns an unsubscribe. */
  watch(changed: () => void): () => void;
}

function watchResize(stage: HTMLElement, changed: () => void): () => void {
  const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => changed());
  ro?.observe(stage);
  let mq: MediaQueryList | null = null;
  // A resolution query fires once when the DPR leaves its value, so it is
  // re-armed with the new DPR every time (window moved to another monitor).
  const arm = (): void => {
    mq?.removeEventListener('change', fire);
    mq = typeof matchMedia === 'function' ? matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`) : null;
    mq?.addEventListener('change', fire);
  };
  const fire = (): void => { arm(); changed(); };
  arm();
  return () => { ro?.disconnect(); mq?.removeEventListener('change', fire); };
}

export function defaultEnv(stage: HTMLElement): ViewerEnv {
  return {
    dpr: () => window.devicePixelRatio || 1,
    view: () => ({ w: stage.clientWidth || window.innerWidth, h: stage.clientHeight || window.innerHeight }),
    now: () => performance.now(),
    watch: (changed) => watchResize(stage, changed),
  };
}
