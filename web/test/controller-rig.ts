// Shared controller rig: a store on folder /r/a, mocked api, host,
// panels and slideshow, routed through the real controller.
import { vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import type { Api, ImageItem, Listing, Session, Settings } from '../src/api';
import { createController, type Controller, type Parts } from '../src/controller';
import type { Ctx } from '../src/ctx';
import type { Host } from '../src/host';
import { EscStack, Store } from '../src/state';
import type { Geometry } from '../src/zoom';

export const flush = async (): Promise<void> => { for (let i = 0; i < 10; i++) await Promise.resolve(); };
export const toasts = (): string[] => Array.from(document.querySelectorAll('.ps-toast')).map((e) => e.textContent ?? '');

export function listingOf(dir: string, n: number): Listing {
  const images: ImageItem[] = Array.from({ length: n }, (_, i) => ({
    name: `p${i}.jpg`, path: `${dir}/p${i}.jpg`, fmt: 'jpeg', size: 1, mtime_ms: 0,
  }));
  return { dir, parent: null, images, dirs: [] };
}

export const GEO: Geometry = { img: { w: 2000, h: 1000 }, view: { w: 1000, h: 500 }, dpr: 1, fit: 'contain' };

export interface Rig {
  ctx: Ctx; ctl: Controller; parts: Parts; store: Store;
  api: { nav: ReturnType<typeof vi.fn>; list: ReturnType<typeof vi.fn> };
  host: Host & { fs: boolean; close: ReturnType<typeof vi.fn> };
  geo: { value: Geometry | null };
  slideshow: Record<string, ReturnType<typeof vi.fn>>;
}

const panel = () => ({ open: vi.fn(), close: vi.fn(), toggle: vi.fn(), isOpen: vi.fn(() => false), dispose: vi.fn() });

export function rig(opts: { count?: number } = {}): Rig {
  const store = new Store();
  store.set({ root: '/r', listing: listingOf('/r/a', opts.count ?? 3), settings: config as Settings });
  const api = { nav: vi.fn(), list: vi.fn() };
  const host = {
    fs: false,
    setFullscreen(on: boolean) { this.fs = on; },
    isFullscreen() { return this.fs; },
    close: vi.fn(),
  };
  const geo: Rig['geo'] = { value: GEO };
  const slideshow = Object.fromEntries(
    ['toggle', 'start', 'stop', 'imageShown', 'imageFailed', 'folderLoaded', 'noNextFolder'].map((k) => [k, vi.fn()]),
  );
  const ctx: Ctx = {
    store, api: api as unknown as Api, src: { imageUrl: () => '', thumbUrl: () => '' }, esc: new EscStack(),
    session: { ...(session as Session), platform: 'linux' }, dispatch: (a) => ctl.dispatch(a),
  };
  const parts = {
    viewer: { geometry: () => geo.value }, info: panel(), settings: panel(), help: panel(),
    slideshow, host, notices: document.body,
  } as unknown as Parts;
  const ctl = createController(() => ctx, () => parts);
  return { ctx, ctl, parts, store, api, host, geo, slideshow };
}
