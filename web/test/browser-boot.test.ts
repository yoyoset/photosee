import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type Listing, type Place, type Session, type Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { defaultUiState } from '../src/uistate';
import type { ViewerEnv } from '../src/viewer';

const env: ViewerEnv = {
  dpr: () => 1, view: () => ({ w: 800, h: 600 }), now: () => Date.now(),
  watch: () => () => {}, watchDrag: () => () => {}, reducedMotion: () => true,
};
const L = (dir: string): Listing => ({
  dir, parent: null, dirs: [],
  images: [{ name: 'a.jpg', path: `${dir}/a.jpg`, fmt: 'jpeg', size: 1, mtime_ms: 0 }],
});
const ROOTS: Place[] = [
  { name: 'Desktop', path: 'C:\\Users\\me\\Desktop', kind: 'folder' },
  { name: 'Pictures', path: 'C:\\Users\\me\\Pictures', kind: 'folder' },
  { name: 'C:', path: 'C:\\', kind: 'drive' },
];

interface Opts { start?: Session['start']; ui?: unknown; roots?: Place[]; fails?: string[] }

function setup(o: Opts = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const api = {
    session: vi.fn(async (): Promise<Session> => ({
      ...(session as Session), mode: 'desktop', platform: 'windows', start: o.start ?? null, roots: o.roots ?? ROOTS,
    })),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async (dir: string) => {
      if (o.fails?.includes(dir)) throw new ApiError('not_found', dir);
      return L(dir);
    }),
    uiState: vi.fn(async () => o.ui ?? {}),
    saveUiState: vi.fn<(s: Record<string, unknown>) => Promise<void>>(async () => {}),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  const viewer = { env, loader: () => new Promise<HTMLImageElement>(() => {}) };
  return { api, run: (): Promise<App> => boot({ api: api as unknown as Api, host, viewer }) };
}

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

const dirOf = (a: App): string | undefined => a.ctx.store.get().listing?.dir;
const asked = (s: ReturnType<typeof setup>): string[] => s.api.list.mock.calls.map((c) => c[0]);

describe('boot: where to start', () => {
  it('no start: the last folder from ui-state, in the browser', async () => {
    const s = setup({ ui: { ...defaultUiState(), last: 'D:\\Shots' } });
    app = await s.run();
    await app.started;
    expect(app.ctx.store.get().mode).toBe('browser');
    expect(dirOf(app)).toBe('D:\\Shots');
    expect(asked(s)).toEqual(['D:\\Shots']);
  });

  it('the last folder is gone: Pictures, quietly (no banner, no toast)', async () => {
    const s = setup({ ui: { ...defaultUiState(), last: 'D:\\Gone' }, fails: ['D:\\Gone'] });
    app = await s.run();
    await app.started;
    expect(dirOf(app)).toBe('C:\\Users\\me\\Pictures');
    expect(document.querySelector('.ps-banner, .ps-toast')).toBeNull();
  });

  it('no usable last folder and no Pictures: the first place', async () => {
    const s = setup({ roots: [ROOTS[0] as Place, ROOTS[2] as Place] });
    app = await s.run();
    await app.started;
    expect(dirOf(app)).toBe('C:\\Users\\me\\Desktop');
  });

  it('nothing lists: the browser still opens, empty, with the tree', async () => {
    const s = setup({ fails: ROOTS.map((r) => r.path) });
    app = await s.run();
    await app.started;
    expect(app.ctx.store.get().listing).toBeNull();
    expect(document.querySelector<HTMLElement>('#browser')?.hidden).toBe(false);
    expect(document.querySelectorAll('.tr-row').length).toBe(3);
  });

});

describe('boot: arguments', () => {
  it('a folder argument opens that folder in the browser', async () => {
    const s = setup({ start: { dir: 'E:\\Trip', file: null }, ui: { ...defaultUiState(), last: 'D:\\Shots' } });
    app = await s.run();
    await app.started;
    expect(app.ctx.store.get().mode).toBe('browser');
    expect(asked(s)).toEqual(['E:\\Trip']);
  });

  it('a file argument goes straight to the viewer, painting before /list answers', async () => {
    const s = setup({ start: { dir: 'E:\\Trip', file: 'E:\\Trip\\a.jpg' } });
    app = await s.run();
    expect(app.ctx.store.get().mode).toBe('viewer');
    expect(app.ctx.store.current()?.name).toBe('a.jpg'); // the provisional or the real listing: either way painted
    await app.started;
    expect(document.body.classList.contains('mode-browser')).toBe(false);
    expect(document.querySelector('#stage img')).not.toBeNull();
    expect(app.modes.peekBrowser()).toBeNull(); // the fast path builds no browser UI at all
    expect(document.querySelector('#browser')?.children.length).toBe(0);
  });

  it('Esc from a file start lands in the browser on that folder, same image selected', async () => {
    const s = setup({ start: { dir: 'E:\\Trip', file: 'E:\\Trip\\a.jpg' } });
    app = await s.run();
    await app.started;
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(app.ctx.store.get().mode).toBe('browser');
    expect(document.querySelector('.gc.sel')?.getAttribute('title')).toBe('a.jpg');
    expect(asked(s)).toEqual(['E:\\Trip']);
  });
});

describe('boot: saved layout', () => {
  it('applies a saved layout; junk falls back to defaults', async () => {
    const s = setup({ ui: { version: 1, tree: { visible: false, width: 300 }, view: 'details', thumb: 96 } });
    app = await s.run();
    await app.started;
    expect((document.querySelector('.br-tree') as HTMLElement).hidden).toBe(true);
    expect(document.querySelector('.details')).not.toBeNull();
    expect(app.prefs.get().thumb).toBe(96);
    app.dispose();
    const d = setup({ ui: 'garbage' });
    app = await d.run();
    await app.started;
    expect(app.prefs.get()).toMatchObject({ view: 'grid', thumb: 160, tree: { visible: true, width: 220 } });
    expect(document.querySelector('.grid')).not.toBeNull();
  });

  it('a missing /ui-state endpoint is not fatal', async () => {
    const s = setup();
    s.api.uiState.mockRejectedValue(new ApiError('network', 'down'));
    app = await s.run();
    await app.started;
    expect(app.prefs.get().view).toBe('grid');
    expect(dirOf(app)).toBe('C:\\Users\\me\\Pictures');
  });

  it('navigating records the folder as last and recent, saved with version 1', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const s = setup({ start: { dir: 'E:\\Trip', file: null } });
    app = await s.run();
    await app.started;
    await vi.advanceTimersByTimeAsync(600);
    const saved = s.api.saveUiState.mock.calls.at(-1)?.[0];
    expect(saved).toMatchObject({ version: 1, last: 'E:\\Trip', recent: ['E:\\Trip'] });
    vi.useRealTimers();
  });
});
