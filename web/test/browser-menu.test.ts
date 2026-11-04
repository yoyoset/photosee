import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type ImageItem, type Listing, type Session, type Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang } from '../src/i18n';
import { defaultUiState, type UiState } from '../src/uistate';
import type { ViewerEnv } from '../src/viewer';

setLang('en');
const flush = async (): Promise<void> => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
const env: ViewerEnv = {
  dpr: () => 1, view: () => ({ w: 800, h: 600 }), now: () => Date.now(),
  watch: () => () => {}, watchDrag: () => () => {}, reducedMotion: () => true,
};

const image = (dir: string, name: string, size = 1): ImageItem =>
  ({ name, path: `${dir}/${name}`, fmt: 'jpeg', size, mtime_ms: 0 });
const listing = (dir: string, count: number, dirs: string[] = [], parent: string | null = null): Listing => ({
  dir, parent,
  dirs: dirs.map((d) => ({ name: d, path: `${dir}/${d}` })),
  images: Array.from({ length: count }, (_, i) => image(dir, `img${String(i).padStart(5, '0')}.jpg`, count - i)),
});

interface Opts { tree?: Record<string, Listing>; ui?: Partial<UiState>; start?: Session['start']; pick?: () => Promise<string | null> | null }

function setup(o: Opts = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const tree = o.tree ?? { '/photos': listing('/photos', 4, ['2024', '2025']), '/photos/2024': listing('/photos/2024', 3, [], '/photos') };
  const api = {
    session: vi.fn(async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: o.start ?? null })),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async (dir: string) => {
      const l = tree[dir];
      if (!l) throw new ApiError('not_found', dir);
      return l;
    }),
    uiState: vi.fn(async () => ({ ...defaultUiState(), ...o.ui })),
    saveUiState: vi.fn(async () => {}),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  if (o.pick) host.pickFolder = o.pick as () => Promise<string | null>;
  const viewer = { env, loader: () => new Promise<HTMLImageElement>(() => {}) };
  return { api, host, run: (): Promise<App> => boot({ api: api as unknown as Api, host, viewer }) };
}

const key = (k: string, init: KeyboardEventInit = {}): void => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...init }));
};
const cells = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('.gc')];
const selected = (): string => document.querySelector<HTMLElement>('.gc.sel, .dr.sel')?.dataset['i'] ?? '';

let app: App | null = null;
beforeEach(() => { localStorage.clear(); vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('browser: right-click menu', () => {
  const rightClick = (el: Element): void => {
    el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 50, clientY: 60 }));
  };
  const menuLabels = (): string[] => [...document.querySelectorAll('.br-menu-item')].map((b) => b.textContent ?? '');

  it('an image offers Open and Start slideshow; the slideshow starts from that image', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    const img = cells().find((c) => c.title === 'img00002.jpg');
    expect(img).toBeDefined();
    rightClick(img as HTMLElement);
    expect(menuLabels()).toEqual(['Open', 'Start slideshow from here']);
    expect(selected()).toBe(img?.dataset['i']);
    (document.querySelectorAll<HTMLButtonElement>('.br-menu-item')[1] as HTMLButtonElement).click();
    expect(document.querySelector('.br-menu')).toBeNull();
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 2, slideshow: 'playing' });
  });

  it('a folder offers only Open; Esc closes the menu without leaving the browser', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    rightClick(cells()[0] as HTMLElement);
    expect(menuLabels()).toEqual(['Open']);
    key('Escape');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(document.querySelector('.br-menu')).toBeNull();
    expect(app.ctx.store.get().mode).toBe('browser');
  });
});

describe('browser: recent list', () => {
  it('the Recent header has a Clear button that empties the list', async () => {
    const s = setup({ start: { dir: '/photos', file: null }, ui: { recent: ['/photos/2024', '/old'] } });
    app = await s.run();
    await app.started;
    const clear = document.querySelector<HTMLButtonElement>('.tr-clear');
    expect(clear?.textContent).toBe('Clear');
    clear?.click();
    expect(app.prefs.get().recent).toEqual([]);
    await flush();
    expect(document.querySelector('.tr-clear')).toBeNull(); // the empty section is gone
  });
});
