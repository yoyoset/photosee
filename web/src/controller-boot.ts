// Boot: session + config -> mount everything -> first image.
// Kept out of main.ts so tests can run it against a fake Api and a jsdom body.

import type { Action } from './actions';
import { Api, HttpImageSource, type ImageSource, type Listing, type Session, type Settings } from './api';
import { HINT_TOAST_MS } from './constants';
import { createController, type Controller, type Parts } from './controller';
import { setFullscreen, syncFullscreen } from './controller-view';
import type { Ctx } from './ctx';
import { showNasBanner, showToast } from './errors';
import { bindGestures } from './gestures';
import { mountHelp } from './help';
import { pickHost, type Host } from './host';
import { t } from './i18n';
import { mountInfo } from './info';
import { bindKeys } from './keys';
import { folderName, mountOverlay } from './overlay';
import { mountSettings } from './settings';
import { mountSlideshow } from './slideshow-ui';
import { EscStack, Store } from './state';
import { mountViewer, type ViewerOptions } from './viewer';

export interface BootDeps {
  api?: Api;
  src?: ImageSource;
  host?: Host;
  doc?: Document;
  win?: Window;
  /** Test injection (image loader, env). onShown/onFailed are wired by boot. */
  viewer?: ViewerOptions;
}

export interface App {
  ctx: Ctx;
  controller: Controller;
  /** Resolves when the first listing is in (or its failure is on screen). */
  started: Promise<void>;
  dispose(): void;
}

/** One-item listing so the opened file paints before /list answers. The real
 * listing replaces it at the file's index; 'jpeg' is a placeholder that only
 * /list corrects (the viewer does not read `fmt`). */
function provisional(dir: string, file: string): Listing {
  const name = folderName(file); // last path segment, either separator
  return { dir, parent: null, dirs: [], images: [{ name, path: file, fmt: 'jpeg', size: 0, mtime_ms: 0 }] };
}

function hintOnce(host: HTMLElement): void {
  try {
    if (localStorage.getItem('ps.fsHint')) return;
    localStorage.setItem('ps.fsHint', '1');
  } catch {
    // Storage blocked: showing it every launch beats never showing it.
  }
  showToast(host, t('hint.browser_fullscreen'), HINT_TOAST_MS * 2);
}

async function loadSession(api: Api): Promise<[Session, Settings | null]> {
  return Promise.all([api.session(), api.config().catch((e: unknown) => {
    console.warn('config unavailable, using defaults', e); // slideshow/settings fall back
    return null;
  })]);
}

export async function boot(deps: BootDeps = {}): Promise<App> {
  const doc = deps.doc ?? document;
  const win = deps.win ?? window;
  const api = deps.api ?? new Api();

  let session: Session;
  let settings: Settings | null;
  try {
    [session, settings] = await loadSession(api);
  } catch (err) {
    const dismiss = showNasBanner(doc.body, '/api', () => { dismiss(); void boot(deps); }, 'io');
    throw err;
  }

  const host = deps.host ?? pickHost(doc, win);
  const store = new Store();
  store.set({ settings, root: session.start?.dir ?? null, fullscreen: host.isFullscreen() });
  const ctx: Ctx = {
    store, api, src: deps.src ?? new HttpImageSource(), esc: new EscStack(), session,
    dispatch: (a: Action) => controller.dispatch(a),
  };
  const controller = createController(() => ctx, () => mounted.parts);
  const mounted = mountAll(ctx, controller, host, doc, win, deps.viewer);
  return { ctx, controller, started: start(ctx, controller, mounted.parts), dispose: mounted.dispose };
}

function mountAll(
  ctx: Ctx, controller: Controller, host: Host, doc: Document, win: Window, viewerOpts?: ViewerOptions,
): { parts: Parts; dispose: () => void } {
  const body = doc.body;
  const stage = doc.getElementById('stage') as HTMLElement;
  const slideshow = mountSlideshow(ctx, {
    loadNextFolder: () => void controller.nav.changeFolder('next', true), host: body,
  });
  const viewer = mountViewer(stage, ctx, {
    ...viewerOpts,
    onShown: () => slideshow.imageShown(),
    onFailed: () => slideshow.imageFailed(),
  });
  const parts: Parts = {
    viewer, slideshow, host, notices: body,
    info: mountInfo(body, ctx), settings: mountSettings(body, ctx), help: mountHelp(body, ctx),
  };
  const offs = [
    mountOverlay(doc.getElementById('overlay') as HTMLElement, ctx),
    controller.registerEsc(host),
    bindKeys(win, ctx.session.platform, ctx.dispatch),
    bindGestures(stage, ctx),
    host.onFullscreenChange?.(() => syncFullscreen(ctx, host)),
  ];
  return {
    parts,
    dispose() {
      for (const off of offs) off?.();
      slideshow.dispose();
      viewer.dispose();
      for (const p of [parts.info, parts.settings, parts.help]) p.dispose();
    },
  };
}

async function start(ctx: Ctx, controller: Controller, parts: Parts): Promise<void> {
  const { session, store } = ctx;
  if (store.get().settings?.start_fullscreen) {
    if (session.mode === 'server') hintOnce(parts.notices);
    else void setFullscreen(ctx, parts.host, true);
  }
  const first = session.start;
  if (!first) return; // no folder: the empty stage is the empty state
  if (first.file) store.set({ listing: provisional(first.dir, first.file), index: 0 });
  await controller.nav.openDir(first.dir, first.file);
}
