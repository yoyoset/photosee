import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HttpImageSource, type ImageItem, type Listing } from '../src/api';
import { itemsOf } from '../src/browser-items';
import { OVERLAY_HIDE_MS, WHEEL_COOLDOWN_MS } from '../src/constants';
import type { Ctx } from '../src/ctx';
import { SingleView } from '../src/single';
import {
  SingleLoader, WheelStepper, displaySize, neighbours, paneBox, type LoadImage,
} from '../src/single-load';
import { EscStack, Store } from '../src/state';

const img = (n: string): ImageItem => ({ name: `${n}.jpg`, path: `/p/${n}.jpg`, fmt: 'jpeg', size: 1, mtime_ms: 0 });
const LISTING: Listing = {
  dir: '/p', parent: null, dirs: [{ name: 'sub', path: '/p/sub' }], images: ['a', 'b', 'c', 'd', 'e'].map(img),
};

describe('single: pure helpers', () => {
  it('requests the pane in device px, rounded up to 64', () => {
    expect(paneBox(800, 600, 1)).toEqual({ w: 832, h: 640 });
    expect(paneBox(800, 600, 2)).toEqual({ w: 1600, h: 1216 });
    expect(paneBox(801, 600, 1)).toEqual(paneBox(830, 600, 1)); // small resizes keep the URL
    expect(paneBox(10, 10, 1)).toEqual({ w: 64, h: 64 });
  });

  it('fits the pane but never upscales beyond 1 device px per image px', () => {
    const pane = { w: 800, h: 600 };
    expect(displaySize({ w: 4000, h: 3000 }, 1, pane)).toEqual({ w: 800, h: 600 }); // fit
    expect(displaySize({ w: 400, h: 300 }, 1, pane)).toEqual({ w: 400, h: 300 }); // small stays small
    expect(displaySize({ w: 400, h: 300 }, 2, pane)).toEqual({ w: 200, h: 150 }); // 1 device px = 0.5 css px
    expect(displaySize({ w: 3000, h: 500 }, 1, pane).w).toBe(800);
    expect(displaySize({ w: 3000, h: 500 }, 1, pane).h).toBeCloseTo(133.33, 1);
    expect(displaySize({ w: 0, h: 0 }, 1, pane)).toEqual({ w: 0, h: 0 });
  });

  it('steps the wheel once per 100 units, then waits out a cool-down', () => {
    const w = new WheelStepper();
    expect(w.push(40, 0)).toBe(0);
    expect(w.push(40, 10)).toBe(0);
    expect(w.push(40, 20)).toBe(1);
    expect(w.push(120, 100)).toBe(0); // cooling down
    expect(w.push(120, 20 + WHEEL_COOLDOWN_MS)).toBe(1);
    expect(w.push(-100, 20 + 2 * WHEEL_COOLDOWN_MS + 1)).toBe(-1);
  });

  it('prefetches the next two and the previous one, inside the list', () => {
    expect(neighbours(2, 10)).toEqual([3, 4, 1]);
    expect(neighbours(0, 10)).toEqual([1, 2]);
    expect(neighbours(9, 10)).toEqual([8]);
    expect(neighbours(0, 1)).toEqual([]);
  });
});

describe('single: loader', () => {
  const box = { w: 832, h: 640 };

  it('reuses a prefetched URL for the real request instead of downloading twice', async () => {
    const load = vi.fn<LoadImage>(async () => new Image());
    const l = new SingleLoader(new HttpImageSource(), load);
    l.prefetch(['/p/b.jpg'], box);
    const url = l.urlFor('/p/b.jpg', box);
    expect(url).toContain('prio=prefetch');
    await l.fetch(url);
    expect(load).toHaveBeenCalledTimes(1);
    expect(l.urlFor('/p/zzz.jpg', box)).toContain('prio=current'); // never prefetched
  });

  it('keeps a bounded cache and forgets failures', async () => {
    let n = 0;
    const load = vi.fn<LoadImage>(() => (++n === 1 ? Promise.reject(new Error('x')) : Promise.resolve(new Image())));
    const l = new SingleLoader(new HttpImageSource(), load);
    await l.fetch('/u/0').catch(() => {});
    await Promise.resolve();
    await l.fetch('/u/0'); // retried: the failure was not cached
    expect(load).toHaveBeenCalledTimes(2);
    for (let i = 1; i < 12; i++) await l.fetch(`/u/${i}`);
    await l.fetch('/u/0');
    expect(load).toHaveBeenCalledTimes(2 + 11 + 1); // /u/0 was evicted, so it loads again
  });
});

interface Pending { url: string; resolve(w?: number, h?: number): void; reject(): void }

function rig() {
  const store = new Store();
  store.set({ listing: LISTING, index: 0 });
  const ctx = { store, src: new HttpImageSource(), esc: new EscStack(), dispatch: vi.fn() } as unknown as Ctx;
  const pending: Pending[] = [];
  const load: LoadImage = (url) => new Promise((res, rej) => {
    const im = new Image();
    pending.push({
      url,
      resolve: (w = 4000, h = 3000) => {
        Object.defineProperty(im, 'naturalWidth', { value: w });
        Object.defineProperty(im, 'naturalHeight', { value: h });
        res(im);
      },
      reject: () => rej(new Error('bad')),
    });
  });
  const hooks = { select: vi.fn(), open: vi.fn(), menu: vi.fn(), step: vi.fn() };
  const view = new SingleView(hooks, { ctx, dpr: () => 1, load });
  document.body.append(view.el);
  view.setItems(itemsOf(LISTING));
  const urls = (): string[] => pending.map((p) => decodeURIComponent(p.url));
  // The first request for a file, whatever its priority: a prefetched copy is the one that gets shown.
  const find = (name: string): Pending | undefined =>
    pending.find((p) => decodeURIComponent(p.url).includes(`/p/${name}.jpg`));
  const flush = async (): Promise<void> => { for (let i = 0; i < 10; i++) await Promise.resolve(); };
  return { view, hooks, pending, urls, find, flush };
}

let r: ReturnType<typeof rig> | null = null;
afterEach(() => { r?.view.dispose(); r = null; document.body.replaceChildren(); vi.useRealTimers(); });
const shownWidth = (): string | undefined => document.querySelector<HTMLImageElement>('.sv-img')?.style.width;

describe('single: showing images', () => {
  it('requests the selected image at the pane box x dpr', () => {
    r = rig();
    r.view.setSelected(1); // item 0 is the folder
    expect(r.urls()).toEqual(['/api/v1/img?p=/p/a.jpg&w=832&h=640&prio=current']);
  });

  it('never shows a folder: nothing is requested for one', () => {
    r = rig();
    r.view.setSelected(0);
    expect(r.pending).toHaveLength(0);
  });

  it('shows the image without upscaling, then prefetches next 2 and previous 1', async () => {
    r = rig();
    r.view.setSelected(3); // c
    r.find('c')?.resolve(400, 300);
    await r.flush();
    expect(shownWidth()).toBe('400px'); // small image: 1 device px per image px, not stretched
    expect(document.querySelector<HTMLImageElement>('.sv-img')?.style.height).toBe('300px');
    const pre = r.urls().filter((u) => u.includes('prio=prefetch')).map((u) => /\/p\/(\w)\.jpg/.exec(u)?.[1]);
    expect(pre).toEqual(['d', 'e', 'b']);
  });

  it('a stale response never replaces a newer selection, in either arrival order', async () => {
    r = rig();
    r.view.setSelected(1); // a
    r.view.setSelected(2); // b
    r.find('b')?.resolve(100, 100);
    await r.flush();
    r.find('a')?.resolve(200, 200); // a arrives late
    await r.flush();
    expect(shownWidth()).toBe('100px'); // still b

    r.view.setSelected(3); // c
    r.find('c')?.resolve(300, 300);
    await r.flush();
    r.view.setSelected(4); // d, slow
    r.view.setSelected(5); // e
    r.find('d')?.resolve(111, 111);
    await r.flush();
    expect(shownWidth()).toBe('300px'); // d is stale, c stays on screen
    r.find('e')?.resolve(500, 500);
    await r.flush();
    expect(shownWidth()).toBe('500px');
  });

});

describe('single: prefetch and failure', () => {
  it('uses the already prefetched copy when moving on, without a second request', async () => {
    r = rig();
    r.view.setSelected(1);
    r.find('a')?.resolve();
    await r.flush();
    r.find('b')?.resolve();
    await r.flush();
    const before = r.pending.length;
    r.view.setSelected(2); // b
    await r.flush();
    expect(r.pending.slice(before).some((p) => decodeURIComponent(p.url).includes('/p/b.jpg'))).toBe(false);
    expect(document.querySelector('.sv.loading')).toBeNull();
  });

  it('a failed image shows a placeholder instead of the old picture', async () => {
    r = rig();
    r.view.setSelected(1);
    r.find('a')?.resolve();
    await r.flush();
    r.view.setSelected(2);
    r.find('b')?.reject();
    await r.flush();
    expect(document.querySelector('.sv.bad')).not.toBeNull();
    expect(document.querySelector('.sv-img')).toBeNull();
  });
});

describe('single: pointer', () => {
  beforeEach(() => vi.useFakeTimers());
  const strip = (): HTMLElement => document.querySelector('.sv-strip') as HTMLElement;

  it('the filmstrip hides after OVERLAY_HIDE_MS of stillness and returns on pointer movement', () => {
    r = rig();
    vi.advanceTimersByTime(OVERLAY_HIDE_MS - 1);
    expect(strip().classList.contains('hide')).toBe(false);
    vi.advanceTimersByTime(2);
    expect(strip().classList.contains('hide')).toBe(true);
    r.view.el.dispatchEvent(new Event('pointermove'));
    expect(strip().classList.contains('hide')).toBe(false);
    vi.advanceTimersByTime(OVERLAY_HIDE_MS + 5);
    expect(strip().classList.contains('hide')).toBe(true);
  });

  it('stays while hovered, hides after the pointer leaves', () => {
    r = rig();
    strip().dispatchEvent(new Event('pointerenter'));
    vi.advanceTimersByTime(OVERLAY_HIDE_MS * 5);
    expect(strip().classList.contains('hide')).toBe(false);
    strip().dispatchEvent(new Event('pointerleave'));
    vi.advanceTimersByTime(OVERLAY_HIDE_MS + 5);
    expect(strip().classList.contains('hide')).toBe(true);
  });

});

describe('single: strip and wheel', () => {
  beforeEach(() => vi.useFakeTimers());
  const strip = (): HTMLElement => document.querySelector('.sv-strip') as HTMLElement;

  it('moving between images shows the strip again', () => {
    r = rig();
    vi.advanceTimersByTime(OVERLAY_HIDE_MS + 5);
    r.view.setSelected(2);
    expect(strip().classList.contains('hide')).toBe(false);
  });

  it('the wheel steps one image per notch of 100 and ignores Ctrl+wheel; double-click opens', () => {
    r = rig();
    r.view.setSelected(2);
    const view = r.view;
    const wheel = (deltaY: number, ctrlKey = false): WheelEvent => {
      const e = new WheelEvent('wheel', { deltaY, ctrlKey, cancelable: true, bubbles: true });
      view.el.dispatchEvent(e);
      return e;
    };
    expect(wheel(100).defaultPrevented).toBe(true);
    expect(r.hooks.step).toHaveBeenLastCalledWith(1);
    vi.advanceTimersByTime(WHEEL_COOLDOWN_MS + 1);
    wheel(-100);
    expect(r.hooks.step).toHaveBeenLastCalledWith(-1);
    expect(r.hooks.step).toHaveBeenCalledTimes(2);
    vi.advanceTimersByTime(WHEEL_COOLDOWN_MS + 1);
    expect(wheel(300, true).defaultPrevented).toBe(false);
    expect(r.hooks.step).toHaveBeenCalledTimes(2);
    view.el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(r.hooks.open).toHaveBeenCalledWith(2);
  });

  it('a finger: one tap on the picture opens the viewer, a tap on the filmstrip does not', () => {
    r = rig();
    const tap = (el: Element): void => {
      el.dispatchEvent(new PointerEvent('pointerdown', { pointerType: 'touch', bubbles: true }));
      el.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    };
    tap(r.view.el.querySelector('.sv-stage') as Element);
    expect(r.hooks.open).toHaveBeenCalledTimes(1); // with the selected index, as a double-click
    // A fresh view: two taps within DOUBLE_TAP_MS count as one (touch-tap.ts).
    r.view.dispose();
    document.body.replaceChildren();
    r = rig();
    tap(r.view.el.querySelector('.sv-strip') as Element);
    expect(r.hooks.open).not.toHaveBeenCalled();
  });
});
