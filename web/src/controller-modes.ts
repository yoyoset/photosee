// The two screens. Viewer mode is the black stage with its overlay, keys and
// gestures; browser mode is the folder tree and thumbnails. They share the
// store and its listing, so switching never reloads a folder.
//
// Why the viewer parts are mounted and unmounted instead of hidden: the viewer
// subscribes to the store and loads the full image whenever the index
// changes. In browser mode the selection moves the index all the time, and a
// hidden viewer would decode every image the user merely clicked on.

import type { Action } from './actions';
import { mountBrowser, type Browser } from './browser';
import type { Controller } from './controller';
import type { Ctx } from './ctx';
import { bindGestures } from './gestures';
import type { Host } from './host';
import { bindKeys } from './keys';
import { mountOverlay } from './overlay';
import type { SlideshowHandle } from './slideshow-ui';
import type { UiPrefs } from './uistate';
import { mountViewer, type Viewer, type ViewerOptions } from './viewer';
import { bindViewerHistory } from './viewer-history';
import type { PanelHandle } from './help';

export interface ModeDeps {
  ctx: Ctx;
  controller: Controller;
  prefs: UiPrefs;
  host: Host;
  doc: Document;
  win: Window;
  slideshow: SlideshowHandle;
  /** Info / settings / help: closed when leaving the viewer. */
  panels: PanelHandle[];
  viewerOpts?: ViewerOptions | undefined;
}

export interface Modes {
  /** What the zoom actions need; null in browser mode. */
  geometry: Viewer['geometry'];
  /** Mounts the browser on first use. */
  browser(): Browser;
  /** The browser if it was ever mounted (for tests). */
  peekBrowser(): Browser | null;
  dispose(): void;
}

function browserRoot(doc: Document): HTMLElement {
  const found = doc.getElementById('browser');
  if (found) return found;
  const el = doc.createElement('div');
  el.id = 'browser';
  doc.body.append(el);
  return el;
}

class ModeSwitcher implements Modes {
  private viewer: Viewer | null = null;
  private viewerOffs: (() => void)[] = [];
  private browserUi: Browser | null = null;
  private readonly unsub: () => void;

  constructor(private readonly d: ModeDeps) {
    this.apply(d.ctx.store.get().mode);
    this.unsub = d.ctx.store.subscribe((s, prev) => { if (s.mode !== prev.mode) this.apply(s.mode); });
  }

  geometry: Viewer['geometry'] = () => this.viewer?.geometry() ?? null;
  peekBrowser = (): Browser | null => this.browserUi;

  browser = (): Browser => {
    const { d } = this;
    const { nav } = d.controller;
    this.browserUi ??= mountBrowser({
      ctx: d.ctx, prefs: d.prefs, host: d.host, root: browserRoot(d.doc), notices: d.doc.body,
      openDir: (dir, file, announce, onError) => nav.openDir(dir, file, announce, onError),
    });
    return this.browserUi;
  };

  private apply(mode: 'browser' | 'viewer'): void {
    this.d.doc.body.classList.toggle('mode-browser', mode === 'browser');
    if (mode === 'viewer') {
      this.browserUi?.hide();
      this.enterViewer();
    } else {
      this.leaveViewer();
      this.browser().show();
    }
  }

  private enterViewer(): void {
    if (this.viewer) return;
    const { ctx, doc, win, slideshow } = this.d;
    const stage = doc.getElementById('stage') as HTMLElement;
    this.viewer = mountViewer(stage, ctx, {
      ...this.d.viewerOpts,
      onShown: () => slideshow.imageShown(),
      onFailed: () => slideshow.imageFailed(),
    });
    this.viewerOffs = [
      mountOverlay(doc.getElementById('overlay') as HTMLElement, ctx),
      bindKeys(win, ctx.session.platform, (a: Action) => ctx.dispatch(a)),
      bindGestures(stage, ctx),
      bindViewerHistory(win, () => ctx.store.set({ mode: 'browser' })),
    ];
  }

  private leaveViewer(): void {
    if (!this.viewer) return;
    this.d.slideshow.stop();
    for (const p of this.d.panels) p.close();
    for (const off of this.viewerOffs) off();
    this.viewerOffs = [];
    this.viewer.dispose();
    this.viewer = null;
  }

  dispose(): void {
    this.unsub();
    this.leaveViewer();
    this.browserUi?.dispose();
    this.browserUi = null;
  }
}

export function mountModes(d: ModeDeps): Modes {
  return new ModeSwitcher(d);
}
