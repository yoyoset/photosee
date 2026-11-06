// The phone review of 2026-11-06: Back in the folder view, long-press on
// iOS, keeping the screen on during a slideshow, folder covers.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import type { Api, Listing, Session, Settings } from '../src/api';
import { bindBrowserBack } from '../src/browser-back';
import { LONG_PRESS_MS } from '../src/constants';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { keepAwakeDuringSlideshow } from '../src/keep-awake';
import { Store } from '../src/state';
import { bindLongPress } from '../src/touch-tap';
import { defaultUiState } from '../src/uistate';
import { bindViewerHistory } from '../src/viewer-history';

const popped = (): Promise<void> => new Promise((r) => window.addEventListener('popstate', () => r(), { once: true }));

describe('Back in the folder view', () => {
  let off: (() => void) | null = null;
  afterEach(() => { off?.(); off = null; });

  it('takes one step inside the app and restores its guard; with no step left it lets go', async () => {
    let steps = 1;
    const step = vi.fn(() => steps-- > 0);
    const b = bindBrowserBack(window, () => false, step);
    off = b.dispose;
    expect(history.state).toEqual({ photoseeGuard: true });
    const len = history.length;
    history.back();
    await popped();
    expect(step).toHaveBeenCalledTimes(1);
    expect(history.state).toEqual({ photoseeGuard: true }); // armed again
    expect(history.length).toBe(len);
    history.back();
    await popped();
    expect(step).toHaveBeenCalledTimes(2);
    expect(history.state).not.toEqual({ photoseeGuard: true }); // the next Back leaves
  });

  it('a Back that closes the viewer, or the viewer tidying up its entry, is not a step', async () => {
    const step = vi.fn(() => true);
    const b = bindBrowserBack(window, () => false, step);
    off = b.dispose;
    b.arm();
    let close = bindViewerHistory(window, () => {});
    history.back(); // the user's Back closes the viewer: lands on the guard
    await popped();
    close();
    close = bindViewerHistory(window, () => {});
    const wait = popped();
    close(); // left by the on-screen button: the viewer goes back itself
    await wait;
    expect(step).not.toHaveBeenCalled();
  });
});

describe('long-press opens the item menu (iOS sends no contextmenu)', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); document.body.replaceChildren(); });

  const setup = () => {
    const el = document.body.appendChild(document.createElement('div'));
    const fire = vi.fn();
    const click = vi.fn();
    bindLongPress(el, fire);
    el.addEventListener('click', click);
    const ev = (type: string, x = 10, kind = 'touch') =>
      el.dispatchEvent(new PointerEvent(type, { pointerType: kind, clientX: x, clientY: 10, bubbles: true }));
    return { el, fire, click, ev };
  };

  it('fires after LONG_PRESS_MS and swallows the click of the lift and a native contextmenu', () => {
    const s = setup();
    s.ev('pointerdown');
    vi.advanceTimersByTime(LONG_PRESS_MS - 1);
    expect(s.fire).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(s.fire).toHaveBeenCalledTimes(1);
    s.ev('pointerup');
    s.el.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    const menu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    s.el.dispatchEvent(menu);
    expect(s.click).not.toHaveBeenCalled();
    expect(menu.defaultPrevented).toBe(true);
  });

  it('a short tap, a finger that moves (a scroll) or a mouse never fires', () => {
    const s = setup();
    s.ev('pointerdown'); vi.advanceTimersByTime(100); s.ev('pointerup');
    s.ev('pointerdown'); s.ev('pointermove', 40); vi.advanceTimersByTime(LONG_PRESS_MS * 2);
    s.ev('pointerdown', 10, 'mouse'); vi.advanceTimersByTime(LONG_PRESS_MS * 2);
    expect(s.fire).not.toHaveBeenCalled();
  });
});

describe('keep the screen on during a slideshow', () => {
  afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });

  it('plain HTTP (no Wake Lock): a muted looping video plays, and goes when the slideshow stops', () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue();
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
    const store = new Store();
    const off = keepAwakeDuringSlideshow(store, document);
    store.set({ slideshow: 'playing' });
    const v = document.querySelector('video');
    expect(v?.getAttribute('src')).toBe('/keepawake.mp4');
    expect([v?.muted, v?.loop]).toEqual([true, true]);
    expect(play).toHaveBeenCalledTimes(1);
    store.set({ slideshow: 'off' });
    expect(document.querySelector('video')).toBeNull();
    off();
  });

  it('a secure page uses the Wake Lock API and releases it on stop', async () => {
    const release = vi.fn(async () => {});
    const request = vi.fn(async () => ({ release }));
    Object.defineProperty(navigator, 'wakeLock', { configurable: true, value: { request } });
    Object.defineProperty(window, 'isSecureContext', { configurable: true, value: true });
    try {
      const store = new Store();
      const off = keepAwakeDuringSlideshow(store, document);
      store.set({ slideshow: 'playing' });
      await Promise.resolve(); await Promise.resolve();
      expect(request).toHaveBeenCalledWith('screen');
      store.set({ slideshow: 'off' });
      expect(release).toHaveBeenCalled();
      expect(document.querySelector('video')).toBeNull();
      off();
    } finally {
      Reflect.deleteProperty(navigator, 'wakeLock');
      Reflect.deleteProperty(window, 'isSecureContext');
    }
  });
});

describe('folder covers in the grid', () => {
  let app: App | null = null;
  afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

  it('a folder tile asks /cover, shows it once loaded, and keeps its icon when there is none', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
    const list: Listing = { dir: '/c', parent: null, images: [], dirs: [{ name: 'v1', path: '/c/v1' }, { name: 'v2', path: '/c/v2' }] };
    const api = {
      session: async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: { dir: '/c', file: null } }),
      config: async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false }),
      list: async () => list, uiState: async () => ({ ...defaultUiState(), thumb: 160 }),
      saveUiState: async () => {}, nav: vi.fn(), meta: () => new Promise(() => {}),
    };
    const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
    app = await boot({ api: api as unknown as Api, host });
    await app.started;
    const covers = [...document.querySelectorAll<HTMLImageElement>('.gc.dir img.gc-cover')];
    expect(covers.map((c) => c.getAttribute('src'))).toEqual(['/api/v1/cover?p=%2Fc%2Fv1&s=192', '/api/v1/cover?p=%2Fc%2Fv2&s=192']);
    const [a, b] = covers as [HTMLImageElement, HTMLImageElement];
    a.dispatchEvent(new Event('load'));
    expect(a.parentElement?.classList.contains('has-cover')).toBe(true);
    b.dispatchEvent(new Event('error'));
    const tile = document.querySelectorAll('.gc.dir .gc-thumb')[1] as HTMLElement;
    expect(tile.querySelector('img')).toBeNull();
    expect(tile.classList.contains('bad')).toBe(false); // the folder icon, not "broken"
    expect(tile.querySelector('svg')).not.toBeNull();
  });
});
