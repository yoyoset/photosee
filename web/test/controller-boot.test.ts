import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import list from '../../contracts/list.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type Listing, type Session, type Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { t } from '../src/i18n';
import type { ViewerEnv } from '../src/viewer';

const flush = async (): Promise<void> => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
const PHOTOS = list as Listing;
const A10 = '/photos/a10.jpg';

const env: ViewerEnv = {
  dpr: () => 1, view: () => ({ w: 800, h: 600 }), now: () => Date.now(),
  watch: () => () => {}, watchDrag: () => () => {}, reducedMotion: () => true,
};

interface Opts {
  start?: Session['start']; mode?: Session['mode']; settings?: Partial<Settings>;
  list?: () => Promise<Listing>;
}

function setup(o: Opts = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="live"></div>';
  const api = {
    session: vi.fn(async (): Promise<Session> => ({
      ...(session as Session), mode: o.mode ?? 'desktop', start: o.start === undefined ? null : o.start,
    })),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), ...o.settings })),
    list: vi.fn(o.list ?? (async () => PHOTOS)),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
  };
  const host: Host & { fs: boolean } = {
    fs: false, setFullscreen(on) { this.fs = on; }, isFullscreen() { return this.fs; }, close: vi.fn(),
  };
  const loaded: string[] = [];
  const viewer = { env, loader: (url: string) => { loaded.push(url); return new Promise<HTMLImageElement>(() => {}); } };
  const run = (): Promise<App> => boot({ api: api as unknown as Api, host, viewer });
  return { api, host, loaded, run };
}

let app: App | null = null;
beforeEach(() => { localStorage.clear(); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); });

describe('boot: start', () => {
  it('paints the start file before /list returns, then lands on its real index', async () => {
    let finish!: (l: Listing) => void;
    const s = setup({ start: { dir: '/photos', file: A10 }, list: () => new Promise((r) => { finish = r; }) });
    app = await s.run();
    // /list is still pending here.
    expect(app.ctx.store.current()?.path).toBe(A10);
    expect(app.ctx.store.get().listing?.images).toHaveLength(1);
    expect(s.loaded.some((u) => u.includes(encodeURIComponent(A10)))).toBe(true);

    finish(PHOTOS);
    await app.started;
    expect(app.ctx.store.get().listing?.images).toHaveLength(2);
    expect(app.ctx.store.get().index).toBe(1);
    expect(app.ctx.store.current()?.path).toBe(A10);
    expect(app.ctx.store.get().root).toBe('/photos');
  });

  it('with only a dir lists it and starts at index 0', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    expect(s.api.list).toHaveBeenCalledWith('/photos');
    expect(app.ctx.store.get().index).toBe(0);
    expect(app.ctx.store.get().listing?.images).toHaveLength(2);
  });

  it('with neither shows the empty stage and lists nothing', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    expect(s.api.list).not.toHaveBeenCalled();
    expect(app.ctx.store.get().listing).toBeNull();
  });

  it('keeps settings from /config in the store', async () => {
    const s = setup({ settings: { interval_s: 9 } });
    app = await s.run();
    expect(app.ctx.store.get().settings?.interval_s).toBe(9);
  });
});

describe('boot: errors', () => {
  it('a /list timeout shows the NAS banner; retry loads and clears it', async () => {
    const calls = [async () => { throw new ApiError('timeout', 'slow'); }, async () => PHOTOS];
    const s = setup({ start: { dir: '/photos', file: null }, list: () => (calls.shift() as () => Promise<Listing>)() });
    app = await s.run();
    await app.started;
    const banner = document.querySelector('.ps-banner');
    expect(banner?.textContent).toContain('/photos');
    expect(app.ctx.store.get().listing).toBeNull();

    (banner?.querySelector('.ps-btn') as HTMLElement).click();
    await flush();
    expect(document.querySelector('.ps-banner')).toBeNull();
    expect(app.ctx.store.get().listing?.images).toHaveLength(2);
  });

  it('a failed /list keeps the provisional image on screen with the banner', async () => {
    const s = setup({
      start: { dir: '/photos', file: A10 }, list: async () => { throw new ApiError('io', 'down'); },
    });
    app = await s.run();
    await app.started;
    expect(document.querySelector('.ps-banner')).not.toBeNull();
    expect(app.ctx.store.current()?.path).toBe(A10);
  });

  it('an empty folder shows the notice', async () => {
    const s = setup({
      start: { dir: '/photos', file: null }, list: async () => ({ ...PHOTOS, images: [] }),
    });
    app = await s.run();
    await app.started;
    expect(document.querySelector('.ps-notice')?.textContent).toBe(t('hint.empty_folder', { key: 'Ctrl+→' }));
  });

  it('a failed /session puts a retry banner up and rejects', async () => {
    const s = setup();
    s.api.session.mockRejectedValue(new ApiError('network', 'offline'));
    await expect(s.run()).rejects.toThrow('offline');
    expect(document.querySelector('.ps-banner')).not.toBeNull();
  });
});

describe('boot: fullscreen', () => {
  it('desktop with start_fullscreen goes fullscreen', async () => {
    const s = setup({ mode: 'desktop' });
    app = await s.run();
    await flush();
    expect(s.host.fs).toBe(true);
    expect(app.ctx.store.get().fullscreen).toBe(true);
  });

  it('browser mode shows the hint once instead', async () => {
    const s = setup({ mode: 'server' });
    app = await s.run();
    await flush();
    expect(s.host.fs).toBe(false);
    expect(document.querySelector('.ps-toast')?.textContent).toBe(t('hint.browser_fullscreen'));
    app.dispose();
    document.querySelector('.ps-toast')?.remove();
    app = await s.run();
    await flush();
    expect(document.querySelector('.ps-toast')).toBeNull();
  });

  it('start_fullscreen off does nothing', async () => {
    const s = setup({ settings: { start_fullscreen: false } });
    app = await s.run();
    await flush();
    expect(s.host.fs).toBe(false);
  });
});

describe('boot: wiring', () => {
  it('keys reach the controller', async () => {
    const s = setup({ start: { dir: '/photos', file: null } });
    app = await s.run();
    await app.started;
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    expect(app.ctx.store.get().index).toBe(1);
  });
});
