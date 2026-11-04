// Browser mode: toolbar, folder tree, grid / details, status bar. This file
// wires the parts to the shared store and the persisted prefs; each part owns
// its own DOM and logic (see the browser-*, tree*, grid*, details files).
//
// Flow: a navigation goes through BrowserNav -> the loader puts the new
// listing in the store -> `onStore` shows it. Nothing here keeps a second
// copy of the listing except what it last drew.

import './browser.css';
import type { Listing } from './api';
import { ContentHost, type ContentDeps } from './browser-content';
import { itemsOf, type BrowserItem } from './browser-items';
import { buildSkeleton } from './browser-layout';
import { Toolbar } from './browser-toolbar';
import { BrowserNav } from './browser-nav';
import { bindBrowserKeys } from './browser-keys';
import { sortListing } from './browser-sort';
import { countsText, selectionText } from './browser-status';
import { mountSplitter, clampTree } from './browser-splitter';
import type { Ctx } from './ctx';
import { showToast } from './errors';
import { HINT_TOAST_MS } from './constants';
import type { Host } from './host';
import { TreeModel } from './tree-model';
import { TreeView } from './tree';
import type { UiPrefs, UiState } from './uistate';
import { FIT } from './zoom';
import { THUMB_MAX_PX } from './browser-consts';
import { sliderAction } from './grid-geometry';
import { itemEntries, showMenu } from './browser-menu';
import type { Navigator } from './controller-nav';

export interface BrowserDeps {
  ctx: Ctx;
  prefs: UiPrefs;
  host: Host;
  /** The element the browser fills (#browser). */
  root: HTMLElement;
  /** Where toasts go. */
  notices: HTMLElement;
  openDir: Navigator['openDir'];
  dpr?: () => number;
  now?: () => number;
}

export interface Browser {
  readonly nav: BrowserNav;
  readonly content: ContentHost;
  readonly tree: TreeView;
  readonly toolbar: Toolbar;
  show(): void;
  hide(): void;
  dispose(): void;
}

class BrowserImpl implements Browser {
  readonly nav: BrowserNav;
  readonly content: ContentHost;
  readonly tree: TreeView;
  readonly toolbar: Toolbar;
  private readonly model: TreeModel;
  private readonly tree$: HTMLElement;
  private readonly split: HTMLElement;
  private readonly empty: HTMLElement;
  private readonly statusL: HTMLElement;
  private readonly statusR: HTMLElement;
  private readonly offs: (() => void)[] = [];
  private listing: Listing | null = null;
  private active = false;
  private prev: UiState;

  constructor(private readonly d: BrowserDeps) {
    const { ctx, prefs } = d;
    this.prev = prefs.get();
    this.nav = new BrowserNav(ctx, prefs, d.openDir);
    this.model = new TreeModel(ctx.session.roots, (dir) => ctx.api.list(dir));
    this.syncLists();
    this.tree = new TreeView(this.model, {
      open: (p) => void this.nav.go(p),
      remove: (n) => prefs.set(n.section === 'recent'
        ? { recent: prefs.get().recent.filter((x) => x !== n.path) }
        : { network: prefs.get().network.filter((x) => x !== n.path) }),
      clearRecent: () => prefs.set({ recent: [] }),
      recentPaused: () => prefs.get().recentPaused,
      setRecentPaused: (recentPaused) => prefs.set({ recentPaused }),
    });
    this.content = new ContentHost(this.contentActions());
    this.toolbar = this.buildToolbar();
    const sk = buildSkeleton(d.root, this.toolbar.el, this.tree.el, this.content.el);
    ({ tree$: this.tree$, split: this.split, empty: this.empty, statusL: this.statusL, statusR: this.statusR } = sk);
    this.offs.push(
      ctx.store.subscribe((s, p) => { if (this.active && s.listing !== p.listing) this.showListing(); }),
      prefs.subscribe((p) => this.onPrefs(p)),
      mountSplitter(this.split, () => this.treeWidth(), (w) => this.setTreeWidth(w), () => window.innerWidth),
      onWindowResize(() => this.applyLayoutPrefs()),
    );
    this.nav.onChanged = () => this.syncToolbar();
    this.offs.push(bindBrowserKeys(d.root.ownerDocument.defaultView ?? window, {
      active: () => this.active,
      up: () => void this.nav.up(),
      back: () => void this.nav.back(),
      forward: () => void this.nav.forward(),
      address: () => this.toolbar.address.edit(),
      content: (cmd, now) => this.content.command(cmd, now),
      now: d.now ?? (() => Date.now()),
    }));
    this.applyLayoutPrefs();
    this.syncToolbar();
    this.tree.render(); // the places show even when no folder could be opened
  }

  private contentActions(): ContentDeps {
    const { ctx, prefs } = this.d;
    return {
      src: ctx.src, api: ctx.api, prefs, dpr: this.d.dpr ?? (() => window.devicePixelRatio || 1),
      onSelect: (it) => this.onSelect(it),
      onOpen: (it) => this.openItem(it),
      onMenu: (it, x, y) => showMenu(itemEntries(it, () => this.openItem(it),
        () => { this.openItem(it); this.d.ctx.dispatch({ type: 'toggle_slideshow' }); }), x, y),
      onDims: () => this.syncStatus(),
      zoomThumb: (delta) => this.setThumb(Math.min(THUMB_MAX_PX, prefs.get().thumb + delta), true),
      sortBy: (key) => {
        const s = prefs.get().sort;
        prefs.set({ sort: { ...s, key, dir: s.key === key && s.dir === 'asc' ? 'desc' : 'asc' } });
      },
    };
  }

  private buildToolbar(): Toolbar {
    const { prefs, host, notices } = this.d;
    const pick = host.pickFolder ? { pick: () => void this.pick() } : {};
    return new Toolbar({
      ...pick,
      toggleTree: () => prefs.set({ tree: { ...prefs.get().tree, visible: !prefs.get().tree.visible } }),
      back: () => void this.nav.back(),
      forward: () => void this.nav.forward(),
      up: () => void this.nav.up(),
      go: async (path, typed) => {
        const msg = await this.nav.go(path, { typed });
        if (typed && msg === null) this.content.focus();
        else if (!typed && msg) showToast(notices, msg, HINT_TOAST_MS);
        return typed ? msg : null;
      },
      setView: (view) => prefs.set({ view }),
      setSortKey: (key) => prefs.set({ sort: { ...prefs.get().sort, key } }),
      toggleSortDir: () => {
        const s = prefs.get().sort;
        prefs.set({ sort: { ...s, dir: s.dir === 'asc' ? 'desc' : 'asc' } });
      },
      setThumb: (px, commit) => this.setThumb(px, commit),
      toggleNames: () => prefs.set({ names: !prefs.get().names }),
    });
  }

  private async pick(): Promise<void> {
    const dir = await this.d.host.pickFolder?.().catch(() => null);
    if (!dir) return;
    const msg = await this.nav.go(dir, { typed: true });
    if (msg) showToast(this.d.notices, msg, HINT_TOAST_MS);
  }

  show(): void {
    this.active = true;
    this.d.root.hidden = false;
    const s = this.d.ctx.store.get();
    if (s.listing !== this.listing) {
      this.showListing(s.listing?.images[s.index]?.path);
    } else if (s.listing) {
      this.content.select(s.listing.dirs.length + s.index, { center: true });
    }
    this.content.focus();
  }

  hide(): void {
    this.active = false;
    this.d.root.hidden = true;
  }

  /** Draw the store's listing. A new folder starts at the top; the same
   * folder in a new order (a re-sort) keeps the scroll position and selection. */
  private showListing(selectPath?: string): void {
    const l = this.d.ctx.store.get().listing;
    const sameDir = l !== null && this.listing !== null && l.dir === this.listing.dir;
    const keep = this.content.selected?.path;
    const pending = this.nav.pendingSelect;
    this.nav.pendingSelect = null;
    this.listing = l;
    this.content.setItems(itemsOf(l), {
      keepScroll: sameDir,
      select: selectPath ?? (sameDir ? keep : pending) ?? 0,
    });
    if (selectPath) this.content.center();
    this.empty.hidden = this.content.count > 0;
    this.toolbar.address.set(l?.dir ?? null);
    this.tree.setCurrent(l?.dir ?? null);
    this.syncToolbar();
    this.syncStatus();
  }

  private onSelect(it: BrowserItem | null): void {
    // The viewer opens at store.index, so keep it on the selected image.
    const { store } = this.d.ctx;
    if (it?.kind === 'img' && store.get().index !== it.index) store.set({ index: it.index });
    this.syncStatus();
  }

  private openItem(it: BrowserItem): void {
    if (it.kind === 'dir') return void this.nav.go(it.path);
    // One set(): the viewer mounts on the mode change and must already see the index.
    this.d.ctx.store.set({ index: it.index, zoom: FIT, rotation: 0, mode: 'viewer' });
  }

  private resort(p: UiState): void {
    const { store } = this.d.ctx;
    const l = store.get().listing;
    if (!l) return;
    const sorted = sortListing(l, p.sort.key, p.sort.dir);
    const cur = this.content.selected;
    const at = cur?.kind === 'img' ? sorted.images.findIndex((x) => x.path === cur.path) : -1;
    store.set({ listing: sorted, index: at >= 0 ? at : store.get().index });
  }

  private onPrefs(p: UiState): void {
    const prev = this.prev;
    this.prev = p;
    if (p.sort.key !== prev.sort.key || p.sort.dir !== prev.sort.dir) this.resort(p);
    this.content.applyPrefs(p, prev);
    if (p.recent !== prev.recent || p.network !== prev.network || p.recentPaused !== prev.recentPaused) {
      this.syncLists();
      this.tree.render();
    }
    this.applyLayoutPrefs();
    this.syncToolbar();
  }

  private syncLists(): void {
    const p = this.d.prefs.get();
    this.model.setRecent(p.recent);
    this.model.setNetwork(p.network);
  }

  private applyLayoutPrefs(): void {
    const { tree } = this.d.prefs.get();
    this.d.root.style.setProperty('--tree-w', `${this.treeWidth()}px`);
    this.tree$.hidden = this.split.hidden = !tree.visible;
  }

  /** The stored width, limited to half of the window right now. The stored
   * value is kept as is, so a window that grows again gets the wide tree back. */
  private treeWidth(): number {
    return clampTree(this.d.prefs.get().tree.width, window.innerWidth);
  }

  private setTreeWidth(px: number): void {
    this.d.prefs.set({ tree: { ...this.d.prefs.get().tree, width: clampTree(px, window.innerWidth) } });
  }

  private setThumb(px: number, commit: boolean): void {
    const { prefs } = this.d;
    const a = sliderAction(px, prefs.get().view);
    prefs.set({ ...(a.view ? { view: a.view } : {}), ...(a.thumb !== undefined ? { thumb: a.thumb } : {}) });
    if (a.thumb !== undefined) this.content.setThumb(a.thumb, commit);
  }

  private syncToolbar(): void {
    this.toolbar.sync(this.d.prefs.get(), {
      canBack: this.nav.history.canBack, canForward: this.nav.history.canForward, canUp: this.nav.canUp,
    });
  }

  private syncStatus(): void {
    const it = this.content.selected;
    this.statusL.textContent = countsText(this.listing);
    this.statusR.textContent = selectionText(it, it?.kind === 'img' ? this.content.meta.get(it.path) : undefined);
  }

  dispose(): void {
    for (const off of this.offs) off();
    this.content.dispose();
    this.d.root.replaceChildren();
    this.d.root.classList.remove('br');
  }
}

function onWindowResize(f: () => void): () => void {
  window.addEventListener('resize', f);
  return () => window.removeEventListener('resize', f);
}

export function mountBrowser(deps: BrowserDeps): Browser {
  return new BrowserImpl(deps);
}
