// The thin DOM binding: events in, Actions out. Classification itself is
// covered purely in gestures.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Action } from '../src/actions';
import type { Api, ImageSource, Session } from '../src/api';
import {
  DOUBLE_TAP_MS, WHEEL_COOLDOWN_MS, WHEEL_PAGE_DELTA, ZOOM_STEP,
} from '../src/constants';
import type { Ctx } from '../src/ctx';
import { bindGestures } from '../src/gestures';
import { EscStack, Store, type ViewState } from '../src/state';

const W = 1000;
const H = 600;
let el: HTMLElement;
let store: Store;
let seen: Action[];
let off: () => void;

function mount(patch: Partial<ViewState> = {}): void {
  seen = [];
  store = new Store();
  store.set(patch);
  const ctx: Ctx = {
    store, esc: new EscStack(), dispatch: (a) => seen.push(a),
    api: {} as Api, src: {} as ImageSource, session: {} as Session,
  };
  el = document.createElement('div');
  el.getBoundingClientRect = () => ({ left: 0, top: 0, width: W, height: H, right: W, bottom: H, x: 0, y: 0, toJSON: () => ({}) });
  document.body.append(el);
  off = bindGestures(el, ctx);
}

const zoomedState: Partial<ViewState> = { zoom: { mode: 'actual', scale: 1, x: 0, y: 0 } };

function ptr(type: string, x: number, y: number, t: number, o: { id?: number; kind?: string; button?: number } = {}): PointerEvent {
  const e = new PointerEvent(type, {
    clientX: x, clientY: y, pointerId: o.id ?? 1, pointerType: o.kind ?? 'touch',
    button: o.button ?? 0, bubbles: true, cancelable: true,
  });
  Object.defineProperty(e, 'timeStamp', { value: t });
  el.dispatchEvent(e);
  return e;
}

function drag(pts: [number, number, number][], kind = 'touch', id = 1): void {
  pts.forEach(([x, y, t], i) => ptr(i === 0 ? 'pointerdown' : 'pointermove', x, y, t, { kind, id }));
  const [x, y, t] = pts[pts.length - 1] as [number, number, number];
  ptr('pointerup', x, y, t, { kind, id });
}

function wheel(o: WheelEventInit, t: number): WheelEvent {
  const e = new WheelEvent('wheel', { bubbles: true, cancelable: true, clientX: 300, clientY: 200, ...o });
  Object.defineProperty(e, 'timeStamp', { value: t });
  el.dispatchEvent(e);
  return e;
}

beforeEach(() => { vi.useFakeTimers(); mount(); });
afterEach(() => { off(); el.remove(); vi.useRealTimers(); });

describe('wheel', () => {
  it('trackpad stream turns one page; the event is always default-prevented', () => {
    for (let i = 0; i < 30; i++) expect(wheel({ deltaY: 8 }, i * 16).defaultPrevented).toBe(true);
    expect(seen).toEqual([{ type: 'next' }]);
  });

  it('mouse notches: up is prev, down is next, cooldown applies', () => {
    wheel({ deltaY: 100 }, 0);
    wheel({ deltaY: 100 }, 50);
    wheel({ deltaY: -100 }, WHEEL_COOLDOWN_MS + 10);
    expect(seen).toEqual([{ type: 'next' }, { type: 'prev' }]);
  });

  it('line-mode wheel (Firefox) still turns one page per notch', () => {
    wheel({ deltaY: 3, deltaMode: 1 }, 0);
    expect(seen).toEqual([{ type: 'next' }]);
  });

});

describe('wheel: zoom and pan', () => {
  it('Ctrl+wheel zooms around the cursor, one ZOOM_STEP per notch', () => {
    expect(wheel({ deltaY: -WHEEL_PAGE_DELTA, ctrlKey: true }, 0).defaultPrevented).toBe(true);
    wheel({ deltaY: WHEEL_PAGE_DELTA, ctrlKey: true }, 1);
    wheel({ deltaY: -4, ctrlKey: true }, 2); // pinch-sized delta: smooth, not a full step
    expect(seen).toHaveLength(3);
    const [a, b, c] = seen as Extract<Action, { type: 'zoom_by' }>[];
    expect(a?.factor).toBeCloseTo(ZOOM_STEP);
    expect(a?.anchor).toEqual({ x: 300, y: 200 });
    expect(b?.factor).toBeCloseTo(1 / ZOOM_STEP);
    expect(c?.factor).toBeGreaterThan(1);
    expect(c?.factor).toBeLessThan(1.02);
  });

  it('wheel setting "zoom" zooms without Ctrl and never turns pages', () => {
    off(); el.remove();
    mount({ settings: { wheel: 'zoom' } as ViewState['settings'] });
    wheel({ deltaY: -100 }, 0);
    expect(seen).toEqual([{ type: 'zoom_by', factor: ZOOM_STEP, anchor: { x: 300, y: 200 } }]);
  });

  it('when zoomed the wheel pans instead of turning pages', () => {
    off(); el.remove();
    mount(zoomedState);
    wheel({ deltaY: 100, deltaX: 20 }, 0);
    wheel({ deltaY: 100 }, 1);
    expect(seen).toEqual([{ type: 'pan_by', dx: -20, dy: -100 }, { type: 'pan_by', dx: -0, dy: -100 }]);
  });
});

describe('mouse', () => {
  it('double-click toggles zoom with no page turn', () => {
    ptr('pointerdown', 500, 300, 0, { kind: 'mouse' }); ptr('pointerup', 500, 300, 30, { kind: 'mouse' });
    ptr('pointerdown', 500, 300, 100, { kind: 'mouse' }); ptr('pointerup', 500, 300, 130, { kind: 'mouse' });
    vi.advanceTimersByTime(1000);
    expect(seen).toEqual([{ type: 'zoom_toggle' }]);
  });

  it('edge click turns the page at once', () => {
    ptr('pointerdown', 900, 300, 0, { kind: 'mouse' }); ptr('pointerup', 900, 300, 30, { kind: 'mouse' });
    expect(seen).toEqual([{ type: 'next' }]);
    ptr('pointerdown', 50, 300, 1000, { kind: 'mouse' }); ptr('pointerup', 50, 300, 1030, { kind: 'mouse' });
    expect(seen).toEqual([{ type: 'next' }, { type: 'prev' }]);
  });

  it('fast edge clicks each turn a page and never zoom', () => {
    for (const t of [0, 100, 200]) {
      ptr('pointerdown', 900, 300, t, { kind: 'mouse' }); ptr('pointerup', 900, 300, t + 20, { kind: 'mouse' });
    }
    vi.advanceTimersByTime(2000);
    expect(seen).toEqual([{ type: 'next' }, { type: 'next' }, { type: 'next' }]);
  });

  it('an edge click does not pair with a following middle click into a zoom', () => {
    ptr('pointerdown', 900, 300, 0, { kind: 'mouse' }); ptr('pointerup', 900, 300, 20, { kind: 'mouse' });
    ptr('pointerdown', 880, 300, 100, { kind: 'mouse' }); ptr('pointerup', 880, 300, 120, { kind: 'mouse' });
    vi.advanceTimersByTime(2000);
    expect(seen.filter((a) => a.type === 'zoom_toggle')).toEqual([]);
  });

  it('a single click in the middle does nothing', () => {
    ptr('pointerdown', 500, 300, 0, { kind: 'mouse' }); ptr('pointerup', 500, 300, 30, { kind: 'mouse' });
    vi.advanceTimersByTime(2000);
    expect(seen).toEqual([]);
  });

});

describe('mouse: pan and guards', () => {
  it('edge clicks do nothing when zoomed; dragging pans', () => {
    off(); el.remove();
    mount(zoomedState);
    ptr('pointerdown', 900, 300, 0, { kind: 'mouse' }); ptr('pointerup', 900, 300, 30, { kind: 'mouse' });
    vi.advanceTimersByTime(1000);
    expect(seen).toEqual([]);
    drag([[500, 300, 100], [504, 300, 110], [520, 310, 120], [540, 330, 130]], 'mouse');
    expect(seen.every((a) => a.type === 'pan_by')).toBe(true);
    const sum = (k: 'dx' | 'dy'): number => seen.reduce((s, a) => s + (a.type === 'pan_by' ? a[k] : 0), 0);
    expect([sum('dx'), sum('dy')]).toEqual([40, 30]); // total drag, nothing lost to the slop
  });

  it('dragging in fit mode, or with a non-primary button, emits nothing', () => {
    drag([[500, 300, 0], [700, 300, 20], [900, 300, 40]], 'mouse');
    ptr('pointerdown', 500, 300, 100, { kind: 'mouse', button: 2 });
    ptr('pointerup', 500, 300, 120, { kind: 'mouse', button: 2 });
    vi.advanceTimersByTime(1000);
    expect(seen).toEqual([]);
  });

  it('ignores pointers that start on controls', () => {
    const b = document.createElement('button');
    el.append(b);
    b.dispatchEvent(new PointerEvent('pointerdown', { clientX: 900, clientY: 300, pointerType: 'mouse', bubbles: true }));
    b.dispatchEvent(new PointerEvent('pointerup', { clientX: 900, clientY: 300, pointerType: 'mouse', bubbles: true }));
    vi.advanceTimersByTime(1000);
    expect(seen).toEqual([]);
  });
});

describe('touch', () => {
  it('sets touch-action: none on the stage and restores it on unbind', () => {
    expect(el.style.touchAction).toBe('none');
    off();
    expect(el.style.touchAction).toBe('');
  });

  it('swipe left past 30% turns to the next page; swipe right to the previous', () => {
    drag([[800, 300, 0], [600, 300, 100], [480, 305, 200]]);
    drag([[200, 300, 1000], [400, 300, 1100], [520, 300, 1200]]);
    expect(seen).toEqual([{ type: 'next' }, { type: 'prev' }]);
  });

  it('a short drag snaps back: no action', () => {
    drag([[500, 300, 0], [450, 300, 300], [400, 300, 600]]);
    expect(seen).toEqual([]);
  });

  it('pull down past PULL_CLOSE_PX sends escape', () => {
    drag([[500, 100, 0], [500, 200, 400], [505, 260, 800]]);
    expect(seen).toEqual([{ type: 'escape' }]);
  });

  it('publishes drag feedback as CSS custom properties and clears it on release', () => {
    ptr('pointerdown', 500, 300, 0);
    ptr('pointermove', 400, 305, 50);
    expect(el.style.getPropertyValue('--drag-x')).toBe('-100px');
    expect(el.dataset.dragging).toBe('x');
    ptr('pointermove', 480, 380, 100);
    expect(el.dataset.dragging).toBe('y');
    expect(el.style.getPropertyValue('--drag-y')).toBe('80px');
    expect(Number(el.style.getPropertyValue('--pull'))).toBeCloseTo(0.8);
    expect(el.style.getPropertyValue('--drag-x')).toBe('0px');
    ptr('pointerup', 480, 380, 3000);
    expect(el.style.getPropertyValue('--pull')).toBe('');
    expect(el.dataset.dragging).toBeUndefined();
  });

  it('pointercancel clears the feedback without an action', () => {
    ptr('pointerdown', 500, 300, 0);
    ptr('pointermove', 100, 300, 50);
    ptr('pointercancel', 100, 300, 60);
    expect(el.dataset.dragging).toBeUndefined();
    expect(seen).toEqual([]);
  });

});

describe('touch: taps', () => {
  it('single tap toggles the overlay after DOUBLE_TAP_MS, via the store only', () => {
    const before = store.get().overlayVisible;
    drag([[500, 300, 0], [501, 300, 40]]);
    vi.advanceTimersByTime(DOUBLE_TAP_MS - 1);
    expect(store.get().overlayVisible).toBe(before);
    vi.advanceTimersByTime(1);
    expect(store.get().overlayVisible).toBe(!before);
    expect(seen).toEqual([]);
  });

  it('double tap zooms and does not toggle the overlay', () => {
    const before = store.get().overlayVisible;
    drag([[500, 300, 0], [500, 300, 40]]);
    drag([[505, 302, 150], [505, 302, 190]]);
    vi.advanceTimersByTime(2000);
    expect(seen).toEqual([{ type: 'zoom_toggle' }]);
    expect(store.get().overlayVisible).toBe(before);
  });

  it('two taps far apart are two overlay toggles, not a zoom', () => {
    const before = store.get().overlayVisible;
    drag([[100, 300, 0], [100, 300, 40]]);
    drag([[800, 300, 150], [800, 300, 190]]);
    vi.advanceTimersByTime(2000);
    expect(seen).toEqual([]);
    expect(store.get().overlayVisible).toBe(before); // toggled twice
  });

});

describe('touch: zoomed and pinch', () => {
  it('when zoomed a drag pans and never turns the page or closes', () => {
    off(); el.remove();
    mount(zoomedState);
    drag([[800, 300, 0], [600, 320, 50], [200, 350, 100]]);
    drag([[500, 100, 500], [500, 500, 550]]);
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.every((a) => a.type === 'pan_by')).toBe(true);
    expect(el.dataset.dragging).toBeUndefined();
  });

  it('pinch zooms around the midpoint and then stays quiet until all fingers lift', () => {
    ptr('pointerdown', 400, 300, 0, { id: 1 });
    ptr('pointerdown', 600, 300, 5, { id: 2 });
    ptr('pointermove', 300, 300, 20, { id: 1 });
    ptr('pointermove', 700, 300, 25, { id: 2 });
    const zooms = seen.filter((a) => a.type === 'zoom_by');
    expect(zooms).toHaveLength(2);
    const last = zooms[1] as Extract<Action, { type: 'zoom_by' }>;
    expect(last.anchor).toEqual({ x: 500, y: 300 });
    const product = zooms.reduce((p, a) => p * (a as { factor: number }).factor, 1);
    expect(product).toBeCloseTo(2); // 200px -> 400px apart
    seen.length = 0;
    // Lift one finger, drag the other far: neither swipe nor tap.
    ptr('pointerup', 300, 300, 40, { id: 1 });
    ptr('pointermove', 100, 300, 60, { id: 2 });
    ptr('pointerup', 100, 300, 70, { id: 2 });
    vi.advanceTimersByTime(2000);
    expect(seen).toEqual([]);
    expect(store.get().overlayVisible).toBe(true);
  });
});
