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
// Motion: image changes and zoom are instant. The one animation is the
// snap-back after a pull-down that did not close (K's --drag-y / --pull /
// data-dragging on the stage); it is skipped under
// prefers-reduced-motion. The slideshow fade is package L's class on the stage.

import type { ApiError, ImageItem } from './api';
import {
  CACHE_BUDGET_BYTES_DESKTOP, CACHE_BUDGET_BYTES_MOBILE, PREFETCH_AHEAD, PREFETCH_BEHIND, THUMB_EDGE,
} from './constants';
import type { Ctx } from './ctx';
import { renderErrorCard } from './errors';
import {
  ImageCache, ImageLru, LoadingBar, SnapBack, decodeImage, defaultEnv, fetchImageError, prefetchWindow,
  type ImageLoader, type ViewerEnv,
} from './prefetch';
import type { ViewState } from './state';
export type { ViewerEnv } from './prefetch';
import {
  cssTransform, effectiveScale, isWholeImage, requestBox, resolutionNeed, rotated, rotatedFit, sameBox,
  type Box, type Geometry,
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
  meta: Box | null; // display size from /meta
  thumb: Box | null; // natural size of the thumbnail on screen
  full: Full | null;
  /** Box of the newest full request; null = the original. */
  want: Box | null;
  asked: boolean;
  failed: boolean;
}

const natural = (img: HTMLImageElement): Box => ({ w: img.naturalWidth, h: img.naturalHeight });
// K's live pull-down offset goes first, i.e. it applies in screen space on
// top of the fitted transform; it is 0 whenever K is not dragging.
const DRAG = 'translate(0px, var(--drag-y, 0px)) ';

class StageView implements Viewer {
  readonly element = document.createElement('img');
  private readonly bar = new LoadingBar(() => this.env.now());
  private readonly cache: ImageCache;
  private readonly env: ViewerEnv;
  private readonly loader: ImageLoader;
  private readonly offs: (() => void)[] = [];
  private cur: Cur | null = null;
  private removeCard: (() => void) | null = null;
  private readonly snap: SnapBack;
  private dragIndex = -1;

  constructor(private readonly stage: HTMLElement, private readonly ctx: Ctx, private readonly opts: ViewerOptions) {
    const mobile = ctx.session.platform === 'android' || ctx.session.platform === 'ios';
    const budget = opts.budgetBytes ?? (mobile ? CACHE_BUDGET_BYTES_MOBILE : CACHE_BUDGET_BYTES_DESKTOP);
    this.loader = opts.loader ?? decodeImage;
    this.cache = new ImageCache(new ImageLru(budget), this.loader);
    this.env = opts.env ?? defaultEnv(stage);
    this.snap = new SnapBack([this.element, 'transform'], [stage, 'background-color']);
    this.element.style.visibility = 'hidden';
    this.element.alt = '';
    this.element.draggable = false;
    // The pull-to-close backdrop: opaque until K reports pull progress.
    stage.style.backgroundColor = 'rgb(0 0 0 / calc(1 - var(--pull, 0)))';
    stage.append(this.element, this.bar.el);
    this.offs.push(
      ctx.store.subscribe((s, prev) => this.onState(s, prev)),
      this.env.watch(() => this.update()),
      this.env.watchDrag((on) => this.onDrag(on)),
    );
    this.show();
  }

  dispose(): void {
    for (const off of this.offs) off();
    this.bar.dispose();
    this.snap.cancel();
    this.dropCard();
    this.cur = null;
    this.element.remove();
    this.bar.el.remove();
    this.stage.style.backgroundColor = '';
  }

  geometry(): Geometry | null {
    const cur = this.cur;
    const size = cur && !cur.failed ? (cur.meta ?? cur.full?.natural ?? cur.thumb) : null;
    if (!cur || !size) return null;
    const { rotation, fit } = this.ctx.store.get();
    return { img: rotated(size, rotation), view: this.env.view(), dpr: this.env.dpr(), fit };
  }

  private onState(s: ViewState, prev: ViewState): void {
    if (s.listing !== prev.listing || s.index !== prev.index) return this.show();
    const cur = this.cur;
    if (prev.scrubbing && !s.scrubbing && cur && !cur.asked && !cur.failed) this.loadFull(cur, this.neededBox(cur));
    if (s.zoom !== prev.zoom || s.rotation !== prev.rotation || s.fit !== prev.fit) this.update();
  }

  private show(): void {
    const s = this.ctx.store.get();
    const item = s.listing?.images[s.index];
    this.dropCard();
    this.snap.cancel(); // a new page shows in place, never snapping back
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
    const { rotation, fit } = this.ctx.store.get();
    const view = rotated(this.env.view(), rotation);
    return requestBox(cur.meta ?? cur.full?.natural ?? cur.thumb, view, this.env.dpr(), rotatedFit(fit, rotation));
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
    const img = await this.loader(url).catch(() => null); // the full image reports failures
    if (!img || this.cur !== cur || cur.full || cur.failed) return;
    cur.thumb = natural(img);
    this.element.src = url;
    this.element.style.visibility = '';
    this.update();
  }

  private async fetchMeta(cur: Cur): Promise<void> {
    // The image request reports failures; meta only refines the geometry.
    const m = await this.ctx.api.meta(cur.item.path).catch(() => null);
    if (!m || this.cur !== cur || !(m.width > 0 && m.height > 0)) return;
    cur.meta = { w: m.width, h: m.height };
    this.update();
  }

  private async fail(cur: Cur, url: string): Promise<void> {
    // Blank first: the thumbnail must not stay while the error body loads.
    cur.failed = true;
    this.blank();
    const err = await (this.opts.fetchError ?? fetchImageError)(url);
    if (this.cur !== cur) return;
    this.bar.end();
    // Retry re-runs the whole index (thumb, meta, full); L shows the button
    // only for transient errors.
    this.removeCard = renderErrorCard(this.stage, err, cur.item.name, () => {
      if (this.cur === cur) this.show();
    });
    this.opts.onFailed?.(cur.index, err);
    this.prefetchAround(cur);
  }

  private dropCard(): void {
    this.removeCard?.();
    this.removeCard = null;
  }

  /** Snap back only when the drag ended on the same image (no page turn)
   * and motion is allowed; never transition while the finger is down. */
  private onDrag(dragging: boolean): void {
    if (dragging) this.dragIndex = this.cur?.index ?? -1;
    if (dragging || this.cur?.index !== this.dragIndex || this.env.reducedMotion()) this.snap.cancel();
    else this.snap.run();
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
    this.element.style.transform = DRAG + cssTransform(s.zoom, g, s.rotation);
    this.checkResolution(g, s);
  }

  /** Re-request when the stage/DPR grew in fit mode, or fetch the original
   * once when zoomed past what is loaded. Never for a shrink. */
  private checkResolution(g: Geometry, s: ViewState): void {
    const cur = this.cur;
    if (!cur?.full || cur.want === null || s.scrubbing || isWholeImage(cur.full.natural, cur.full.box, cur.meta)) return;
    const shown = effectiveScale(s.zoom, g);
    const r = resolutionNeed(s.zoom.mode === 'fit', shown, rotated(g.img, s.rotation), cur.full.natural, this.neededBox(cur), cur.want);
    if (r) this.loadFull(cur, r === 'original' ? null : r);
  }

  private prefetchAround(cur: Cur): void {
    const s = this.ctx.store.get();
    const images = s.listing?.images ?? [];
    if (s.scrubbing) return;
    // Neighbours are guessed unrotated (a page turn resets the view) at the
    // screen box; a panorama neighbour is re-requested once its size is known.
    const box = requestBox(null, this.env.view(), this.env.dpr(), s.fit);
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
