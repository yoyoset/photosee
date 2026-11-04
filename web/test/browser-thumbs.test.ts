import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import type { Api, Listing, Session, Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { defaultUiState, type UiState } from '../src/uistate';

const LIST: Listing = {
  dir: '/photos', parent: null, dirs: [],
  images: ['a', 'b', 'c'].map((n) => ({ name: `${n}.jpg`, path: `/photos/${n}.jpg`, fmt: 'jpeg' as const, size: 1, mtime_ms: 0 })),
};

async function run(ui: Partial<UiState>): Promise<App> {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const api = {
    session: async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: { dir: '/photos', file: null } }),
    config: async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false }),
    list: async () => LIST,
    uiState: async () => ({ ...defaultUiState(), ...ui }),
    saveUiState: async () => {}, nav: vi.fn(), meta: () => new Promise(() => {}),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  const app = await boot({ api: api as unknown as Api, host });
  await app.started;
  return app;
}

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => {
  app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks();
  Object.defineProperty(window, 'devicePixelRatio', { configurable: true, value: 1 });
});
const srcs = (): string[] => [...document.querySelectorAll('.gc img')].map((i) => i.getAttribute('src') ?? '');

describe('grid: which endpoint a cell uses', () => {
  it('a small cell (<= 256 device px) keeps using /thumb', async () => {
    app = await run({ thumb: 160 });
    expect(srcs()).toHaveLength(3);
    expect(srcs().every((u) => u.startsWith('/api/v1/thumb?') && u.endsWith('&s=192'))).toBe(true);
  });

  it('a big cell asks for a resized image: the cell box x dpr, quantised, low priority', async () => {
    app = await run({ thumb: 320 });
    expect(srcs().every((u) => u.startsWith('/api/v1/img?p=') && u.endsWith('&w=384&h=384&prio=prefetch'))).toBe(true);
    expect(srcs().some((u) => u.includes('/thumb'))).toBe(false);
  });

  it('dpr counts: 160 px at 2x needs 320 device px, so it also leaves /thumb', async () => {
    Object.defineProperty(window, 'devicePixelRatio', { configurable: true, value: 2 });
    app = await run({ thumb: 160 });
    expect(srcs()[0]).toContain('&w=384&h=384&prio=prefetch');
  });

  it('the maximum size is 640 px; the slider has one more notch', async () => {
    app = await run({ thumb: 640 });
    expect(srcs()[0]).toContain('&w=640&h=640');
    expect((document.querySelector('.tb-range') as HTMLInputElement).max).toBe('648');
    expect(app.prefs.get().thumb).toBe(640);
  });
});
