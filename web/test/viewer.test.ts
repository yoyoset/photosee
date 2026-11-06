import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, HttpImageSource, type Api, type Listing, type Meta, type Session } from '../src/api';
import type { Ctx } from '../src/ctx';
import { renderErrorCard } from '../src/errors';
import { EscStack, Store, initialState } from '../src/state';
import { mountViewer, type ViewerEnv } from '../src/viewer';
import type { Box } from '../src/zoom';

const removeCard = vi.fn();
vi.mock('../src/errors', () => ({ renderErrorCard: vi.fn(() => removeCard) }));

const enc = (i: number): string => encodeURIComponent(`/d/p${i}.jpg`);
const img = (i: number, w = 1000, h = 800, prio = 'current'): string => `/api/v1/img?p=${enc(i)}&w=${w}&h=${h}&prio=${prio}`;
const orig = (i: number): string => `/api/v1/img?p=${enc(i)}&prio=current`;
const thumb = (i: number): string => `/api/v1/thumb?p=${enc(i)}&s=256`;
const flush = async (): Promise<void> => { for (let i = 0; i < 10; i++) await Promise.resolve(); };

function fakeImg(w: number, h: number): HTMLImageElement {
  const el = document.createElement('img');
  Object.defineProperty(el, 'naturalWidth', { value: w });
  Object.defineProperty(el, 'naturalHeight', { value: h });
  return el;
}

function listingOf(n: number): Listing {
  const images = Array.from({ length: n }, (_, i) => ({
    name: `p${i}.jpg`, path: `/d/p${i}.jpg`, fmt: 'jpeg' as const, size: 1, mtime_ms: 0,
  }));
  return { dir: '/d', parent: null, images, dirs: [] };
}

function harness(o: { count?: number; dpr?: number; scrubbing?: boolean } = {}) {
  const loads = new Map<string, { ok: (i: HTMLImageElement) => void; fail: (e: Error) => void }>();
  const calls: string[] = [];
  const loader = (url: string): Promise<HTMLImageElement> => new Promise((ok, fail) => {
    calls.push(url);
    loads.set(url, { ok, fail });
  });
  const metas = new Map<string, (m: Meta) => void>();
  const api = { meta: (p: string) => new Promise<Meta>((r) => metas.set(p, r)) } as unknown as Api;
  let dpr = o.dpr ?? 1;
  let view: Box = { w: 1000, h: 800 };
  let changed = (): void => {};
  let drag = (on: boolean): void => void on;
  const motion = { reduced: false };
  const env: ViewerEnv = {
    dpr: () => dpr, view: () => view, now: () => 0, reducedMotion: () => motion.reduced,
    watch: (cb) => ((changed = cb), () => {}), watchDrag: (cb) => ((drag = cb), () => {}),
  };
  const store = new Store({ ...initialState, listing: listingOf(o.count ?? 5), scrubbing: o.scrubbing ?? false });
  const session: Session = { api_version: 1, mode: 'desktop', platform: 'windows', start: null, roots: [] };
  const ctx: Ctx = { store, api, src: new HttpImageSource(), esc: new EscStack(), session, dispatch: () => {} };
  const stage = document.createElement('div');
  const shown: number[] = [];
  const failed: [number, string][] = [];
  const fetchError = vi.fn(() => Promise.resolve(new ApiError('corrupt', 'truncated')));
  const v = mountViewer(stage, ctx, {
    loader, env, fetchError, onShown: (i) => shown.push(i), onFailed: (i, e) => failed.push([i, e.code]),
  });
  const get = (url: string) => {
    const l = loads.get(url);
    if (!l) throw new Error(`never requested: ${url}\n${calls.join('\n')}`);
    return l;
  };
  return {
    v, store, stage, calls, shown, failed, fetchError, el: v.element,
    resolve: async (url: string, w = 1000, h = 750) => { get(url).ok(fakeImg(w, h)); await flush(); },
    reject: async (url: string) => { get(url).fail(new Error('EncodingError')); await flush(); },
    meta: async (i: number, width: number, height: number) => {
      metas.get(`/d/p${i}.jpg`)?.({ width, height } as Meta);
      await flush();
    },
    setDpr: (d: number) => { dpr = d; changed(); },
    drag: (on: boolean) => drag(on),
    motion,
    setView: (b: Box) => { view = b; changed(); },
  };
}

afterEach(() => {
  vi.mocked(renderErrorCard).mockClear();
  removeCard.mockClear();
  vi.useRealTimers();
});

describe('progressive display', () => {
  it('shows the thumbnail, then the full image, then prefetches the window', async () => {
    const h = harness();
    expect(h.calls).toEqual([thumb(0), img(0)]);
    expect(h.el.style.visibility).toBe('hidden');
    await h.resolve(thumb(0), 256, 192);
    expect(h.el.getAttribute('src')).toBe(thumb(0));
    expect(h.v.geometry()?.fit).toBe('contain'); // the thumbnail stands in, scaled to fit
    await h.resolve(img(0));
    expect(h.el.getAttribute('src')).toBe(img(0));
    expect(h.shown).toEqual([0]);
    expect(h.calls.slice(2)).toEqual([img(1, 1000, 800, 'prefetch'), img(2, 1000, 800, 'prefetch')]);
  });
  it('sizes the element from /meta and transforms it to fit', async () => {
    const h = harness();
    await h.resolve(img(0));
    await h.meta(0, 4000, 3000);
    expect(h.v.geometry()).toEqual({ img: { w: 4000, h: 3000 }, view: { w: 1000, h: 800 }, dpr: 1, fit: 'contain' });
    expect([h.el.style.width, h.el.style.height]).toEqual(['4000px', '3000px']);
    expect(h.el.style.transform).toContain('scale(0.25)');
  });
  it('swaps a prefetched image in at once, without a new request', async () => {
    const h = harness();
    await h.resolve(img(0));
    await h.resolve(img(1, 1000, 800, 'prefetch'));
    const before = h.calls.length;
    h.store.set({ index: 1 });
    expect(h.el.getAttribute('src')).toBe(img(1, 1000, 800, 'prefetch'));
    expect(h.shown).toEqual([0, 1]);
    expect(h.calls.slice(before).filter((u) => u.includes(enc(1)))).toEqual([]);
  });
  it('reuses an in-flight prefetch instead of asking twice', async () => {
    const h = harness();
    await h.resolve(img(0));
    h.store.set({ index: 1 });
    expect(h.calls).not.toContain(img(1));
    await h.resolve(img(1, 1000, 800, 'prefetch'));
    expect(h.el.getAttribute('src')).toBe(img(1, 1000, 800, 'prefetch'));
  });
});

describe('stale responses', () => {
  it('an old full image arriving late never replaces the current one', async () => {
    const h = harness();
    h.store.set({ index: 3 });
    await h.resolve(img(3));
    await h.resolve(img(0));
    await h.resolve(thumb(0), 256, 192);
    expect(h.el.getAttribute('src')).toBe(img(3));
    expect(h.shown).toEqual([3]);
  });
  it('the newest index wins whatever order responses come in', async () => {
    const h = harness();
    h.store.set({ index: 3 });
    h.store.set({ index: 4 });
    await h.resolve(img(4));
    await h.resolve(img(3));
    await h.reject(img(0));
    expect(h.el.getAttribute('src')).toBe(img(4));
    expect(h.failed).toEqual([]);
    expect(h.fetchError).not.toHaveBeenCalled();
  });
  it('a smaller request finishing after a bigger one is dropped', async () => {
    const h = harness();
    await h.resolve(img(0));
    h.setDpr(1.5);
    h.setDpr(2);
    await h.resolve(img(0, 2000, 1600), 2000, 1500);
    await h.resolve(img(0, 1500, 1200), 1500, 1125);
    expect(h.el.getAttribute('src')).toBe(img(0, 2000, 1600));
  });
});

describe('failure', () => {
  it('the error card replaces the thumbnail and the slideshow is told', async () => {
    const h = harness();
    await h.resolve(thumb(0), 256, 192);
    expect(h.el.getAttribute('src')).toBe(thumb(0));
    await h.reject(img(0));
    expect(h.fetchError).toHaveBeenCalledWith(img(0));
    expect(h.el.hasAttribute('src')).toBe(false);
    expect(h.el.style.visibility).toBe('hidden');
    const [host, err, name] = vi.mocked(renderErrorCard).mock.calls[0] ?? [];
    expect([host, err?.code, name]).toEqual([h.stage, 'corrupt', 'p0.jpg']);
    expect(h.failed).toEqual([[0, 'corrupt']]);
    expect(h.v.geometry()).toBeNull();
  });
  it('a thumbnail arriving after the failure stays off screen; the next image clears the card', async () => {
    const h = harness();
    await h.reject(img(0));
    await h.resolve(thumb(0), 256, 192);
    expect(h.el.hasAttribute('src')).toBe(false);
    expect(removeCard).not.toHaveBeenCalled();
    h.store.set({ index: 1 });
    expect(removeCard).toHaveBeenCalledTimes(1);
  });
  it('retry removes the card and loads the same index again', async () => {
    const h = harness();
    await h.reject(img(0));
    const retry = vi.mocked(renderErrorCard).mock.calls[0]?.[3];
    retry?.();
    expect(removeCard).toHaveBeenCalledTimes(1);
    expect(h.calls.filter((u) => u === img(0))).toHaveLength(2);
    await h.resolve(img(0));
    expect([h.el.getAttribute('src'), h.shown]).toEqual([img(0), [0]]);
  });
});

describe('scrubbing', () => {
  it('shows thumbnails only, then requests the full image on release', async () => {
    const h = harness({ scrubbing: true });
    h.store.set({ index: 1 });
    h.store.set({ index: 2 });
    expect(h.calls).toEqual([thumb(0), thumb(1), thumb(2)]);
    await h.resolve(thumb(1), 256, 192);
    expect(h.el.hasAttribute('src')).toBe(false); // stale thumb
    await h.resolve(thumb(2), 256, 192);
    expect(h.el.getAttribute('src')).toBe(thumb(2));
    h.store.set({ scrubbing: false });
    expect(h.calls.at(-1)).toBe(img(2));
    await h.resolve(img(2));
    expect(h.el.getAttribute('src')).toBe(img(2));
  });
});

describe('resolution changes', () => {
  it('re-requests when the DPR grows, never when it shrinks', async () => {
    const h = harness();
    await h.resolve(img(0));
    h.setDpr(1.5);
    expect(h.calls).toContain(img(0, 1500, 1200));
    await h.resolve(img(0, 1500, 1200), 1500, 1125);
    const n = h.calls.length;
    h.setDpr(1);
    h.setDpr(1.5);
    expect(h.calls.length).toBe(n);
  });
  it('re-requests when the stage grows, not for a shrink or the whole image', async () => {
    const h = harness();
    await h.resolve(img(0), 200, 150); // smaller than the box: it is the original
    h.setView({ w: 2000, h: 1600 });
    expect(h.calls.filter((u) => u.includes(enc(0)) && u.includes('/img'))).toEqual([img(0)]);
    const g = harness();
    await g.resolve(img(0));
    g.setView({ w: 500, h: 400 });
    g.setView({ w: 1600, h: 1000 });
    expect(g.calls.at(-1)).toBe(img(0, 1600, 1000));
  });
  it('zooming past the loaded pixels loads the original once, keeping the scaled one meanwhile', async () => {
    const h = harness();
    await h.resolve(img(0));
    await h.meta(0, 4000, 3000);
    h.store.set({ zoom: { mode: 'custom', scale: 0.3125, x: 0, y: 0 } });
    expect(h.calls).toContain(orig(0));
    h.store.set({ zoom: { mode: 'custom', scale: 1, x: 0, y: 0 } });
    expect(h.calls.filter((u) => u === orig(0))).toHaveLength(1);
    expect(h.el.getAttribute('src')).toBe(img(0));
    await h.resolve(orig(0), 4000, 3000);
    expect(h.el.getAttribute('src')).toBe(orig(0));
    expect(h.el.style.width).toBe('4000px');
  });
});

describe('drag feedback (K)', () => {
  it('follows --drag-y and fades the backdrop with --pull', async () => {
    const h = harness();
    await h.resolve(img(0));
    expect(h.el.style.transform.startsWith('translate(0px, var(--drag-y, 0px)) translate(')).toBe(true);
    expect(h.stage.style.backgroundColor).toBe('rgb(0 0 0 / calc(1 - var(--pull, 0)))');
  });
  it('snaps back with a short transition only after a drag on the same image', async () => {
    vi.useFakeTimers();
    const h = harness();
    await h.resolve(img(0));
    h.drag(true);
    expect(h.el.style.transition).toBe('');
    h.drag(false);
    expect(h.el.style.transition).toBe('transform 150ms ease-out');
    expect(h.stage.style.transition).toBe('background-color 150ms ease-out');
    vi.advanceTimersByTime(150);
    expect([h.el.style.transition, h.stage.style.transition]).toEqual(['', '']);
  });
  it('no snap when the swipe turned the page, or under reduced motion', async () => {
    const h = harness();
    await h.resolve(img(0));
    h.drag(true);
    h.store.set({ index: 1 });
    h.drag(false);
    expect(h.el.style.transition).toBe('');
    h.motion.reduced = true;
    h.drag(true);
    h.drag(false);
    expect(h.el.style.transition).toBe('');
  });
});

describe('dispose', () => {
  it('removes its elements and stops reacting', async () => {
    const h = harness();
    expect(h.stage.children).toHaveLength(2);
    h.v.dispose();
    expect(h.stage.children).toHaveLength(0);
    h.store.set({ index: 2 });
    expect(h.calls).toEqual([thumb(0), img(0)]);
  });
});
