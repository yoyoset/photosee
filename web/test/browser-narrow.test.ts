import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type ImageItem, type Listing, type Session, type Settings } from '../src/api';
import { treeLayout } from '../src/browser-drawer';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang, t } from '../src/i18n';
import { defaultUiState } from '../src/uistate';

setLang('en');
const flush = async (): Promise<void> => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
const img = (dir: string, n: string): ImageItem => ({ name: n, path: `${dir}/${n}`, fmt: 'jpeg', size: 1, mtime_ms: 0 });
const L = (dir: string, names: string[], dirs: string[], parent: string | null): Listing => ({
  dir, parent, images: names.map((n) => img(dir, n)), dirs: dirs.map((d) => ({ name: d, path: `${dir}/${d}` })),
});
const TREE: Record<string, Listing> = {
  '/photos': L('/photos', ['a.jpg', 'b.jpg'], ['2024'], null),
  '/photos/2024': L('/photos/2024', ['x.jpg'], [], '/photos'),
};

function setup() {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const api = {
    session: vi.fn(async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: { dir: '/photos', file: null } })),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async (dir: string) => {
      const l = TREE[dir];
      if (!l) throw new ApiError('not_found', dir);
      return l;
    }),
    uiState: vi.fn(async () => defaultUiState()), // tree.visible: true, as a desktop leaves it
    saveUiState: vi.fn(async () => {}),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  return { api, run: (): Promise<App> => boot({ api: api as unknown as Api, host }) };
}

const width = (px: number): void => { Object.defineProperty(window, 'innerWidth', { value: px, configurable: true }); };
const q = (sel: string): HTMLElement => document.querySelector(sel) as HTMLElement;
const treeBtn = (): HTMLElement => q(`.tbar button[aria-label="${t('br.tree')}"]`);
const cell = (i: number): HTMLElement => q(`.gc[data-i="${i}"]`);
const tap = (el: HTMLElement, kind = 'touch'): void => {
  el.dispatchEvent(new PointerEvent('pointerdown', { pointerType: kind, bubbles: true }));
  el.dispatchEvent(new MouseEvent('click', { bubbles: true }));
};

let app: App | null = null;
const wide = window.innerWidth;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; width(wide); document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('treeLayout', () => {
  it('narrow: only the drawer decides, never the splitter', () => {
    expect(treeLayout(true, false, true)).toEqual({ tree: false, split: false, scrim: false });
    expect(treeLayout(true, true, false)).toEqual({ tree: true, split: false, scrim: true });
  });
  it('wide: the persisted pref decides, no scrim', () => {
    expect(treeLayout(false, true, false)).toEqual({ tree: false, split: false, scrim: false });
    expect(treeLayout(false, false, true)).toEqual({ tree: true, split: true, scrim: false });
  });
});

describe('narrow window: the tree is a drawer', () => {
  it('starts closed although the shared pref says visible, and toggling never writes the pref', async () => {
    width(375);
    app = await setup().run();
    await app.started;
    expect(q('#browser').classList.contains('narrow')).toBe(true);
    expect(q('.br-tree').hidden).toBe(true);
    expect(treeBtn().getAttribute('aria-pressed')).toBe('false');
    treeBtn().click();
    expect(q('.br-tree').hidden).toBe(false);
    expect(q('.br-scrim').hidden).toBe(false);
    expect(q('.br-split').hidden).toBe(true);
    expect(treeBtn().getAttribute('aria-pressed')).toBe('true');
    q('.br-scrim').click();
    expect(q('.br-tree').hidden).toBe(true);
    expect(app.prefs.get().tree.visible).toBe(true);
  });

  it('picking a folder in the drawer opens it and closes the drawer', async () => {
    width(375);
    app = await setup().run();
    await app.started;
    treeBtn().click();
    (q('.tr-row .tr-chev:not(.none)')).click(); // expand /photos
    await flush();
    const row = [...document.querySelectorAll<HTMLElement>('.tr-row')].find((r) => r.textContent === '2024') as HTMLElement;
    row.click();
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
    expect(q('.br-tree').hidden).toBe(true);
  });

  it('widening the window brings back the column from the pref', async () => {
    width(375);
    app = await setup().run();
    await app.started;
    width(1200);
    window.dispatchEvent(new Event('resize'));
    expect(q('#browser').classList.contains('narrow')).toBe(false);
    expect(q('.br-tree').hidden).toBe(false);
    expect(q('.br-scrim').hidden).toBe(true);
  });
});

describe('touch: a tap opens', () => {
  it('a finger tap opens a folder; a mouse click only selects', async () => {
    app = await setup().run();
    await app.started;
    tap(cell(1), 'mouse');
    expect(app.ctx.store.get().mode).toBe('browser');
    tap(cell(1));
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 0 });
  });

  it('a habitual double tap does not open twice', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    tap(cell(0)); // folder 2024
    tap(cell(0)); // lands within DOUBLE_TAP_MS: ignored
    cell(0).dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); // a touch dblclick is ignored too
    await flush();
    expect(app.ctx.store.get()).toMatchObject({ mode: 'browser' });
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
    expect(s.api.list.mock.calls.filter(([d]) => d === '/photos/2024')).toHaveLength(1);
  });
});
