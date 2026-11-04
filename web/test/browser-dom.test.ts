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

describe('browser: virtualization', () => {
  it('10 000 items keep fewer than 200 cell nodes, also after scrolling to the end', async () => {
    const s = setup({ tree: { '/big': listing('/big', 10_000) }, start: { dir: '/big', file: null } });
    app = await s.run();
    await app.started;
    expect(cells().length).toBeGreaterThan(0);
    expect(cells().length).toBeLessThan(200);
    const sc = document.querySelector<HTMLElement>('.vv') as HTMLElement;
    sc.scrollTop = 480_000; // near the end of ~485 000 px of content
    sc.dispatchEvent(new Event('scroll'));
    expect(cells().length).toBeLessThan(200);
    expect(Number(cells().at(-1)?.dataset['i'])).toBeGreaterThan(9000);
  });

  it('the details list is virtualized too', async () => {
    const s = setup({ tree: { '/big': listing('/big', 10_000) }, start: { dir: '/big', file: null }, ui: { view: 'details' } });
    app = await s.run();
    await app.started;
    const rows = document.querySelectorAll('.dr').length;
    expect(rows).toBeGreaterThan(0);
    expect(rows).toBeLessThan(200);
  });

  it('thumbnails are requested at a stepped edge and only for cells in the DOM', async () => {
    const s = setup({ tree: { '/big': listing('/big', 500) }, start: { dir: '/big', file: null } });
    app = await s.run();
    await app.started;
    const srcs = [...document.querySelectorAll('.gc img')].map((i) => i.getAttribute('src') ?? '');
    expect(srcs.length).toBeGreaterThan(0);
    expect(srcs.every((u) => u.startsWith('/api/v1/thumb?') && u.endsWith('&s=192'))).toBe(true);
    expect(document.querySelectorAll('img').length).toBeLessThan(200);
  });
});

describe('browser: keyboard and modes', () => {
  it('folders come first; arrows move; Enter opens a folder and then an image in the viewer', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    expect(app.ctx.store.get().mode).toBe('browser');
    expect(cells().map((c) => c.title).slice(0, 3)).toEqual(['2024', '2025', 'img00000.jpg']);
    expect(selected()).toBe('0');
    key('ArrowRight');
    key('ArrowRight');
    key('ArrowRight');
    expect(selected()).toBe('3');
    expect(app.ctx.store.get().index).toBe(1);
    key('Enter');
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 1 });
    expect(document.querySelector('#stage img')).not.toBeNull();
    expect(document.body.classList.contains('mode-browser')).toBe(false);
  });

  it('Esc in the viewer returns to the browser with the same image selected', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    key('End');
    key('Enter');
    expect(app.ctx.store.get().mode).toBe('viewer');
    key('ArrowLeft'); // page back in the viewer: index 3 -> 2
    expect(app.ctx.store.get().index).toBe(2);
    key('Escape');
    expect(app.ctx.store.get().mode).toBe('browser');
    expect(document.querySelector('#stage img')).toBeNull(); // viewer unmounted: no decoding while browsing
    expect(document.querySelector<HTMLElement>('#browser')?.hidden).toBe(false);
    expect(selected()).toBe(String(2 + 2)); // two folders come first
    expect(s.api.list).toHaveBeenCalledTimes(1); // switching modes never reloads the folder
    expect(s.host.close).not.toHaveBeenCalled();
  });

  it('browsing does not load full images: the viewer is not mounted in browser mode', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    key('ArrowDown'); key('End'); key('Home');
    expect(document.querySelector('#stage')?.children.length).toBe(0);
  });

});

describe('browser: mouse and keys', () => {
  it('single click selects, double-click opens; a folder double-click navigates', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    const at = (i: number): HTMLElement => document.querySelector(`.gc[data-i="${i}"]`) as HTMLElement;
    at(4).dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(selected()).toBe('4');
    expect(app.ctx.store.get().mode).toBe('browser');
    at(4).dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 2 });
    key('Escape');
    document.querySelector(`.gc[data-i="0"]`)?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
  });

  it('typing jumps to the next name with that prefix', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    vi.useFakeTimers({ toFake: ['Date'] });
    key('2'); // the cursor is on "2024": a single letter goes to the next match
    expect(selected()).toBe('1');
    vi.setSystemTime(Date.now() + 1500); // a pause starts a new word
    for (const ch of 'img00002') key(ch);
    vi.useRealTimers();
    expect(selected()).toBe('4'); // img00002.jpg, after two folders and img00000/1
  });

  it('the viewer key map is inactive in browser mode', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    key('f'); // would toggle fullscreen in the viewer, is type-ahead here
    key('r'); // would rotate
    expect(app.ctx.store.get().rotation).toBe(0);
    expect(app.ctx.store.get().mode).toBe('browser');
  });
});

describe('browser: navigation', () => {
  it('Back, Forward and Up walk folders; Up reselects the folder we came from', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    const b = app.modes.peekBrowser();
    await b?.nav.go('/photos/2024');
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
    expect(b?.nav.canUp).toBe(true);
    key('Backspace'); // up
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos');
    expect(cells()[selected() ? Number(selected()) : 0]?.title).toBe('2024');
    key('ArrowLeft', { altKey: true });
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
    key('ArrowRight', { altKey: true });
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos');
  });

  it('"next folder" in the viewer stays inside the place that contains the folder', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    await app.modes.peekBrowser()?.nav.go('/photos/2024');
    expect(app.ctx.store.get().root).toBe('/photos');
  });
});
