import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Api, ImageItem, Listing, Session, Settings } from '../src/api';
import type { Ctx } from '../src/ctx';
import { setLang, t } from '../src/i18n';
import { FADE_CLASS, mountSlideshow, type SlideshowHooks } from '../src/slideshow-ui';
import { EscStack, Store, initialState } from '../src/state';

const SETTINGS: Settings = {
  interval_s: 5, on_end: 'next_folder', shuffle: false,
  transition: 'none', start_fullscreen: true, wheel: 'navigate',
};
function listing(n: number): Listing {
  const images = Array.from({ length: n }, (_, i) => ({ name: `${i}.jpg` }) as ImageItem);
  return { dir: 'd', parent: null, images, dirs: [] };
}

function setup(over: Partial<Settings> = {}, n = 3, hooks: Partial<SlideshowHooks> = {}) {
  const store = new Store({ ...initialState, settings: { ...SETTINGS, ...over }, listing: listing(n), index: 0 });
  const dispatch = vi.fn((a: { type: string; index?: number }) => {
    if (a.type === 'goto') store.set({ index: a.index ?? 0 }); // the controller would do this
  });
  const ctx = { store, esc: new EscStack(), dispatch, api: {} as Api, src: {}, session: {} as Session } as unknown as Ctx;
  const loadNextFolder = vi.fn();
  const stage = document.body.appendChild(document.createElement('div'));
  const ui = mountSlideshow(ctx, { loadNextFolder, stage, ...hooks });
  const gotos = (): number[] => dispatch.mock.calls.filter(([a]) => a.type === 'goto').map(([a]) => a.index ?? -1);
  return { ctx, store, ui, dispatch, loadNextFolder, stage, gotos };
}

beforeEach(() => {
  setLang('en');
  document.body.replaceChildren();
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe('slideshow timer', () => {
  it('starts the timer at once (the current image is on screen) and mirrors the phase', () => {
    const { ui, store, gotos } = setup();
    ui.start();
    expect(store.get().slideshow).toBe('playing');
    vi.advanceTimersByTime(4999);
    expect(gotos()).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(gotos()).toEqual([1]);
  });

  it('a tick waits for "image shown": a slow image keeps its full interval', () => {
    const { ui, gotos } = setup();
    ui.start();
    vi.advanceTimersByTime(5000);
    expect(gotos()).toEqual([1]);
    vi.advanceTimersByTime(60_000); // slow NAS: image 1 still loading, nothing may advance
    expect(gotos()).toEqual([1]);
    ui.imageShown();
    vi.advanceTimersByTime(4999);
    expect(gotos()).toEqual([1]);
    vi.advanceTimersByTime(1);
    expect(gotos()).toEqual([1, 2]);
  });

  it('reads the interval from settings each time', () => {
    const { ui, store, gotos } = setup({ interval_s: 2 });
    ui.start();
    vi.advanceTimersByTime(2000);
    expect(gotos()).toEqual([1]);
    store.set({ settings: { ...SETTINGS, interval_s: 10 } });
    ui.imageShown();
    vi.advanceTimersByTime(9999);
    expect(gotos()).toEqual([1]);
    vi.advanceTimersByTime(1);
    expect(gotos()).toEqual([1, 2]);
  });

});

describe('slideshow pause and navigation', () => {
  it('pause stops ticks, resume restarts a full interval', () => {
    const { ui, store, gotos } = setup();
    ui.start();
    vi.advanceTimersByTime(3000);
    ui.toggle();
    expect(store.get().slideshow).toBe('paused');
    vi.advanceTimersByTime(60_000);
    expect(gotos()).toEqual([]);
    ui.toggle();
    expect(store.get().slideshow).toBe('playing');
    vi.advanceTimersByTime(4999);
    expect(gotos()).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(gotos()).toEqual([1]);
  });

  it('pausing while an image loads does not start a timer until it is shown', () => {
    const { ui, gotos } = setup();
    ui.start();
    vi.advanceTimersByTime(5000);
    ui.pause();
    ui.resume(); // still loading image 1
    vi.advanceTimersByTime(60_000);
    expect(gotos()).toEqual([1]);
    ui.imageShown();
    vi.advanceTimersByTime(5000);
    expect(gotos()).toEqual([1, 2]);
  });

  it('manual navigation restarts the interval and continues from there', () => {
    const { ui, store, gotos } = setup({}, 5);
    ui.start();
    vi.advanceTimersByTime(4000);
    store.set({ index: 3 }); // user pressed End-ish
    ui.imageShown();
    vi.advanceTimersByTime(4999);
    expect(gotos()).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(gotos()).toEqual([4]);
  });

});

describe('slideshow failures', () => {
  it('a failed image is skipped right away', () => {
    const { ui, gotos } = setup();
    ui.start();
    vi.advanceTimersByTime(5000);
    ui.imageFailed();
    expect(gotos()).toEqual([1]); // not yet: deferred one task
    vi.advanceTimersByTime(0);
    expect(gotos()).toEqual([1, 2]);
  });

  it('stops instead of spinning when every image fails', () => {
    const { ui, store, gotos } = setup({ on_end: 'loop' }, 3);
    ui.start();
    for (let i = 0; i < 10 && store.get().slideshow === 'playing'; i++) {
      ui.imageFailed();
      vi.advanceTimersByTime(0);
    }
    expect(store.get().slideshow).toBe('off');
    expect(gotos().length).toBeLessThanOrEqual(3);
  });
});

describe('stopping and the end', () => {
  it('Esc stops a running slideshow, then yields to the next layer', () => {
    const { ui, ctx, store, gotos } = setup();
    expect(ctx.esc.handle()).toBeNull();
    ui.start();
    expect(ctx.esc.handle()).toBe('slideshow');
    expect(store.get().slideshow).toBe('off');
    vi.advanceTimersByTime(60_000);
    expect(gotos()).toEqual([]);
    expect(ctx.esc.handle()).toBeNull();
  });

  it('on_end=stop shows the ended toast', () => {
    const { ui, store } = setup({ on_end: 'stop' }, 2);
    ui.start();
    vi.advanceTimersByTime(5000);
    ui.imageShown();
    vi.advanceTimersByTime(5000);
    expect(store.get().slideshow).toBe('off');
    expect(document.body.querySelector('.ps-toast')?.textContent).toBe(t('slideshow.ended'));
  });

  it('on_end=loop wraps to the first image', () => {
    const { ui, gotos } = setup({ on_end: 'loop' }, 2);
    ui.start();
    vi.advanceTimersByTime(5000);
    ui.imageShown();
    vi.advanceTimersByTime(5000);
    expect(gotos()).toEqual([1, 0]);
  });

  it('starting on an empty folder stays off with the ended toast', () => {
    const { ui, store } = setup({}, 0);
    ui.start();
    expect(store.get().slideshow).toBe('off');
    expect(document.body.querySelector('.ps-toast')?.textContent).toBe(t('slideshow.ended'));
  });
});

describe('next folder', () => {
  function atLastImage() {
    const s = setup({ on_end: 'next_folder' }, 2);
    s.ui.start();
    vi.advanceTimersByTime(5000);
    s.ui.imageShown();
    vi.advanceTimersByTime(5000);
    return s;
  }

  it('asks the controller for the next folder and runs no timer meanwhile', () => {
    const { loadNextFolder, store, gotos } = atLastImage();
    expect(loadNextFolder).toHaveBeenCalledOnce();
    expect(store.get().slideshow).toBe('playing');
    vi.advanceTimersByTime(60_000);
    expect(loadNextFolder).toHaveBeenCalledOnce();
    expect(gotos()).toEqual([1]);
  });

  it('folderLoaded toasts the folder name and plays on from its first image', () => {
    const { ui, store, gotos } = atLastImage();
    store.set({ listing: listing(3), index: 0 }); // the controller switched folders
    ui.imageShown(); // the viewer showed #0 while we were still waiting
    ui.folderLoaded(3, 'Trip 2024');
    expect(document.body.querySelector('.ps-toast')?.textContent).toBe('Trip 2024');
    vi.advanceTimersByTime(1500);
    expect(document.body.querySelector('.ps-toast')).toBeNull();
    vi.advanceTimersByTime(5000);
    expect(gotos()).toEqual([1, 1]);
  });

  it('folderLoaded waits for the viewer when #0 has not been reported yet', () => {
    const { ui, store, gotos } = atLastImage();
    store.set({ listing: listing(3), index: 0 });
    ui.folderLoaded(3, 'B');
    vi.advanceTimersByTime(60_000); // image 0 of the new folder still loading
    expect(gotos()).toEqual([1]);
    ui.imageShown();
    vi.advanceTimersByTime(5000);
    expect(gotos()).toEqual([1, 1]);
  });

  it('noNextFolder stops with the ended toast', () => {
    const { ui, store } = atLastImage();
    ui.noNextFolder();
    expect(store.get().slideshow).toBe('off');
    expect(document.body.querySelector('.ps-toast')?.textContent).toBe(t('slideshow.ended'));
  });
});

describe('real state machine, end to end', () => {
  it('start, shown, tick, end -> loadNextFolder -> noNextFolder -> stopped + toast', () => {
    const { ui, store, loadNextFolder, gotos } = setup({ on_end: 'next_folder' }, 2);
    ui.start();
    expect(store.get().slideshow).toBe('playing');
    vi.advanceTimersByTime(5000);
    expect(gotos()).toEqual([1]);
    ui.imageShown();
    vi.advanceTimersByTime(5000);
    expect(loadNextFolder).toHaveBeenCalledOnce();
    expect(store.get().slideshow).toBe('playing');
    ui.noNextFolder();
    expect(store.get().slideshow).toBe('off');
    expect(document.body.querySelector('.ps-toast')?.textContent).toBe(t('slideshow.ended'));
    vi.advanceTimersByTime(3000); // HINT_TOAST_MS
    expect(document.body.querySelector('.ps-toast')).toBeNull();
  });

  it('shuffle keeps the current image first and visits each image once', () => {
    const seq = [0.1, 0.9, 0.4, 0.7];
    let i = 0;
    const rand = (): number => seq[i++ % seq.length] ?? 0;
    const { ui, store, gotos } = setup({ shuffle: true, on_end: 'stop' }, 5, { rand });
    store.set({ index: 2 });
    ui.start();
    for (let n = 0; n < 4; n++) {
      vi.advanceTimersByTime(5000);
      ui.imageShown();
    }
    const visited = [2, ...gotos()];
    expect([...visited].sort()).toEqual([0, 1, 2, 3, 4]);
  });
});

describe('fade class', () => {
  it('is on the stage only while running with transition=fade', () => {
    const { ui, stage } = setup({ transition: 'fade' });
    expect(stage.classList.contains(FADE_CLASS)).toBe(false);
    ui.start();
    expect(stage.classList.contains(FADE_CLASS)).toBe(true);
    expect(stage.style.getPropertyValue('--ps-fade-ms')).toBe('300ms');
    ui.stop();
    expect(stage.classList.contains(FADE_CLASS)).toBe(false);
  });

  it('is never set for transition=none or under reduced motion', () => {
    const a = setup({ transition: 'none' });
    a.ui.start();
    expect(a.stage.classList.contains(FADE_CLASS)).toBe(false);
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    const b = setup({ transition: 'fade' });
    b.ui.start();
    expect(b.stage.classList.contains(FADE_CLASS)).toBe(false);
    vi.unstubAllGlobals();
  });
});
