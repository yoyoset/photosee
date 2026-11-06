import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Listing } from '../src/api';
import { t } from '../src/i18n';
import { mountSlideshow } from '../src/slideshow-ui';
import { flush, toasts, listingOf, rig } from './controller-rig';

beforeEach(() => { document.body.replaceChildren(); });
afterEach(() => { vi.useRealTimers(); });

describe('page turns', () => {
  it('clamps at both ends and hints at the last image', () => {
    const r = rig({ count: 2 });
    r.ctl.dispatch({ type: 'next' });
    expect(r.store.get().index).toBe(1);
    expect(toasts()).toEqual([]);
    r.ctl.dispatch({ type: 'next' });
    expect(r.store.get().index).toBe(1);
    expect(toasts()).toEqual([t('hint.last')]);
    r.ctl.dispatch({ type: 'first' });
    r.ctl.dispatch({ type: 'prev' });
    expect(r.store.get().index).toBe(0);
    expect(toasts()).toEqual([t('hint.first')]);
  });

  it('goto clamps out-of-range indexes', () => {
    const r = rig({ count: 3 });
    r.ctl.dispatch({ type: 'goto', index: 99 });
    expect(r.store.get().index).toBe(2);
    r.ctl.dispatch({ type: 'goto', index: -4 });
    expect(r.store.get().index).toBe(0);
    r.ctl.dispatch({ type: 'last' });
    expect(r.store.get().index).toBe(2);
  });

  it.each(['next', 'last', 'first'] as const)('%s resets zoom and rotation', (type) => {
    const r = rig({ count: 3 });
    r.store.set({ index: 1, rotation: 3, zoom: { mode: 'custom', scale: 2, x: 5, y: 5 } });
    r.ctl.dispatch({ type });
    const s = r.store.get();
    expect(s.zoom.mode).toBe('fit');
    expect(s.rotation).toBe(0);
  });
});

describe('zoom and rotation', () => {
  it('computes zoom from the viewer geometry and ignores it when there is none', () => {
    const r = rig();
    r.ctl.dispatch({ type: 'zoom_actual' });
    expect(r.store.get().zoom.mode).toBe('actual');
    r.ctl.dispatch({ type: 'zoom_fit' });
    expect(r.store.get().zoom.mode).toBe('fit');
    r.ctl.dispatch({ type: 'zoom_by', factor: 1.25 });
    expect(r.store.get().zoom.mode).toBe('custom');
    r.ctl.dispatch({ type: 'zoom_toggle' });
    expect(r.store.get().zoom.mode).toBe('fit');

    r.geo.value = null;
    r.ctl.dispatch({ type: 'zoom_actual' });
    expect(r.store.get().zoom.mode).toBe('fit');
  });

  it('pans only while zoomed in', () => {
    const r = rig();
    r.ctl.dispatch({ type: 'zoom_actual' });
    r.ctl.dispatch({ type: 'pan_by', dx: 10, dy: 0 });
    expect(r.store.get().zoom.x).not.toBe(0);
  });

  it('rotates in quarter turns, 0..3', () => {
    const r = rig();
    r.ctl.dispatch({ type: 'rotate', quarterTurns: -1 });
    expect(r.store.get().rotation).toBe(3);
    r.ctl.dispatch({ type: 'rotate', quarterTurns: 1 });
    r.ctl.dispatch({ type: 'rotate', quarterTurns: 1 });
    expect(r.store.get().rotation).toBe(1);
  });
});

describe('panels, scrub, fullscreen', () => {
  it('routes panel toggles and scrub', () => {
    const r = rig();
    r.ctl.dispatch({ type: 'toggle_info' });
    r.ctl.dispatch({ type: 'toggle_help' });
    r.ctl.dispatch({ type: 'open_settings' });
    r.ctl.dispatch({ type: 'toggle_slideshow' });
    expect(r.parts.info.toggle).toHaveBeenCalledOnce();
    expect(r.parts.help.toggle).toHaveBeenCalledOnce();
    expect(r.parts.settings.open).toHaveBeenCalledOnce();
    expect(r.slideshow.toggle).toHaveBeenCalledOnce();
    r.ctl.dispatch({ type: 'scrub', active: true });
    expect(r.store.get().scrubbing).toBe(true);
  });

  it('toggles fullscreen through the host and mirrors it in the store', async () => {
    const r = rig();
    r.ctl.dispatch({ type: 'toggle_fullscreen' });
    await flush();
    expect(r.store.get().fullscreen).toBe(true);
    r.ctl.dispatch({ type: 'toggle_fullscreen' });
    await flush();
    expect(r.store.get().fullscreen).toBe(false);
  });

  it('keeps the store honest when the host refuses fullscreen', async () => {
    const r = rig();
    r.host.setFullscreen = () => Promise.reject(new Error('needs a user gesture'));
    r.ctl.dispatch({ type: 'toggle_fullscreen' });
    await flush();
    expect(r.store.get().fullscreen).toBe(false);
  });
});

describe('Esc order', () => {
  it('unwinds panel, zoom, fullscreen, then returns to the browser', async () => {
    const r = rig();
    r.ctl.registerEsc(r.host);
    let panelOpen = true;
    r.ctx.esc.register('panel', () => {
      if (!panelOpen) return false;
      panelOpen = false;
      return true;
    });
    r.store.set({ zoom: { mode: 'actual', scale: 1, x: 0, y: 0 } });
    r.host.fs = true;
    r.store.set({ fullscreen: true });

    r.ctl.dispatch({ type: 'escape' });
    expect(panelOpen).toBe(false);
    expect(r.store.get().zoom.mode).toBe('actual');
    r.ctl.dispatch({ type: 'escape' });
    expect(r.store.get().zoom.mode).toBe('fit');
    expect(r.host.fs).toBe(true);
    r.ctl.dispatch({ type: 'escape' });
    await flush();
    expect([r.host.fs, r.store.get().fullscreen]).toEqual([false, false]);
    expect(r.store.get().mode).toBe('viewer');
    r.ctl.dispatch({ type: 'escape' }); // the bottom of the stack: back to the browser, never close
    expect(r.store.get().mode).toBe('browser');
    expect(r.host.close).not.toHaveBeenCalled();
  });
});

describe('next folder', () => {
  it('navs, lists, lands on index 0 and toasts the folder name', async () => {
    const r = rig();
    r.store.set({ index: 2 });
    r.api.nav.mockResolvedValue('/r/b');
    r.api.list.mockResolvedValue(listingOf('/r/b', 4));
    r.ctl.dispatch({ type: 'next_folder' });
    await flush();
    expect(r.api.nav).toHaveBeenCalledWith('/r/a', '/r', 'next');
    expect(r.api.list).toHaveBeenCalledWith('/r/b');
    expect(r.store.get().index).toBe(0);
    expect(r.store.get().listing?.dir).toBe('/r/b');
    expect(r.store.get().root).toBe('/r');
    expect(toasts()).toEqual(['b']);
  });

  it('prev_folder asks for prev', async () => {
    const r = rig();
    r.api.nav.mockResolvedValue(null);
    r.ctl.dispatch({ type: 'prev_folder' });
    await flush();
    expect(r.api.nav).toHaveBeenCalledWith('/r/a', '/r', 'prev');
  });

  it('null answer shows the no-next-folder hint and keeps the listing', async () => {
    const r = rig();
    r.api.nav.mockResolvedValue(null);
    r.ctl.dispatch({ type: 'next_folder' });
    await flush();
    expect(toasts()).toEqual([t('hint.no_next_folder')]);
    expect(r.store.get().listing?.dir).toBe('/r/a');
    expect(r.api.list).not.toHaveBeenCalled();
  });

  it('is blocked while the slideshow is paused', async () => {
    const r = rig();
    r.store.set({ slideshow: 'paused' });
    r.ctl.dispatch({ type: 'next_folder' });
    await flush();
    expect(r.api.nav).not.toHaveBeenCalled();
  });


});

describe('overlapping folder requests', () => {
  it('a slower older answer cannot overwrite a newer one', async () => {
    const r = rig();
    let slow!: (l: Listing) => void;
    r.api.nav.mockResolvedValueOnce('/r/b').mockResolvedValueOnce('/r/c');
    r.api.list.mockReturnValueOnce(new Promise<Listing>((res) => { slow = res; }))
      .mockResolvedValueOnce(listingOf('/r/c', 2));
    r.ctl.dispatch({ type: 'next_folder' });
    await flush();
    r.ctl.dispatch({ type: 'next_folder' });
    await flush();
    slow(listingOf('/r/b', 9));
    await flush();
    expect(r.store.get().listing?.dir).toBe('/r/c');
  });

  it('tells a playing slideshow how the load went', async () => {
    const r = rig();
    r.store.set({ slideshow: 'playing' });
    r.api.nav.mockResolvedValueOnce('/r/b').mockResolvedValueOnce(null);
    r.api.list.mockResolvedValue(listingOf('/r/b', 4));
    void r.ctl.nav.changeFolder('next', true);
    await flush();
    expect(r.slideshow.folderLoaded).toHaveBeenCalledWith(4, 'b');
    void r.ctl.nav.changeFolder('next', true);
    await flush();
    expect(r.slideshow.noNextFolder).toHaveBeenCalledOnce();
  });
});

describe('slideshow end with next_folder', () => {
  it('loads the next folder and continues from its first image', async () => {
    vi.useFakeTimers();
    const r = rig({ count: 2 });
    const show = mountSlideshow(r.ctx, {
      loadNextFolder: () => void r.ctl.nav.changeFolder('next', true), host: document.body, stage: null,
    });
    r.parts.slideshow = show;
    r.api.nav.mockResolvedValue('/r/b');
    r.api.list.mockResolvedValue(listingOf('/r/b', 3));
    // Starts at 0, not the last image: slideshow.ts begin() leaves pos 0 for
    // a mid-folder start (reported to the orchestrator), which would replay.
    show.start();
    expect(r.store.get().slideshow).toBe('playing');

    await vi.advanceTimersByTimeAsync(5000); // image 0 -> image 1
    expect(r.store.get().index).toBe(1);
    show.imageShown();
    await vi.advanceTimersByTimeAsync(5000); // the last image's interval ends
    expect(r.api.nav).toHaveBeenCalledWith('/r/a', '/r', 'next');
    expect(r.store.get().listing?.dir).toBe('/r/b');
    expect(r.store.get().index).toBe(0);
    expect(r.store.get().slideshow).toBe('playing');

    show.imageShown(); // the viewer reports the new folder's first image
    await vi.advanceTimersByTimeAsync(5000);
    expect(r.store.get().index).toBe(1);
    show.dispose();
  });
});
