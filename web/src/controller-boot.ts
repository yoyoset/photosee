// Boot: session + config -> mount everything -> first image.
// Kept out of main.ts so tests can run it against a fake Api and a jsdom body.

import type { Action } from './actions';
import { Api, ApiError, HttpImageSource, type ImageSource, type Listing, type Session, type Settings } from './api';
import { HINT_TOAST_MS } from './constants';
import { createController, type Controller, type Parts } from './controller';
import type { PanelHandle } from './help';
import { setFullscreen, syncFullscreen } from './controller-view';
import type { Ctx } from './ctx';
import { showNasBanner, showToast } from './errors';
import { mountHelp } from './help';
import { pickHost, type Host } from './host';
import { t } from './i18n';
import { mountInfo } from './info';
import { mountModes, type Modes } from './controller-modes';
import { folderName } from './overlay';
import { mountSettings } from './settings';
import { mountSlideshow } from './slideshow-ui';
import { EscStack, Store } from './state';
import { sortListing } from './browser-sort';
import { loadUiState, UiPrefs, type UiState } from './uistate';
import type { ViewerOptions } from './viewer';
import { showLogin } from './login';
import { recordReading, resumeReading, resumes } from './resume';
import { mountAccount } from './account';

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
  prefs: UiPrefs;
  /** The browser's layout and history; saved through `api.saveUiState`. */
  modes: Modes;
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
  // "Press F" means nothing on a phone, which has no keyboard (2026-11-06).
  if (host.ownerDocument.defaultView?.matchMedia?.('(pointer: coarse)').matches) return;
  try {
    if (localStorage.getItem('ps.fsHint')) return;
    localStorage.setItem('ps.fsHint', '1');
  } catch {
    // Storage blocked: showing it every launch beats never showing it.
  }
  showToast(host, t('hint.browser_fullscreen'), HINT_TOAST_MS * 2);
}

async function loadSession(api: Api): Promise<[Session, Settings | null, UiState]> {
  return Promise.all([api.session(), api.config().catch((e: unknown) => {
    console.warn('config unavailable, using defaults', e); // slideshow/settings fall back
    return null;
  }), loadUiState(api)]);
}

export async function boot(deps: BootDeps = {}): Promise<App> {
  const doc = deps.doc ?? document;
  const win = deps.win ?? window;
  const api = deps.api ?? new Api();

  let session: Session;
  let settings: Settings | null;
  let ui: UiState;
  try {
    [session, settings, ui] = await loadSession(api);
  } catch (err) {
    if (err instanceof ApiError && err.code === 'unauthorized') {
      // Multi-user server, nobody logged in on this device: log in, start over.
      await showLogin(doc, api, await api.users());
      return boot(deps);
    }
    const dismiss = showNasBanner(doc.body, '/api', () => { dismiss(); void boot(deps); }, 'io');
    throw err;
  }

  const host = deps.host ?? pickHost(doc, win);
  const store = new Store();
  // A file argument is the fast path straight into the viewer; anything else
  // (nothing, or a folder) starts in the browser.
  store.set({
    settings, root: session.start?.dir ?? null, fullscreen: host.isFullscreen(),
    mode: session.start?.file ? 'viewer' : 'browser', fit: ui.fit,
  });
  const prefs = new UiPrefs(ui, api);
  const offFit = store.subscribe((s, p) => { if (s.fit !== p.fit) prefs.set({ fit: s.fit }); });
  const ctx: Ctx = {
    store, api, src: deps.src ?? new HttpImageSource(), esc: new EscStack(), session,
    dispatch: (a: Action) => controller.dispatch(a),
  };
  const controller = createController(() => ctx, () => mounted.parts);
  const mounted = mountAll(ctx, controller, prefs, host, doc, win, deps.viewer);
  win.addEventListener('pagehide', () => void prefs.flush());
  const offRecord = resumes(session) ? recordReading(store, prefs) : () => {};
  if (session.user) {
    mountAccount(mounted.modes.browser().toolbar.el, { doc, api, session, reload: () => win.location.reload() });
  }
  return {
    ctx, controller, prefs, modes: mounted.modes,
    started: start(ctx, controller, mounted.parts, mounted.modes, prefs),
    dispose() { offRecord(); offFit(); mounted.dispose(); prefs.dispose(); },
  };
}

function mountAll(
  ctx: Ctx, controller: Controller, prefs: UiPrefs, host: Host, doc: Document, win: Window,
  viewerOpts?: ViewerOptions,
): { parts: Parts; modes: Modes; dispose: () => void } {
  const body = doc.body;
  const slideshow = mountSlideshow(ctx, {
    loadNextFolder: () => void controller.nav.changeFolder('next', true), host: body,
  });
  const panels = [mountInfo(body, ctx), mountSettings(body, ctx), mountHelp(body, ctx)];
  const offs = [
    controller.registerEsc(host),
    host.onFullscreenChange?.(() => syncFullscreen(ctx, host)),
  ];
  // Mounted last: it applies the starting mode, which mounts the viewer or the browser.
  const modes = mountModes({ ctx, controller, prefs, host, doc, win, slideshow, panels, viewerOpts });
  const parts: Parts = {
    viewer: { geometry: () => modes.geometry() }, slideshow, host, notices: body,
    info: panels[0] as PanelHandle, settings: panels[1] as PanelHandle, help: panels[2] as PanelHandle,
    // Every listing arrives in the user's sort order, so the viewer pages in the order the grid shows.
    arrange: (l) => sortListing(l, prefs.get().sort.key, prefs.get().sort.dir),
  };
  return {
    parts,
    modes,
    dispose() {
      for (const off of offs) off?.();
      modes.dispose();
      slideshow.dispose();
      for (const p of panels) p.dispose();
    },
  };
}

/** No argument: the last folder if it still lists, else Pictures, else the
 * first place. Each attempt is quiet; only the last resort stays silent too,
 * because an empty browser with the tree is a fine place to land. */
async function startBrowser(ctx: Ctx, modes: Modes, prefs: UiPrefs): Promise<void> {
  const { nav } = modes.browser();
  if (resumes(ctx.session) && (await resumeReading(ctx.store, prefs, nav))) return;
  const roots = ctx.session.roots;
  const pictures = roots.find((r) => r.kind === 'folder' && r.name.toLowerCase() === 'pictures');
  const tries = [prefs.get().last, pictures?.path, roots[0]?.path];
  for (const dir of tries) {
    if (dir && (await nav.go(dir, { quiet: true })) === null && ctx.store.get().listing) return;
  }
}

async function start(ctx: Ctx, controller: Controller, parts: Parts, modes: Modes, prefs: UiPrefs): Promise<void> {
  const { session, store } = ctx;
  const first = session.start;
  if (store.get().mode === 'viewer' && store.get().settings?.start_fullscreen) {
    if (session.mode === 'server') hintOnce(parts.notices);
    else void setFullscreen(ctx, parts.host, true);
  }
  if (first?.file) {
    store.set({ listing: provisional(first.dir, first.file), index: 0 });
    await controller.nav.openDir(first.dir, first.file);
  } else if (first) {
    await modes.browser().nav.go(first.dir); // a failure is reported like any other
  } else {
    await startBrowser(ctx, modes, prefs);
  }
}
