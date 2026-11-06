// Page turns on touch by tapping the sides (the swipe was the phone's own
// Back gesture, 2026-11-06), and pages fitted bigger than the stage.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { W, drag, el, mount, off, seen, store, wheel } from './gesture-rig';

beforeEach(() => { vi.useFakeTimers(); mount(); });
afterEach(() => { off(); el.remove(); vi.useRealTimers(); });

describe('touch: side taps', () => {
  it('a tap on the left or right 30% turns the page at once', () => {
    drag([[50, 300, 0], [52, 300, 40]]);
    drag([[W - 50, 300, 1000], [W - 50, 300, 1040]]);
    drag([[290, 300, 2000], [290, 300, 2040]]);
    drag([[710, 300, 3000], [710, 300, 3040]]);
    expect(seen).toEqual([{ type: 'prev' }, { type: 'next' }, { type: 'prev' }, { type: 'next' }]);
    vi.advanceTimersByTime(2000);
    expect(store.get().overlayVisible).toBe(true); // side taps never toggle the bars
  });

  it('quick taps on one side turn one page each, never zoom', () => {
    drag([[900, 300, 0], [900, 300, 30]]);
    drag([[900, 300, 100], [900, 300, 130]]);
    expect(seen).toEqual([{ type: 'next' }, { type: 'next' }]);
  });

});

describe('fitted bigger than the stage (fit width)', () => {
  it('a drag pans, a pull does not close, and side taps still turn pages', () => {
    off(); el.remove();
    mount({}, true);
    drag([[500, 500, 0], [500, 300, 50], [500, 100, 100]]);
    drag([[500, 100, 500], [500, 500, 550]]);
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.every((a) => a.type === 'pan_by')).toBe(true);
    seen.length = 0;
    drag([[900, 300, 2000], [900, 300, 2040]]);
    expect(seen).toEqual([{ type: 'next' }]);
  });

  it('the wheel scrolls the page and turns it only after the bottom edge', () => {
    off(); el.remove();
    // The controller clamps: the image can move up 150 px, then no further.
    mount({}, true, (a) => {
      if (a.type !== 'pan_by') return;
      const z = store.get().zoom;
      store.set({ zoom: { ...z, y: Math.max(-150, z.y + a.dy) } });
    });
    store.set({ zoom: { mode: 'fit', scale: 1, x: 0, y: 0 } });
    wheel({ deltaY: 100 }, 0);
    wheel({ deltaY: 100 }, 50);
    expect(seen.filter((a) => a.type === 'next')).toEqual([]);
    expect(store.get().zoom.y).toBe(-150);
    wheel({ deltaY: 100 }, 100); // at the edge now: this notch turns the page
    expect(seen.filter((a) => a.type === 'next')).toEqual([{ type: 'next' }]);
  });
});
