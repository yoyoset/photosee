import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Api, Format, ImageItem, ImageSource, Session } from '../src/api';
import type { Ctx } from '../src/ctx';
import { THUMB_EDGE } from '../src/constants';
import { STRIDE_PX, centerOffset, mountFilmstrip, visibleRange } from '../src/filmstrip';
import { EscStack, Store, initialState } from '../src/state';

const must = <T>(x: T | undefined): T => {
  if (x === undefined) throw new Error('expected a value');
  return x;
};

function images(n: number): ImageItem[] {
  return Array.from({ length: n }, (_, i) => ({
    name: `p${i}.jpg`, path: `/d/p${i}.jpg`, fmt: 'jpeg' as Format, size: 1, mtime_ms: 0,
  }));
}

function makeCtx(n: number, index = 0) {
  const store = new Store({
    ...initialState, index, listing: { dir: '/d', parent: null, images: images(n), dirs: [] },
  });
  const dispatch = vi.fn();
  const src: ImageSource = {
    imageUrl: (p) => `img:${p}`, thumbUrl: (p, e) => `thumb:${p}:${e}`,
  };
  const session = { platform: 'windows' } as Session;
  const ctx: Ctx = { store, api: {} as Api, src, esc: new EscStack(), session, dispatch };
  return { ctx, store, dispatch };
}

afterEach(() => { document.body.replaceChildren(); });

describe('visibleRange', () => {
  it('covers the viewport plus overscan, clamped at the start', () => {
    expect(visibleRange(0, 200, 50, 100, 2)).toEqual({ start: 0, end: 6 });
    expect(visibleRange(500, 200, 50, 100, 2)).toEqual({ start: 8, end: 16 });
  });
  it('clamps at the end', () => {
    expect(visibleRange(4900, 200, 50, 100, 3)).toEqual({ start: 95, end: 100 });
  });
  it('treats a strip that is not scrolled past its content as empty, not negative', () => {
    expect(visibleRange(99999, 200, 50, 100, 2)).toEqual({ start: 100, end: 100 });
  });
  it('handles a negative offset (first item centred) and count smaller than the viewport', () => {
    expect(visibleRange(-100, 200, 50, 3, 0)).toEqual({ start: 0, end: 2 });
    expect(visibleRange(0, 1000, 50, 3, 4)).toEqual({ start: 0, end: 3 });
  });
  it('returns an empty range for degenerate input', () => {
    const none = { start: 0, end: 0 };
    expect(visibleRange(0, 200, 50, 0, 2)).toEqual(none);
    expect(visibleRange(0, 0, 50, 10, 2)).toEqual(none);
    expect(visibleRange(0, 200, 0, 10, 2)).toEqual(none);
  });
  it('keeps overscan items when the viewport is partially covered by a cell', () => {
    // 25..225 touches cells 0..4 (cell 4 spans 200..250)
    expect(visibleRange(25, 200, 50, 100, 0)).toEqual({ start: 0, end: 5 });
  });
});

describe('mountFilmstrip', () => {
  it('keeps the DOM small with 10 000 items and follows the index', () => {
    const { ctx, store } = makeCtx(10_000, 5000);
    const host = document.body.appendChild(document.createElement('div'));
    const unmount = mountFilmstrip(host, ctx);
    const count = (): number => host.querySelectorAll('img').length;
    expect(count()).toBeGreaterThan(0);
    expect(count()).toBeLessThan(100);
    store.set({ index: 9000 });
    expect(count()).toBeLessThan(100);
    expect(host.querySelector('[data-i="9000"]')).not.toBeNull();
    expect(host.querySelector('[data-i="5000"]')).toBeNull();
    unmount();
    expect(host.children.length).toBe(0);
  });

  it('builds lazy, async, empty-alt thumbnails from thumbUrl', () => {
    const { ctx } = makeCtx(3);
    const host = document.body.appendChild(document.createElement('div'));
    mountFilmstrip(host, ctx);
    const img = host.querySelector('img');
    expect(img?.getAttribute('loading')).toBe('lazy');
    expect(img?.getAttribute('decoding')).toBe('async');
    expect(img?.getAttribute('alt')).toBe('');
    expect(img?.getAttribute('src')).toBe(`thumb:/d/p0.jpg:${THUMB_EDGE}`);
  });

});

describe('mountFilmstrip: position and input', () => {
  it('highlights and centres the current item', () => {
    const { ctx, store } = makeCtx(50, 10);
    const host = document.body.appendChild(document.createElement('div'));
    mountFilmstrip(host, ctx);
    expect(host.querySelector('.cur')?.getAttribute('data-i')).toBe('10');
    store.set({ index: 11 });
    expect(host.querySelectorAll('.cur').length).toBe(1);
    expect(host.querySelector('.cur')?.getAttribute('data-i')).toBe('11');
    const track = host.querySelector<HTMLElement>('.fs-track');
    const expected = -centerOffset(11, window.innerWidth);
    expect(track?.style.transform).toBe(`translateX(${expected}px)`);
    // the middle of cell 11 lands on the middle of the viewport
    expect(11 * STRIDE_PX + 32 + expected).toBe(window.innerWidth / 2);
  });

  it('dispatches goto when a thumbnail is clicked', () => {
    const { ctx, dispatch } = makeCtx(50, 10);
    const host = document.body.appendChild(document.createElement('div'));
    mountFilmstrip(host, ctx);
    host.querySelector<HTMLElement>('[data-i="12"] img')?.click();
    expect(dispatch).toHaveBeenCalledWith({ type: 'goto', index: 12 });
  });

  it('rebuilds when the listing changes', () => {
    const { ctx, store } = makeCtx(20, 3);
    const host = document.body.appendChild(document.createElement('div'));
    mountFilmstrip(host, ctx);
    store.set({ listing: { dir: '/e', parent: null, dirs: [], images: [{ ...must(images(1)[0]), path: '/e/x.jpg' }] }, index: 0 });
    const srcs = [...host.querySelectorAll('img')].map((i) => i.getAttribute('src'));
    expect(srcs).toEqual([`thumb:/e/x.jpg:${THUMB_EDGE}`]);
  });

  it('marks a failed thumbnail instead of leaving a broken icon', () => {
    const { ctx } = makeCtx(3);
    const host = document.body.appendChild(document.createElement('div'));
    mountFilmstrip(host, ctx);
    host.querySelector('img')?.dispatchEvent(new Event('error'));
    expect(host.querySelector('.fs-cell.bad')).not.toBeNull();
  });
});
