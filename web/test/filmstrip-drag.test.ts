// The filmstrip can be dragged and wheeled (user, 2026-11-06: "the strip is
// dead"); a drag never jumps to a thumbnail, a tap still does.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Api, Format, ImageItem, ImageSource, Session } from '../src/api';
import { FILMSTRIP_THUMB_PX } from '../src/constants';
import type { Ctx } from '../src/ctx';
import { STRIDE_PX, centerOffset, mountFilmstrip } from '../src/filmstrip';
import { DRAG_SLOP_PX, clampOffset, releaseSpeed } from '../src/filmstrip-drag';
import { EscStack, Store, initialState } from '../src/state';

function setup(n = 100, index = 50) {
  const images: ImageItem[] = Array.from({ length: n }, (_, i) => ({
    name: `p${i}.jpg`, path: `/d/p${i}.jpg`, fmt: 'jpeg' as Format, size: 1, mtime_ms: 0,
  }));
  const store = new Store({ ...initialState, index, listing: { dir: '/d', parent: null, images, dirs: [] } });
  const dispatch = vi.fn();
  const src: ImageSource = { imageUrl: (p) => p, thumbUrl: (p) => p };
  const ctx: Ctx = { store, api: {} as Api, src, esc: new EscStack(), session: { platform: 'windows' } as Session, dispatch };
  const host = document.body.appendChild(document.createElement('div'));
  const off = mountFilmstrip(host, ctx);
  const strip = host.querySelector('.fs') as HTMLElement;
  const track = host.querySelector('.fs-track') as HTMLElement;
  const offset = (): number => -Number(/translateX\((-?[\d.]+)px\)/.exec(track.style.transform)?.[1]);
  const cell = (i: number): HTMLElement => track.querySelector(`[data-i="${i}"]`) as HTMLElement;
  const ptr = (target: EventTarget, type: string, x: number, t: number): void => {
    const e = new PointerEvent(type, { clientX: x, button: 0, pointerType: 'touch', bubbles: true });
    Object.defineProperty(e, 'timeStamp', { value: t });
    target.dispatchEvent(e);
  };
  return { store, dispatch, strip, track, offset, cell, ptr, off };
}

afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('filmstrip-drag: pure parts', () => {
  it('clampOffset keeps the first or last cell centred at the ends', () => {
    const min = FILMSTRIP_THUMB_PX / 2 - 500;
    expect(clampOffset(-9999, 1000, 10, STRIDE_PX)).toBe(min);
    expect(clampOffset(9999, 1000, 10, STRIDE_PX)).toBe(9 * STRIDE_PX + min);
    expect(clampOffset(123, 1000, 10, STRIDE_PX)).toBe(123);
    expect(clampOffset(50, 1000, 0, STRIDE_PX)).toBe(min); // empty: no NaN
  });
  it('releaseSpeed uses only the end of the drag', () => {
    expect(releaseSpeed([{ x: 0, t: 0 }, { x: 0, t: 500 }, { x: 80, t: 540 }, { x: 160, t: 580 }])).toBe(2);
    expect(releaseSpeed([{ x: 0, t: 0 }])).toBe(0);
    expect(releaseSpeed([])).toBe(0);
  });
});

describe('dragging the strip', () => {
  it('a drag moves the strip with the finger and does not change the image', () => {
    const s = setup();
    const start = s.offset();
    expect(start).toBe(centerOffset(50, window.innerWidth));
    s.ptr(s.cell(50), 'pointerdown', 300, 0);
    s.ptr(window, 'pointermove', 300 - DRAG_SLOP_PX, 10);
    expect(s.offset()).toBe(start); // within the slop: still a tap
    s.ptr(window, 'pointermove', 100, 400);
    expect(s.offset()).toBe(start + 200);
    expect(s.cell(55)).toBeTruthy(); // cells that came into view were rendered
    s.ptr(window, 'pointerup', 100, 2000); // held still before lifting: no glide
    s.cell(50).dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(s.dispatch).not.toHaveBeenCalled(); // the lift of a drag is not a tap
    expect(s.store.get().index).toBe(50);
  });

  it('a plain tap still jumps to that thumbnail, and the next image re-centres the strip', () => {
    const s = setup();
    s.ptr(s.cell(50), 'pointerdown', 300, 0);
    s.ptr(window, 'pointermove', 100, 400);
    s.ptr(window, 'pointerup', 100, 2000);
    s.cell(52).dispatchEvent(new MouseEvent('click', { bubbles: true })); // swallowed (lift of the drag)
    s.ptr(s.cell(52), 'pointerdown', 200, 3000);
    s.ptr(window, 'pointerup', 200, 3050);
    s.cell(52).dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(s.dispatch).toHaveBeenCalledWith({ type: 'goto', index: 52 });
    s.store.set({ index: 52 });
    expect(s.offset()).toBe(centerOffset(52, window.innerWidth));
  });

});

describe('the strip at its ends, the wheel, a flick', () => {
  it('stops at the ends, and the wheel scrolls it without reaching the page', () => {
    const s = setup(10, 0);
    const min = s.offset();
    s.ptr(s.cell(0), 'pointerdown', 100, 0);
    s.ptr(window, 'pointermove', 900, 100);
    expect(s.offset()).toBe(min);
    s.ptr(window, 'pointerup', 900, 2000);
    const outer = vi.fn();
    document.body.addEventListener('wheel', outer);
    const w = new WheelEvent('wheel', { deltaY: 100, bubbles: true, cancelable: true });
    s.strip.dispatchEvent(w);
    expect([w.defaultPrevented, s.offset(), outer.mock.calls.length]).toEqual([true, min + 100, 0]);
  });

  it('a flick keeps gliding after the finger lifts', () => {
    let frames: FrameRequestCallback[] = [];
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((f) => { frames.push(f); return frames.length; });
    vi.spyOn(performance, 'now').mockReturnValue(0);
    const s = setup();
    s.ptr(s.cell(50), 'pointerdown', 400, 0);
    s.ptr(window, 'pointermove', 360, 20);
    s.ptr(window, 'pointermove', 300, 40);
    s.ptr(window, 'pointerup', 300, 40);
    const atLift = s.offset();
    for (let t = 16; t <= 160; t += 16) { const f = frames; frames = []; f.forEach((cb) => cb(t)); }
    expect(s.offset()).toBeGreaterThan(atLift + 50); // finger went left: the strip keeps going that way
  });
});
