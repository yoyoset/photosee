import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Api, Format, ImageItem, ImageSource, Session } from '../src/api';
import { CURSOR_HIDE_MS, OVERLAY_HIDE_MS, OVERLAY_REVEAL_ZONE_PX } from '../src/constants';
import type { Ctx } from '../src/ctx';
import { setLang } from '../src/i18n';
import {
  IdleTimer, buttonViews, folderName, inRevealZone, mountOverlay, splitName, titleParts,
} from '../src/overlay';
import { EscStack, Store, initialState } from '../src/state';

const must = <T>(x: T | null | undefined): T => {
  if (x == null) throw new Error('expected a value');
  return x;
};
function make() {
  const changes: boolean[] = [];
  return { changes, timer: new IdleTimer(1000, (v) => changes.push(v)) };
}

describe('IdleTimer', () => {
  it('hides once idle for the timeout, not before', () => {
    const { timer, changes } = make();
    timer.wake(0);
    timer.tick(999);
    expect(timer.isShown).toBe(true);
    timer.tick(1000);
    expect(timer.isShown).toBe(false);
    expect(changes).toEqual([false]);
  });

  it('restarts the countdown on input', () => {
    const { timer } = make();
    timer.wake(0);
    timer.wake(800);
    timer.tick(1500);
    expect(timer.isShown).toBe(true);
    expect(timer.remaining(1500)).toBe(300);
    timer.tick(1800);
    expect(timer.isShown).toBe(false);
  });

});

describe('IdleTimer holds', () => {
  it('never hides while hovered or focused, and counts again after release', () => {
    const { timer } = make();
    timer.hold('hover', true, 0);
    timer.hold('focus', true, 0);
    timer.tick(60_000);
    expect(timer.isShown).toBe(true);
    expect(timer.remaining(60_000)).toBeNull();
    timer.hold('hover', false, 60_000);
    timer.tick(70_000);
    expect(timer.isShown).toBe(true); // focus still holds
    timer.hold('focus', false, 70_000);
    timer.tick(70_999);
    expect(timer.isShown).toBe(true);
    timer.tick(71_000);
    expect(timer.isShown).toBe(false);
  });

  it('shows again on wake and reports each change once', () => {
    const { timer, changes } = make();
    timer.tick(5000);
    timer.wake(5001);
    timer.wake(5002);
    expect(changes).toEqual([false, true]);
  });

});

describe('reveal zone', () => {
  it('is the bottom band of the viewport', () => {
    expect(inRevealZone(700, 800)).toBe(true);
    expect(inRevealZone(800 - OVERLAY_REVEAL_ZONE_PX, 800)).toBe(true);
    expect(inRevealZone(800 - OVERLAY_REVEAL_ZONE_PX - 1, 800)).toBe(false);
    expect(inRevealZone(10, 800)).toBe(false);
  });
});

describe('title helpers', () => {
  it('splits the extension off so the stem can be ellipsized alone', () => {
    expect(splitName('IMG_0001.jpg')).toEqual({ stem: 'IMG_0001', ext: '.jpg' });
    expect(splitName('a.b.tar.png')).toEqual({ stem: 'a.b.tar', ext: '.png' });
    expect(splitName('.hidden')).toEqual({ stem: '.hidden', ext: '' });
    expect(splitName('no_extension')).toEqual({ stem: 'no_extension', ext: '' });
    expect(splitName('weird.thisisnotanext')).toEqual({ stem: 'weird.thisisnotanext', ext: '' });
  });

  it('takes the folder from either separator and ignores a trailing one', () => {
    expect(folderName('/photos/2024/trip')).toBe('trip');
    expect(folderName('D:\\photos\\trip\\')).toBe('trip');
    expect(folderName('/')).toBe('/');
  });

  it('numbers the position from 1', () => {
    const item = { name: 'a.jpg', path: '/x/a.jpg', fmt: 'jpeg' as Format, size: 1, mtime_ms: 0 };
    expect(titleParts(item, 22, 412, '/x')).toEqual({ stem: 'a', ext: '.jpg', pos: '23 / 412', folder: 'x' });
  });
});

function images(n: number): ImageItem[] {
  return Array.from({ length: n }, (_, i) => ({
    name: `photo${i}.jpg`, path: `/d/photo${i}.jpg`, fmt: 'jpeg' as Format, size: 1, mtime_ms: 0,
  }));
}

function setup(platform: Session['platform'] = 'windows', n = 30) {
  const root = document.body.appendChild(document.createElement('div'));
  root.id = 'overlay';
  const live = document.body.appendChild(document.createElement('div'));
  live.id = 'live';
  const store = new Store({
    ...initialState, index: 22, listing: { dir: '/pics/trip', parent: null, images: images(n), dirs: [] },
  });
  const dispatch = vi.fn();
  const src: ImageSource = { imageUrl: (p) => p, thumbUrl: (p) => p };
  const session = { platform } as Session;
  const ctx: Ctx = { store, api: {} as Api, src, esc: new EscStack(), session, dispatch };
  const unmount = mountOverlay(root, ctx);
  return { root, live, store, dispatch, unmount };
}

const pointer = (type: string, clientY: number): void => {
  document.dispatchEvent(new MouseEvent(type, { clientY, bubbles: true }));
};

beforeEach(() => setLang('en'));
afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
  document.documentElement.classList.remove('cursor-hidden');
});

describe('mountOverlay: buttons', () => {
  it.each(['windows', 'macos'] as const)('dispatches one action per button on %s', (platform) => {
    const { root, dispatch } = setup(platform);
    const btns = [...root.querySelectorAll<HTMLButtonElement>('.ov-btn')];
    expect(btns.length).toBe(8);
    btns.forEach((b) => b.click());
    expect(dispatch.mock.calls.map((c) => c[0])).toEqual([
      { type: 'prev' }, { type: 'toggle_slideshow' }, { type: 'next' },
      { type: 'rotate', quarterTurns: -1 }, { type: 'rotate', quarterTurns: 1 },
      { type: 'zoom_toggle' }, { type: 'toggle_fullscreen' }, { type: 'open_settings' },
    ]);
    for (const b of btns) {
      const label = b.getAttribute('aria-label') ?? '';
      expect(label).not.toBe('');
      expect(b.querySelector('svg')).not.toBeNull();
      expect(b.title.startsWith(label)).toBe(true);
    }
  });

  it('shows the platform key for fullscreen in the tooltip', () => {
    const find = (platform: Session['platform']) =>
      buttonViews(setup(platform).store.get(), platform).find((v) => v.id === 'fullscreen');
    expect(find('windows')?.shortcut).toContain('F11');
    expect(find('macos')?.shortcut).toMatch(/^F \/ .*⌘/);
  });

  it('writes "label (shortcut)" into title and follows slideshow and zoom state', () => {
    const { root, store } = setup();
    const by = (id: string): HTMLElement => must(root.querySelector<HTMLElement>(`[data-id="${id}"]`));
    expect(by('play').title).toBe('Play slideshow (S)');
    expect(by('fit').title).toBe('Actual size (1)');
    expect(by('settings').title).toBe('Settings');
    store.set({ slideshow: 'playing', zoom: { mode: 'actual', scale: 1, x: 0, y: 0 } });
    expect(by('play').getAttribute('aria-label')).toBe('Pause slideshow');
    expect(by('fit').title).toBe('Fit to window (0)');
  });

  it('follows the system language through t()', () => {
    setLang('zh');
    const { root } = setup();
    expect(root.querySelector('[data-id="next"]')?.getAttribute('aria-label')).toBe('下一张');
  });
});

describe('mountOverlay: title and live region', () => {
  it('renders name, position and folder, and announces index changes', () => {
    const { root, live, store } = setup();
    const text = (): string => root.querySelector('.ov-title')?.textContent ?? '';
    expect(text()).toBe('photo22.jpg · 23 / 30 · trip');
    expect(live.textContent).toBe('23 of 30, photo22.jpg');
    store.set({ index: 3 });
    expect(text()).toBe('photo3.jpg · 4 / 30 · trip');
    expect(live.textContent).toBe('4 of 30, photo3.jpg');
  });

});

describe('mountOverlay: auto-hide', () => {
  it('hides after OVERLAY_HIDE_MS and a pointer move in the reveal zone brings it back', () => {
    vi.useFakeTimers();
    const { root, store } = setup();
    expect(store.get().overlayVisible).toBe(true);
    vi.advanceTimersByTime(OVERLAY_HIDE_MS - 1);
    expect(store.get().overlayVisible).toBe(true);
    vi.advanceTimersByTime(1);
    expect(store.get().overlayVisible).toBe(false);
    expect(root.classList.contains('ov-hidden')).toBe(true);
    pointer('pointermove', window.innerHeight - 10);
    expect(store.get().overlayVisible).toBe(true);
    expect(root.classList.contains('ov-hidden')).toBe(false);
  });

  it('a pointer resting in the reveal zone keeps it open; leaving lets it fade', () => {
    vi.useFakeTimers();
    const { store } = setup();
    pointer('pointermove', window.innerHeight - 10);
    vi.advanceTimersByTime(OVERLAY_HIDE_MS * 3);
    expect(store.get().overlayVisible).toBe(true);
    pointer('pointermove', 50);
    vi.advanceTimersByTime(OVERLAY_HIDE_MS);
    expect(store.get().overlayVisible).toBe(false);
  });

});

describe('mountOverlay: holds', () => {
  it('does not hide while the bottom bar is hovered', () => {
    vi.useFakeTimers();
    const { root, store } = setup();
    const bottom = must(root.querySelector('.ov-bottom'));
    bottom.dispatchEvent(new MouseEvent('pointerenter'));
    vi.advanceTimersByTime(OVERLAY_HIDE_MS * 3);
    expect(store.get().overlayVisible).toBe(true);
    bottom.dispatchEvent(new MouseEvent('pointerleave'));
    vi.advanceTimersByTime(OVERLAY_HIDE_MS);
    expect(store.get().overlayVisible).toBe(false);
  });

  it('does not hide while a button has keyboard focus', () => {
    vi.useFakeTimers();
    const { root, store } = setup();
    const btn = must(root.querySelector<HTMLButtonElement>('.ov-btn'));
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    btn.focus();
    vi.advanceTimersByTime(OVERLAY_HIDE_MS * 3);
    expect(store.get().overlayVisible).toBe(true);
    btn.blur();
    vi.advanceTimersByTime(OVERLAY_HIDE_MS);
    expect(store.get().overlayVisible).toBe(false);
  });

  it('a mouse-focused button (not :focus-visible) does not pin the bar open', () => {
    vi.useFakeTimers();
    const { root, store } = setup();
    const btn = must(root.querySelector<HTMLButtonElement>('.ov-btn'));
    const real = btn.matches.bind(btn);
    Object.defineProperty(btn, 'matches', {
      value: (sel: string): boolean => (sel === ':focus-visible' ? false : real(sel)),
    });
    btn.focus();
    vi.advanceTimersByTime(OVERLAY_HIDE_MS);
    expect(store.get().overlayVisible).toBe(false);
  });

  it('follows overlayVisible written by someone else, and input still revives it', () => {
    vi.useFakeTimers();
    const { store } = setup();
    store.set({ overlayVisible: false });
    pointer('pointermove', 10);
    expect(store.get().overlayVisible).toBe(true);
  });

});

describe('mountOverlay: cursor and lifecycle', () => {
  it('hides the cursor in fullscreen only, and shows it on movement', () => {
    vi.useFakeTimers();
    const { store } = setup();
    const hidden = (): boolean => document.documentElement.classList.contains('cursor-hidden');
    vi.advanceTimersByTime(CURSOR_HIDE_MS * 2);
    expect(hidden()).toBe(false);
    store.set({ fullscreen: true });
    vi.advanceTimersByTime(CURSOR_HIDE_MS);
    expect(hidden()).toBe(true);
    pointer('pointermove', 10);
    expect(hidden()).toBe(false);
    vi.advanceTimersByTime(CURSOR_HIDE_MS);
    expect(hidden()).toBe(true);
    store.set({ fullscreen: false });
    pointer('pointermove', 10);
    expect(hidden()).toBe(false);
    vi.advanceTimersByTime(CURSOR_HIDE_MS * 2);
    expect(hidden()).toBe(false);
  });

  it('unmount removes listeners and timers', () => {
    vi.useFakeTimers();
    const { store, unmount } = setup();
    unmount();
    vi.advanceTimersByTime(OVERLAY_HIDE_MS * 2);
    expect(store.get().overlayVisible).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });
});
