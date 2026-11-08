// Fullscreen is for looking: the bars stay away unless summoned (ux-spec §Overlay).
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Api, ImageSource, Session } from '../src/api';
import { OVERLAY_HIDE_MS } from '../src/constants';
import type { Ctx } from '../src/ctx';
import { mountOverlay } from '../src/overlay';
import { EscStack, Store, initialState } from '../src/state';

const must = <T>(x: T | null | undefined): T => {
  if (x == null) throw new Error('expected a value');
  return x;
};
let unmount: (() => void) | null = null;

function setup() {
  const root = document.body.appendChild(document.createElement('div'));
  const store = new Store({ ...initialState });
  const src: ImageSource = { imageUrl: (p) => p, thumbUrl: (p) => p };
  const ctx: Ctx = { store, api: {} as Api, src, esc: new EscStack(), session: { platform: 'windows' } as Session, dispatch: vi.fn() };
  unmount = mountOverlay(root, ctx);
  return { root, store };
}
const pointer = (type: string, clientY: number): void => {
  document.dispatchEvent(new MouseEvent(type, { clientY, bubbles: true }));
};
afterEach(() => {
  // Listeners sit on `document`: a leftover overlay would keep reacting to the next test.
  unmount?.();
  unmount = null;
  vi.useRealTimers();
  document.body.replaceChildren();
  document.documentElement.classList.remove('cursor-hidden');
});

describe('mountOverlay: fullscreen stays clean', () => {
  const hide = (store: Store): void => {
    vi.advanceTimersByTime(OVERLAY_HIDE_MS);
    expect(store.get().overlayVisible).toBe(false);
  };

  it('keys, wheel and mouse moves away from the bottom do not bring the bars back', () => {
    vi.useFakeTimers();
    const { store } = setup();
    store.set({ fullscreen: true });
    hide(store);
    pointer('pointermove', 50);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    document.dispatchEvent(new WheelEvent('wheel', { deltaY: 100, bubbles: true }));
    expect(store.get().overlayVisible).toBe(false);
    pointer('pointermove', window.innerHeight - 10); // the bottom edge still reveals them
    expect(store.get().overlayVisible).toBe(true);
  });

  it('a finger on the picture (a page-turn tap) never wakes them, windowed or not', () => {
    vi.useFakeTimers();
    const { store } = setup();
    hide(store);
    const tap = new PointerEvent('pointerdown', { pointerType: 'touch', clientY: window.innerHeight - 5, bubbles: true });
    document.dispatchEvent(tap);
    expect(store.get().overlayVisible).toBe(false);
    store.set({ fullscreen: true });
    document.dispatchEvent(tap);
    expect(store.get().overlayVisible).toBe(false);
  });

  it('a finger on the bars themselves keeps them up', () => {
    vi.useFakeTimers();
    const { root, store } = setup();
    store.set({ fullscreen: true });
    vi.advanceTimersByTime(OVERLAY_HIDE_MS - 10);
    must(root.querySelector('.ov-btn')).dispatchEvent(
      new PointerEvent('pointerdown', { pointerType: 'touch', bubbles: true }));
    vi.advanceTimersByTime(OVERLAY_HIDE_MS - 10);
    expect(store.get().overlayVisible).toBe(true);
  });

  // iPad mini report (2026-11-08): every tap brought all the bars up, and a middle
  // tap flashed them and hid them again. A touch pointer fires `pointerleave` on
  // the root element right after it lifts, and that used to count as input.
  it('the pointerleave a lifted finger fires does not wake the bars', () => {
    vi.useFakeTimers();
    const { store } = setup();
    hide(store);
    document.documentElement.dispatchEvent(new PointerEvent('pointerleave', { pointerType: 'touch' }));
    expect(store.get().overlayVisible).toBe(false);
    store.set({ fullscreen: true });
    document.documentElement.dispatchEvent(new PointerEvent('pointerleave', { pointerType: 'touch' }));
    expect(store.get().overlayVisible).toBe(false);
  });
});
