// Ways on without a keyboard (2026-11-06): past the last image a second
// "next" opens the next folder, and the fit button cycles whole / width / height.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HINT_TOAST_MS } from '../src/constants';
import { t } from '../src/i18n';
import { flush, listingOf, rig, toasts } from './controller-rig';

beforeEach(() => { document.body.replaceChildren(); });
afterEach(() => { vi.useRealTimers(); });

describe('past the end', () => {
  // Phones have no Ctrl+→: "tap next again" is the way on (user, 2026-11-06).
  it('next past the last image hints once; next again while the hint is up opens the next folder', async () => {
    vi.useFakeTimers();
    const r = rig({ count: 2 });
    r.store.set({ index: 1 });
    r.api.nav.mockResolvedValue('/r/b');
    r.api.list.mockResolvedValue(listingOf('/r/b', 4));
    r.ctl.dispatch({ type: 'next' });
    await flush();
    expect(r.api.nav).not.toHaveBeenCalled();
    expect(toasts()).toEqual([t('hint.last')]);
    r.ctl.dispatch({ type: 'next' });
    await flush();
    expect(r.api.nav).toHaveBeenCalledWith('/r/a', '/r', 'next');
    expect(r.store.get().listing?.dir).toBe('/r/b');
  });

  it('prev at the first image twice goes to the previous folder; the hint expiring disarms it', async () => {
    vi.useFakeTimers();
    const r = rig({ count: 2 });
    r.api.nav.mockResolvedValue(null);
    r.ctl.dispatch({ type: 'prev' });
    vi.advanceTimersByTime(HINT_TOAST_MS + 1);
    r.ctl.dispatch({ type: 'prev' }); // the first hint is gone: this only hints again
    await flush();
    expect(r.api.nav).not.toHaveBeenCalled();
    r.ctl.dispatch({ type: 'prev' });
    await flush();
    expect(r.api.nav).toHaveBeenCalledWith('/r/a', '/r', 'prev');
  });

});

describe('past the end: what never goes on', () => {
  it('a held arrow key (scrub) or a playing slideshow never chains into the next folder', async () => {
    const r = rig({ count: 2 });
    r.store.set({ index: 1, scrubbing: true });
    for (let i = 0; i < 5; i++) r.ctl.dispatch({ type: 'next' });
    r.store.set({ scrubbing: false, slideshow: 'playing' });
    for (let i = 0; i < 3; i++) r.ctl.dispatch({ type: 'next' });
    await flush();
    expect(r.api.nav).not.toHaveBeenCalled();
  });

  it('a page turn in between disarms it', async () => {
    const r = rig({ count: 2 });
    r.store.set({ index: 1 });
    r.ctl.dispatch({ type: 'next' });
    r.ctl.dispatch({ type: 'prev' });
    r.ctl.dispatch({ type: 'next' });
    await flush();
    expect(r.api.nav).not.toHaveBeenCalled();
  });

  it('in an empty folder the first next goes on', async () => {
    const r = rig({ count: 0 });
    r.api.nav.mockResolvedValue(null);
    r.ctl.dispatch({ type: 'next' });
    await flush();
    expect(r.api.nav).toHaveBeenCalledWith('/r/a', '/r', 'next');
  });
});

describe('fit modes', () => {
  it('the fit button steps whole -> width -> height -> whole, naming each in a toast', () => {
    const r = rig();
    const seen: string[] = [];
    for (let i = 0; i < 3; i++) {
      r.ctl.dispatch({ type: 'fit_cycle' });
      seen.push(r.store.get().fit);
    }
    expect(seen).toEqual(['width', 'height', 'contain']);
    expect(toasts()).toEqual([t('fit.contain')]); // toasts replace each other
    expect(r.store.get().zoom.mode).toBe('fit');
  });

  it('when zoomed it first returns to the fit already chosen', () => {
    const r = rig();
    r.store.set({ fit: 'width', zoom: { mode: 'actual', scale: 1, x: 0, y: 0 } });
    r.ctl.dispatch({ type: 'fit_cycle' });
    expect([r.store.get().zoom.mode, r.store.get().fit]).toEqual(['fit', 'width']);
  });

  it('a page turn keeps the fit mode', () => {
    const r = rig();
    r.store.set({ fit: 'height' });
    r.ctl.dispatch({ type: 'next' });
    expect(r.store.get().fit).toBe('height');
  });
});
