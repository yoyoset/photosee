import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type ImageItem, type Listing, type Session, type Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang, t } from '../src/i18n';
import { loginError } from '../src/login';
import { defaultUiState, parseUiState, type UiState } from '../src/uistate';
import type { ViewerEnv } from '../src/viewer';

setLang('en');
const flush = async (): Promise<void> => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
const env: ViewerEnv = {
  dpr: () => 1, view: () => ({ w: 800, h: 600 }), now: () => Date.now(),
  watch: () => () => {}, watchDrag: () => () => {}, reducedMotion: () => true,
};
const img = (dir: string, n: string): ImageItem => ({ name: n, path: `${dir}/${n}`, fmt: 'jpeg', size: 1, mtime_ms: 0 });
const TREE: Record<string, Listing> = {
  '/comics': { dir: '/comics', parent: null, images: [], dirs: [{ name: 'v01.cbz', path: '/comics/v01.cbz' }] },
  '/comics/v01.cbz': { dir: '/comics/v01.cbz', parent: '/comics', images: ['p1.jpg', 'p2.jpg', 'p3.jpg'].map((n) => img('/comics/v01.cbz', n)), dirs: [] },
};

interface Opts { ui?: Partial<UiState>; mode?: Session['mode']; loggedIn?: boolean }

function setup(o: Opts = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  let loggedIn = o.loggedIn ?? true;
  const sess = (): Session => ({
    ...(session as Session), mode: o.mode ?? 'server', start: null,
    roots: [{ name: 'comics', path: '/comics', kind: 'root' }], user: 'dad',
  });
  const api = {
    session: vi.fn(async (): Promise<Session> => {
      if (!loggedIn) throw new ApiError('unauthorized', 'login required');
      return sess();
    }),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async (dir: string) => {
      const l = TREE[dir];
      if (!l) throw new ApiError('not_found', dir);
      return l;
    }),
    uiState: vi.fn(async () => ({ ...defaultUiState(), ...o.ui })),
    saveUiState: vi.fn(async () => {}),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
    users: vi.fn(async () => ['dad', 'mum']),
    login: vi.fn(async (_n: string, pin: string) => {
      if (pin !== '1234') throw new ApiError('unauthorized', 'wrong name or PIN');
      loggedIn = true;
    }),
    logout: vi.fn(async () => {}),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  const viewer = { env, loader: () => new Promise<HTMLImageElement>(() => {}) };
  return { api, run: (): Promise<App> => boot({ api: api as unknown as Api, host, viewer }) };
}

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

const submit = (pin: string): void => {
  (document.querySelector('.lg-pin') as HTMLInputElement).value = pin;
  (document.querySelector('.lg-form') as HTMLFormElement).requestSubmit();
};

describe('login screen', () => {
  it('names as buttons, a wrong PIN says so, the right one boots as that user', async () => {
    const s = setup({ loggedIn: false });
    const booting = s.run();
    await flush();
    const names = [...document.querySelectorAll('.lg-name')].map((b) => b.textContent);
    expect(names).toEqual(['dad', 'mum']);
    (document.querySelector('.lg-name') as HTMLElement).click();
    expect((document.querySelector('.lg-pin') as HTMLInputElement).inputMode).toBe('numeric');
    submit('0000');
    await flush();
    expect(document.querySelector('.lg-msg')?.textContent).toBe(t('login.wrong'));
    submit('1234');
    app = await booting;
    await app.started;
    expect(s.api.login).toHaveBeenLastCalledWith('dad', '1234');
    expect(document.querySelector('.lg')).toBeNull();
    expect(document.querySelector('.tb-user')?.textContent).toBe('dad');
  });

  it('tells a lockout from a wrong PIN', () => {
    expect(loginError(new ApiError('unauthorized', 'too many wrong PINs; try again in 40 s'))).toBe(t('login.locked'));
    expect(loginError(new ApiError('unauthorized', 'wrong name or PIN'))).toBe(t('login.wrong'));
    expect(loginError(new ApiError('network', 'x'))).toBe(t('err.network'));
  });

  it('Switch user logs out', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    (document.querySelector('.tb-user') as HTMLElement).click();
    await flush();
    expect(s.api.logout).toHaveBeenCalled();
  });
});

describe('resume the last page', () => {
  it('opens the viewer at the page that was on screen', async () => {
    app = await setup({ ui: { reading: { dir: '/comics/v01.cbz', file: '/comics/v01.cbz/p2.jpg' } } }).run();
    await app.started;
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 1 });
    expect(app.ctx.store.get().listing?.dir).toBe('/comics/v01.cbz');
  });

  it('a vanished volume falls back to the folder list', async () => {
    app = await setup({ ui: { reading: { dir: '/comics/gone.cbz', file: '/comics/gone.cbz/p1.jpg' } } }).run();
    await app.started;
    expect(app.ctx.store.get()).toMatchObject({ mode: 'browser' });
    expect(app.ctx.store.get().listing?.dir).toBe('/comics');
  });

  it('records each page the viewer shows; desktop records nothing', async () => {
    const openPage3 = async (a: App): Promise<void> => {
      await a.modes.browser().nav.go('/comics/v01.cbz');
      a.ctx.store.set({ index: 2, mode: 'viewer' }); // what a double-click on p3 does
    };
    app = await setup().run();
    await app.started;
    await openPage3(app);
    expect(app.prefs.get().reading).toEqual({ dir: '/comics/v01.cbz', file: '/comics/v01.cbz/p3.jpg' });
    app.dispose();
    app = await setup({ mode: 'desktop' }).run();
    await app.started;
    await openPage3(app);
    expect(app.prefs.get().reading).toBeNull();
  });

  it('ui-state keeps only a well-formed reading', () => {
    const base = { version: 1 };
    expect(parseUiState({ ...base, reading: { dir: '/a', file: '/a/b' } }).reading).toEqual({ dir: '/a', file: '/a/b' });
    expect(parseUiState({ ...base, reading: { dir: '/a' } }).reading).toBeNull();
    expect(parseUiState({ ...base, reading: 'x' }).reading).toBeNull();
  });
});
