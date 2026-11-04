import { afterEach, describe, expect, it, vi } from 'vitest';
import { LOADING_DELAY_MS as D, LOADING_MIN_VISIBLE_MS as M } from '../src/constants';
import {
  ImageCache, ImageLru, LoadingBar, SnapBack, defaultEnv, fetchImageError, loadingBarNextChange, loadingBarVisible, prefetchWindow,
} from '../src/prefetch';

function fakeImg(w: number, h: number): HTMLImageElement {
  const img = document.createElement('img');
  Object.defineProperty(img, 'naturalWidth', { value: w });
  Object.defineProperty(img, 'naturalHeight', { value: h });
  return img;
}

describe('prefetchWindow', () => {
  it('is nearest first, ahead before behind', () => {
    expect(prefetchWindow(5, 10, 2, 1)).toEqual([6, 4, 7]);
    expect(prefetchWindow(5, 10, 1, 3)).toEqual([6, 4, 3, 2]);
  });
  it('clips at both ends and never includes the index', () => {
    expect(prefetchWindow(0, 10, 2, 1)).toEqual([1, 2]);
    expect(prefetchWindow(9, 10, 2, 1)).toEqual([8]);
    expect(prefetchWindow(8, 10, 2, 1)).toEqual([9, 7]);
    expect(prefetchWindow(0, 1, 2, 1)).toEqual([]);
    expect(prefetchWindow(0, 0, 2, 1)).toEqual([]);
  });
});

describe('ImageLru', () => {
  it('evicts least recently used beyond the budget', () => {
    const lru = new ImageLru<string>(100);
    lru.put('a', 'A', 40);
    lru.put('b', 'B', 40);
    expect(lru.get('a')).toBe('A'); // a is now most recent
    lru.put('c', 'C', 40);
    expect(lru.keys()).toEqual(['a', 'c']);
    expect(lru.bytes).toBe(80);
    lru.put('d', 'D', 30);
    expect(lru.keys()).toEqual(['c', 'd']);
    lru.put('e', 'E', 25);
    expect(lru.keys()).toEqual(['c', 'd', 'e']);
    expect(lru.bytes).toBe(95);
  });
  it('does not cache an entry larger than the whole budget', () => {
    const lru = new ImageLru<string>(100);
    lru.put('a', 'A', 60);
    lru.put('huge', 'H', 101);
    expect(lru.has('huge')).toBe(false);
    expect(lru.keys()).toEqual(['a']);
  });
  it('replaces an existing key and recounts its bytes', () => {
    const lru = new ImageLru<string>(100);
    lru.put('a', 'A', 60);
    lru.put('a', 'A2', 30);
    expect(lru.get('a')).toBe('A2');
    expect(lru.bytes).toBe(30);
    lru.put('a', 'A3', 500);
    expect(lru.has('a')).toBe(false);
    expect(lru.bytes).toBe(0);
  });
});

describe('ImageCache', () => {
  it('dedupes in-flight loads and stores w*h*4 bytes', async () => {
    const loader = vi.fn((url: string) => Promise.resolve(fakeImg(url === 'big' ? 100 : 10, 10)));
    const cache = new ImageCache(new ImageLru(1000), loader);
    const [a, b] = await Promise.all([cache.load('x'), cache.load('x')]);
    expect(a).toBe(b);
    expect(loader).toHaveBeenCalledTimes(1);
    expect(cache.lru.bytes).toBe(400);
    await cache.load('x');
    expect(loader).toHaveBeenCalledTimes(1);
    await cache.load('big'); // 40 000 bytes > budget: shown, not cached
    expect(cache.lru.has('big')).toBe(false);
  });
  it('forgets a failed load so it can be retried', async () => {
    let fail = true;
    const loader = vi.fn(() => (fail ? Promise.reject(new Error('x')) : Promise.resolve(fakeImg(1, 1))));
    const cache = new ImageCache(new ImageLru(1000), loader);
    await expect(cache.load('u')).rejects.toThrow('x');
    expect(cache.pending('u')).toBeUndefined();
    fail = false;
    cache.prefetch('u');
    await cache.pending('u');
    expect(cache.cached('u')).toBeDefined();
    cache.prefetch('u');
    expect(loader).toHaveBeenCalledTimes(2);
  });
});

describe('loading bar timing', () => {
  it('never shows for a load that finished within the delay', () => {
    for (const now of [0, D - 1, D, D + M, 10_000]) expect(loadingBarVisible(0, D - 1, now)).toBe(false);
    expect(loadingBarNextChange(0, D - 1, D - 1)).toBeNull();
  });
  it('shows after the delay while loading', () => {
    expect(loadingBarVisible(100, null, 100 + D - 1)).toBe(false);
    expect(loadingBarVisible(100, null, 100 + D)).toBe(true);
    expect(loadingBarNextChange(100, null, 100)).toBe(100 + D);
    expect(loadingBarNextChange(100, null, 100 + D + 5)).toBeNull();
  });
  it('stays at least the minimum once shown', () => {
    const done = D + 50;
    expect(loadingBarVisible(0, done, done + 1)).toBe(true);
    expect(loadingBarVisible(0, done, D + M - 1)).toBe(true);
    expect(loadingBarVisible(0, done, D + M)).toBe(false);
    expect(loadingBarNextChange(0, done, done)).toBe(D + M);
  });
  it('hides as soon as a long load ends', () => {
    expect(loadingBarVisible(0, 5000, 4999)).toBe(true);
    expect(loadingBarVisible(0, 5000, 5000)).toBe(false);
    expect(loadingBarNextChange(0, 5000, 5000)).toBeNull();
  });
});

describe('LoadingBar', () => {
  afterEach(() => vi.useRealTimers());
  it('drives the element from injected time', () => {
    vi.useFakeTimers();
    let t = 0;
    const advance = (ms: number): void => { t += ms; vi.advanceTimersByTime(ms); };
    const bar = new LoadingBar(() => t);
    const el = bar.el;
    bar.start();
    advance(D - 1);
    expect(el.hidden).toBe(true);
    advance(1);
    expect(el.hidden).toBe(false);
    bar.start(); // paging on while loading: same bar, no restart
    advance(10);
    bar.end();
    expect(el.hidden).toBe(false);
    advance(M - 11);
    expect(el.hidden).toBe(false);
    advance(1);
    expect(el.hidden).toBe(true);
    bar.start();
    bar.end();
    advance(D + M);
    expect(el.hidden).toBe(true);
    bar.dispose();
  });
});

describe('fetchImageError', () => {
  afterEach(() => vi.unstubAllGlobals());
  it('reads the JSON error body', async () => {
    const body = { code: 'missing_codec', codec: 'hevc_ext', message: 'WIC: component not found' };
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve(new Response(JSON.stringify(body), { status: 415 }))));
    const e = await fetchImageError('/api/v1/img?p=a');
    expect([e.code, e.codec, e.detail]).toEqual(['missing_codec', 'hevc_ext', 'WIC: component not found']);
  });
  it('maps a 200 that would not decode to corrupt, and a throw to network', async () => {
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve(new Response('xx', { status: 200 }))));
    expect((await fetchImageError('/u')).code).toBe('corrupt');
    vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new TypeError('Failed to fetch'))));
    expect((await fetchImageError('/u')).code).toBe('network');
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve(new Response('<html>', { status: 502 }))));
    const e = await fetchImageError('/u');
    expect([e.code, e.detail]).toEqual(['io', 'HTTP 502']);
  });
});

describe('stage environment', () => {
  it('reports data-dragging appearing and disappearing on the stage', async () => {
    const stage = document.createElement('div');
    const seen: boolean[] = [];
    const off = defaultEnv(stage).watchDrag((on) => seen.push(on));
    stage.dataset.dragging = 'x';
    await Promise.resolve();
    stage.dataset.dragging = 'y'; // same drag, new axis: not a new event
    await Promise.resolve();
    delete stage.dataset.dragging;
    await Promise.resolve();
    off();
    stage.dataset.dragging = 'x';
    await Promise.resolve();
    expect(seen).toEqual([true, false]);
  });
  it('SnapBack sets and clears its transitions; cancel clears at once', () => {
    vi.useFakeTimers();
    const a = document.createElement('div');
    const snap = new SnapBack([a, 'transform']);
    snap.run();
    expect(a.style.transition).toBe('transform 150ms ease-out');
    snap.cancel();
    expect(a.style.transition).toBe('');
    snap.run();
    vi.advanceTimersByTime(150);
    expect(a.style.transition).toBe('');
    vi.useRealTimers();
  });
});
