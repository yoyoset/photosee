import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import type { Api, Listing, Session, Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang, t } from '../src/i18n';
import { defaultUiState, type UiState } from '../src/uistate';
import type { ViewerEnv } from '../src/viewer';

setLang('en');
const env: ViewerEnv = {
  dpr: () => 1, view: () => ({ w: 800, h: 600 }), now: () => Date.now(),
  watch: () => () => {}, watchDrag: () => () => {}, reducedMotion: () => true,
};
const LIST: Listing = {
  dir: '/photos', parent: null,
  dirs: [{ name: 'sub', path: '/photos/sub' }, { name: 'sub2', path: '/photos/sub2' }],
  images: ['a', 'b', 'c', 'd', 'e'].map((n) => ({ name: `${n}.jpg`, path: `/photos/${n}.jpg`, fmt: 'jpeg' as const, size: 1, mtime_ms: 0 })),
};

async function run(ui: Partial<UiState> = { view: 'single' }) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const api = {
    session: async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: { dir: '/photos', file: null } }),
    config: async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false }),
    list: vi.fn(async () => LIST),
    uiState: async () => ({ ...defaultUiState(), ...ui }),
    saveUiState: vi.fn<(s: Record<string, unknown>) => Promise<void>>(async () => {}),
    nav: vi.fn(), meta: () => new Promise(() => {}),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  const viewer = { env, loader: () => new Promise<HTMLImageElement>(() => {}) };
  const app = await boot({ api: api as unknown as Api, host, viewer });
  await app.started;
  return { app, api };
}

const key = (k: string, init: KeyboardEventInit = {}): void => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...init }));
};
const status = (): string => document.querySelector('.st-r')?.textContent ?? '';
const slider = (): HTMLInputElement => document.querySelector('.tb-range') as HTMLInputElement;
const drag = (v: number): void => { slider().value = String(v); slider().dispatchEvent(new Event('input')); };

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('single view: getting there', () => {
  it('dragging the slider past its maximum snaps to single view; dragging back returns to the grid', async () => {
    ({ app } = await run({ thumb: 200 }));
    expect(document.querySelector('.grid')).not.toBeNull();
    expect(slider().max).toBe('648');
    drag(648);
    expect(app.prefs.get().view).toBe('single');
    expect(document.querySelector('.sv')).not.toBeNull();
    expect(document.querySelector('.gc')).toBeNull();
    expect(slider().value).toBe('648');
    expect(document.querySelector('.tb-tick')?.textContent).toBe(t('br.view.single'));
    expect(document.querySelector('.tb-tick.on')).not.toBeNull();
    expect(app.prefs.get().thumb).toBe(200); // the grid size is remembered
    drag(300);
    expect(app.prefs.get()).toMatchObject({ view: 'grid', thumb: 300 });
    expect(document.querySelector('.grid')).not.toBeNull();
    expect(document.querySelector('.tb-tick.on')).toBeNull();
  });

  it('is also the third option of the view buttons', async () => {
    ({ app } = await run({}));
    const btns = [...document.querySelectorAll<HTMLElement>('.tbar button[aria-pressed]')].filter((b) => b.title);
    const single = btns.find((b) => b.title === 'Single') as HTMLElement;
    expect(single).toBeDefined();
    single.click();
    expect(app.prefs.get().view).toBe('single');
    expect(single.getAttribute('aria-pressed')).toBe('true');
  });

  it('is persisted and restored like the other views', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const r = await run({});
    app = r.app;
    drag(648);
    await vi.advanceTimersByTimeAsync(600);
    expect(r.api.saveUiState.mock.calls.at(-1)?.[0]).toMatchObject({ version: 1, view: 'single' });
    vi.useRealTimers();
    app.dispose();
    ({ app } = await run({ view: 'single' }));
    expect(document.querySelector('.sv')).not.toBeNull();
  });
});

describe('single view: moving between images', () => {
  it('opens on the first image, never on a folder', async () => {
    ({ app } = await run());
    expect(status()).toBe('Selected: a.jpg · 1 B');
    expect(app.ctx.store.get().index).toBe(0);
    expect(document.querySelector('.st-l')?.textContent).toBe('5 images · 2 folders');
  });

  it('arrows, PgUp/PgDn, Home and End walk the images and skip the folders', async () => {
    ({ app } = await run());
    const at = (): number => app?.ctx.store.get().index ?? -1;
    key('ArrowRight'); key('ArrowRight');
    expect(at()).toBe(2);
    key('PageDown');
    expect(at()).toBe(3);
    key('PageUp'); key('PageUp');
    expect(at()).toBe(1);
    key('End');
    expect(at()).toBe(4);
    key('ArrowRight'); // the last image: stays
    expect(at()).toBe(4);
    key('Home');
    expect(at()).toBe(0);
    key('ArrowLeft'); // the first image: does not fall into the folders
    expect(at()).toBe(0);
    expect(status()).toContain('a.jpg');
  });

  it('the wheel moves between images', async () => {
    ({ app } = await run());
    document.querySelector('.sv')?.dispatchEvent(new WheelEvent('wheel', { deltaY: 120, cancelable: true, bubbles: true }));
    expect(app.ctx.store.get().index).toBe(1);
  });

  it('a click on a filmstrip thumbnail goes there', async () => {
    ({ app } = await run());
    (document.querySelector('.sv .fs-cell[data-i="3"]') as HTMLElement).click();
    expect(app.ctx.store.get().index).toBe(3);
    expect(status()).toContain('d.jpg');
  });

  it('switching from the grid with a folder selected lands on the first image', async () => {
    ({ app } = await run({}));
    expect(status()).toBe('Selected: sub');
    drag(648);
    expect(status()).toContain('a.jpg');
  });
});

describe('single view: viewer and back', () => {
  it('Enter opens the viewer at that image; Esc returns to single view on the same image', async () => {
    ({ app } = await run());
    key('ArrowRight'); key('ArrowRight');
    key('Enter');
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 2 });
    expect(document.querySelector('#stage img')).not.toBeNull();
    key('ArrowRight'); // paging in the viewer
    expect(app.ctx.store.get().index).toBe(3);
    key('Escape');
    expect(app.ctx.store.get().mode).toBe('browser');
    expect(app.prefs.get().view).toBe('single');
    expect(document.querySelector('.sv')).not.toBeNull();
    expect(status()).toContain('d.jpg');
    key('ArrowLeft');
    expect(app.ctx.store.get().index).toBe(2);
  });

  it('a double-click on the image opens the viewer', async () => {
    ({ app } = await run());
    key('End');
    document.querySelector('.sv')?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(app.ctx.store.get()).toMatchObject({ mode: 'viewer', index: 4 });
  });

  it('folders are reachable through the tree, and the view stays single', async () => {
    ({ app } = await run());
    expect(document.querySelectorAll('.sv')).toHaveLength(1);
    await app.modes.peekBrowser()?.nav.go('/photos/sub');
    expect(document.querySelectorAll('.sv')).toHaveLength(1);
    expect(app.ctx.store.get().listing?.dir).toBe('/photos');
  });
});
