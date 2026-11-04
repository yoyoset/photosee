import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type Listing, type Session, type Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang } from '../src/i18n';
import { defaultUiState, parseUiState, type UiState } from '../src/uistate';

setLang('en');
const flush = async (): Promise<void> => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
const L = (dir: string, parent: string | null = null): Listing => ({
  dir, parent, dirs: [],
  images: [{ name: 'a.jpg', path: `${dir}/a.jpg`, fmt: 'jpeg', size: 1, mtime_ms: 0 }],
});

function setup(ui: Partial<UiState> = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const api = {
    session: vi.fn(async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: { dir: '/photos', file: null } })),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async (dir: string) => {
      if (!dir.startsWith('/photos')) throw new ApiError('not_found', dir);
      return L(dir, dir === '/photos' ? null : '/photos');
    }),
    uiState: vi.fn(async () => ({ ...defaultUiState(), ...ui })),
    saveUiState: vi.fn(async () => {}),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  return { api, run: (): Promise<App> => boot({ api: api as unknown as Api, host }) };
}

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

const box = (): HTMLElement => document.querySelector('.tr-recent') as HTMLElement;
const heads = (): string[] => [...document.querySelectorAll('.tr-head .tr-title')].map((h) => h.textContent ?? '');
const recents = Array.from({ length: 9 }, (_, i) => `/photos/r${i}`); // + /photos, opened at start

describe('recent box', () => {
  it('has a fixed height of five rows and scrolls inside, however many entries', async () => {
    const s = setup({ recent: recents });
    app = await s.run();
    await app.started;
    expect(box().style.height).toBe('120px'); // 5 rows x 24 px
    expect(box().style.overflowY).toBe('auto');
    expect(box().querySelectorAll('.tr-row').length).toBe(10);
    expect(box().nextElementSibling?.classList.contains('tr-head')).toBe(true); // Places starts right after it
    expect(document.querySelector('.tr-head ~ .tr-recent ~ .tr-head ~ .tr-row')).not.toBeNull();
  });

  it('keeps its size when empty and when it grows, so Places never moves', async () => {
    const s = setup({ recentPaused: true }); // nothing recorded at start: the box begins empty
    app = await s.run();
    await app.started;
    await flush();
    const before = [box().style.height, heads()];
    expect(box().querySelector('.tr-none')).not.toBeNull(); // placeholder, not a collapse
    app.prefs.set({ recentPaused: false });
    for (const p of recents) await app.modes.peekBrowser()?.nav.go(p);
    expect(box().querySelectorAll('.tr-row').length).toBe(9);
    expect([box().style.height, heads()]).toEqual(before);
    // The one thing between the two headers is the fixed box.
    const kids = [...document.querySelector('.tr')?.children ?? []];
    const i = kids.indexOf(box());
    expect(kids[i - 1]?.classList.contains('tr-head')).toBe(true);
    expect(kids[i + 1]?.classList.contains('tr-head')).toBe(true);
  });

  it('keeps its own scroll position when the list re-renders', async () => {
    const s = setup({ recent: recents });
    app = await s.run();
    await app.started;
    box().scrollTop = 40;
    app.prefs.set({ recent: [...recents.slice(0, 8)] });
    expect(box().scrollTop).toBe(40);
  });
});

describe('recent: pause recording', () => {
  const pause = (): HTMLInputElement => document.querySelector('.tr-head .tr-pause input') as HTMLInputElement;

  it('the Recent header has a checkbox before its label, with a tooltip', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    const label = pause().parentElement as HTMLElement;
    expect(pause().type).toBe('checkbox');
    expect(pause().checked).toBe(false);
    expect(label.firstElementChild).toBe(pause()); // the box comes first, the words after it
    expect(label.textContent).toBe('Pause');
    expect(label.title).toBe('Stop adding folders to Recent');
    expect(document.querySelectorAll('.tr-pause').length).toBe(1); // not on Places / Network
  });

  it('checked: opened folders are not added, existing entries and Clear stay, last still moves', async () => {
    const s = setup({ recent: ['/photos/old'] });
    app = await s.run();
    await app.started;
    pause().click();
    expect(app.prefs.get().recentPaused).toBe(true);
    expect(pause().checked).toBe(true); // redrawn from the prefs
    await app.modes.peekBrowser()?.nav.go('/photos/new');
    expect(app.prefs.get().recent).toEqual(['/photos', '/photos/old']); // /photos: recorded at start, before the pause
    expect(app.prefs.get().last).toBe('/photos/new');
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/new');
    (document.querySelector('.tr-clear') as HTMLElement).click();
    expect(app.prefs.get().recent).toEqual([]);
    pause().click(); // unchecked: recording resumes
    await app.modes.peekBrowser()?.nav.go('/photos/again');
    expect(app.prefs.get().recent).toEqual(['/photos/again']);
  });

  it('is persisted as recentPaused, default false, and restored', async () => {
    expect(defaultUiState().recentPaused).toBe(false);
    expect(parseUiState({ version: 1, recentPaused: 'yes' }).recentPaused).toBe(false);
    expect(parseUiState({ version: 1, recentPaused: true }).recentPaused).toBe(true);
    const s = setup({ recentPaused: true });
    app = await s.run();
    await app.started;
    expect(pause().checked).toBe(true);
    await app.modes.peekBrowser()?.nav.go('/photos/x');
    expect(app.prefs.get().recent).toEqual([]);
  });
});

describe('tree width limit', () => {
  const width = (w: number): void => { Object.defineProperty(window, 'innerWidth', { configurable: true, value: w }); };
  const cssW = (): string => (document.querySelector('#browser') as HTMLElement).style.getPropertyValue('--tree-w');

  it('is half of the window at drag time, and 160 px at least', async () => {
    width(1000);
    const s = setup({ tree: { visible: true, width: 220 } });
    app = await s.run();
    await app.started;
    const split = document.querySelector('.br-split') as HTMLElement;
    for (let i = 0; i < 60; i++) split.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    expect(app.prefs.get().tree.width).toBe(500);
    width(500);
    for (let i = 0; i < 60; i++) split.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    expect(app.prefs.get().tree.width).toBe(250);
    width(200); // 50% would be 100: the minimum wins
    for (let i = 0; i < 60; i++) split.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    expect(app.prefs.get().tree.width).toBe(160);
  });

  it('a stored width is clamped on resize but kept, so a bigger window restores it', async () => {
    width(1600);
    const s = setup({ tree: { visible: true, width: 700 } });
    app = await s.run();
    await app.started;
    expect(cssW()).toBe('700px');
    width(800);
    window.dispatchEvent(new Event('resize'));
    expect(cssW()).toBe('400px');
    expect(app.prefs.get().tree.width).toBe(700);
    width(1600);
    window.dispatchEvent(new Event('resize'));
    expect(cssW()).toBe('700px');
  });

  it('a stored width above half of the window is clamped when the browser first shows', async () => {
    width(600);
    const s = setup({ tree: { visible: true, width: 480 } });
    app = await s.run();
    await app.started;
    expect(cssW()).toBe('300px');
  });
});
