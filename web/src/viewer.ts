// The image stage: progressive display (thumb -> full), fit/zoom/pan via CSS
// transform on a single <img>, DPR-aware requests, error card on failure.
// Package I. Geometry is in zoom.ts, caching/timing in prefetch.ts.
//
// Swap method: the visible <img> gets `src = url` of an image that a detached
// `new Image()` has already decoded (kept alive in the LRU). This is the
// method M0-T measured: 31.8 ms median swap -> paint for a screen-size image.
// Replacing the element with the decoded Image object was not measured, and
// would break "one <img>" that package L's fade class and K's gestures rely on.
//
// Nothing here animates, so prefers-reduced-motion needs no branch: changes
// are instant, and the optional slideshow fade is package L's class on the
// stage around `Viewer.element`.

import type { ApiError, ImageItem } from './api';
import {
  CACHE_BUDGET_BYTES_DESKTOP, CACHE_BUDGET_BYTES_MOBILE, PREFETCH_AHEAD, PREFETCH_BEHIND, THUMB_EDGE,
} from './constants';
import type { Ctx } from './ctx';
import { renderErrorCard } from './errors';
import {
  ImageCache, ImageLru, LoadingBar, decodeImage, defaultEnv, fetchImageError, prefetchWindow,
  type ImageLoader, type ViewerEnv,
} from './prefetch';
import type { ViewState } from './state';
export type { ViewerEnv } from './prefetch';
import {
  beyondLoaded, cssTransform, effectiveScale, grew, isWholeImage, requestBox, rotated, type Box, type Geometry,
} from './zoom';

export interface ViewerOptions {
  loader?: ImageLoader;
  fetchError?: (url: string) => Promise<ApiError>;
  env?: ViewerEnv;
  budgetBytes?: number;
  /** The full image for `index` is on screen: the slideshow starts its interval. */
  onShown?: (index: number) => void;
  /** It failed and the error card replaced it: the slideshow skips it. This
   * callback is the only failure signal (no DOM event). */
  onFailed?: (index: number, err: ApiError) => void;
}

export interface Viewer {
  /** The single <img>, for package L's slideshow fade. */
  readonly element: HTMLImageElement;
  /** What the zoom helpers in zoom.ts need; null while nothing is shown. */
  geometry(): Geometry | null;
  dispose(): void;
}

interface Full { url: string; natural: Box; box: Box | null }

/** Everything about the image at one index. A new index gets a new object,
 * and every async continuation checks `this.cur === cur` before touching the
 * screen: that identity check is what keeps stale responses off it. */
interface Cur {
  index: number;
  item: ImageItem;
  meta: Box | null;
  thumb: Box | null;
  full: Full | null;
  /** Box of the newest full request; null = the original. */
  want: Box | null;
  asked: boolean;
  failed: boolean;
}

const sameBox = (a: Box | null, b: Box | null): boolean => a === b || (!!a && !!b && a.w === b.w && a.h === b.h);
const natural = (img: HTMLImageElement): Box => ({ w: img.naturalWidth, h: img.naturalHeight });

function makeEl<K extends keyof HTMLElementTagNameMap>(tag: K, css: string): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  el.style.cssText = css;
  return el;
}

class StageView implements Viewer {
  readonly element = makeEl('img', 'visibility:hidden');
  private readonly bar = new LoadingBar(
    makeEl('div', 'position:absolute;left:0;right:0;top:0;height:2px;background:var(--accent);pointer-events:none'),
    () => this.env.now(),
  );
  private readonly errHost = makeEl('div', 'position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);max-width:90%');
  private readonly cache: ImageCache;
  private readonly env: ViewerEnv;
  private readonly loader: ImageLoader;
  private readonly offs: (() => void)[] = [];
  private cur: Cur | null = null;

  constructor(stage: HTMLElement, private readonly ctx: Ctx, private readonly opts: ViewerOptions) {
    const mobile = ctx.session.platform === 'android' || ctx.session.platform === 'ios';
    const budget = opts.budgetBytes ?? (mobile ? CACHE_BUDGET_BYTES_MOBILE : CACHE_BUDGET_BYTES_DESKTOP);
    this.loader = opts.loader ?? decodeImage;
    this.cache = new ImageCache(new ImageLru(budget), this.loader);
    this.env = opts.env ?? defaultEnv(stage);
    this.element.alt = '';
    this.element.draggable = false;
    this.errHost.hidden = true;
    stage.append(this.element, this.errHost, this.bar.el);
    this.offs.push(ctx.store.subscribe((s, prev) => this.onState(s, prev)), this.env.watch(() => this.update()));
    this.show();
  }

  dispose(): void {
    for (const off of this.offs) off();
    this.bar.dispose();
    this.cur = null;
    this.element.remove();
    this.bar.el.remove();
    this.errHost.remove();
  }

  geometry(): Geometry | null {
    const cur = this.cur;
    const size = cur && !cur.failed ? (cur.meta ?? cur.full?.natural ?? cur.thumb) : null;
    if (!cur || !size) return null;
    // Only a bare thumbnail is upscaled: it stands in for a bigger image.
    const upscale = !cur.meta && !cur.full;
    return { img: rotated(size, this.ctx.store.get().rotation), view: this.env.view(), dpr: this.env.dpr(), upscale };
  }

  private onState(s: ViewState, prev: ViewState): void {
    if (s.listing !== prev.listing || s.index !== prev.index) return this.show();
    const cur = this.cur;
    if (prev.scrubbing && !s.scrubbing && cur && !cur.asked && !cur.failed) this.loadFull(cur, this.neededBox(cur));
    if (s.zoom !== prev.zoom || s.rotation !== prev.rotation) this.update();
  }

  private show(): void {
    const s = this.ctx.store.get();
    const item = s.listing?.images[s.index];
    this.errHost.replaceChildren();
    this.errHost.hidden = true;
    const cur: Cur | null = item
      ? { index: s.index, item, meta: null, thumb: null, full: null, want: null, asked: false, failed: false }
      : null;
    this.cur = cur;
    if (!cur) {
      this.blank();
      this.bar.end();
      return;
    }
    void this.fetchMeta(cur);
    const box = this.neededBox(cur);
    if (this.showCached(cur, box)) return;
    this.blank();
    void this.loadThumb(cur);
    if (s.scrubbing) this.bar.end(); // thumbnails only: nothing is loading for the user
    else this.loadFull(cur, box);
  }

  /** A decoded hit (shown before, or prefetched) swaps in at once. */
  private showCached(cur: Cur, box: Box): boolean {
    for (const prio of ['current', 'prefetch'] as const) {
      const url = this.ctx.src.imageUrl(cur.item.path, box, prio);
      const hit = this.cache.cached(url);
      if (!hit) continue;
      cur.asked = true;
      cur.want = box;
      this.display(cur, url, hit, box);
      return true;
    }
    return false;
  }

  /** The device-px box for `cur` now. The server's box is in unrotated
   * image axes, so the view is rotated instead of the image. */
  private neededBox(cur: Cur): Box {
    const view = rotated(this.env.view(), this.ctx.store.get().rotation);
    return requestBox(cur.meta ?? cur.full?.natural ?? cur.thumb, view, this.env.dpr());
  }

  private loadFull(cur: Cur, box: Box | null): void {
    cur.asked = true;
    cur.want = box;
    const pre = box ? this.ctx.src.imageUrl(cur.item.path, box, 'prefetch') : null;
    // A prefetch already on the wire for this box is reused, not raced: a
    // second request would make the server read and resize the file twice.
    const url = pre && this.cache.pending(pre) ? pre : this.ctx.src.imageUrl(cur.item.path, box ?? undefined, 'current');
    if (!cur.full) this.bar.start();
    this.cache.load(url).then(
      // An older, smaller request finishing late must not replace a newer one.
      (img) => { if (this.cur === cur && (!cur.full || sameBox(box, cur.want))) this.display(cur, url, img, box); },
      () => { if (this.cur === cur && !cur.full) void this.fail(cur, url); },
    );
  }

  private display(cur: Cur, url: string, img: HTMLImageElement, box: Box | null): void {
    const first = !cur.full;
    cur.full = { url, natural: natural(img), box };
    this.element.src = url;
    this.element.style.visibility = '';
    this.update();
    if (!first) return;
    this.bar.end();
    this.opts.onShown?.(cur.index);
    this.prefetchAround(cur);
  }

  private async loadThumb(cur: Cur): Promise<void> {
    const url = this.ctx.src.thumbUrl(cur.item.path, THUMB_EDGE);
    let img: HTMLImageElement;
    try {
      img = await this.loader(url);
    } catch {
      return; // the full image reports its own failure
    }
    if (this.cur !== cur || cur.full || cur.failed) return;
    cur.thumb = natural(img);
    this.element.src = url;
    this.element.style.visibility = '';
    this.update();
  }

  private async fetchMeta(cur: Cur): Promise<void> {
    try {
      const m = await this.ctx.api.meta(cur.item.path);
      if (this.cur !== cur || !(m.width > 0 && m.height > 0)) return;
      cur.meta = { w: m.width, h: m.height };
      this.update();
    } catch {
      // The image request reports failures; meta only refines the geometry.
    }
  }

  private async fail(cur: Cur, url: string): Promise<void> {
    // Blank first: the thumbnail must not stay while the error body loads.
    cur.failed = true;
    this.blank();
    const err = await (this.opts.fetchError ?? fetchImageError)(url);
    if (this.cur !== cur) return;
    this.bar.end();
    this.errHost.hidden = false;
    renderErrorCard(this.errHost, err, cur.item.name);
    this.opts.onFailed?.(cur.index, err);
    this.prefetchAround(cur);
  }

  private blank(): void {
    this.element.removeAttribute('src');
    this.element.style.visibility = 'hidden';
  }

  private update(): void {
    const g = this.geometry();
    if (!g) return;
    const s = this.ctx.store.get();
    const raw = rotated(g.img, s.rotation);
    this.element.style.width = `${raw.w}px`;
    this.element.style.height = `${raw.h}px`;
    this.element.style.transform = cssTransform(s.zoom, g, s.rotation);
    this.checkResolution(g, s);
  }

  /** Re-request when the stage/DPR grew in fit mode, or fetch the original
   * once when zoomed past what is loaded. Never for a shrink. */
  private checkResolution(g: Geometry, s: ViewState): void {
    const cur = this.cur;
    if (!cur?.full || cur.want === null || s.scrubbing || isWholeImage(cur.full.natural, cur.full.box, cur.meta)) return;
    if (s.zoom.mode !== 'fit') {
      const raw = rotated(g.img, s.rotation);
      if (beyondLoaded(effectiveScale(s.zoom, g), raw, cur.full.natural)) this.loadFull(cur, null);
      return;
    }
    const need = this.neededBox(cur);
    if (grew(need, cur.want)) this.loadFull(cur, need);
  }

  private prefetchAround(cur: Cur): void {
    const s = this.ctx.store.get();
    const images = s.listing?.images ?? [];
    if (s.scrubbing) return;
    // Neighbours are guessed unrotated (a page turn resets the view) at the
    // screen box; a panorama neighbour is re-requested once its size is known.
    const box = requestBox(null, this.env.view(), this.env.dpr());
    for (const i of prefetchWindow(cur.index, images.length, PREFETCH_AHEAD, PREFETCH_BEHIND)) {
      const it = images[i];
      if (!it || this.cache.lru.has(this.ctx.src.imageUrl(it.path, box, 'current'))) continue;
      this.cache.prefetch(this.ctx.src.imageUrl(it.path, box, 'prefetch'));
    }
  }
}

/** Mount the viewer on `stage` (the #stage element). The stage stays the
 * gesture target; the viewer only adds the <img>, a loading bar and the
 * error-card host. */
export function mountViewer(stage: HTMLElement, ctx: Ctx, opts: ViewerOptions = {}): Viewer {
  return new StageView(stage, ctx, opts);
}
