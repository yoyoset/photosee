// The content pane: owns whichever view (grid or details) is showing, the
// item sequence, and the selection. The selection is a position in `items`
// (folders first, then images); the browser mirrors image selections into
// the store so the viewer opens on the right file.

import type { Api, ImageSource } from './api';
import type { BrowserCmd } from './browser-keys';
import type { BrowserItem } from './browser-items';
import { MetaCache } from './browser-meta';
import { TypeAhead, findPrefix } from './browser-typeahead';
import { DetailsView } from './details';
import { GridView, type ContentView } from './grid';
import { moveIndex, type MoveKey } from './grid-geometry';
import type { UiPrefs, UiState, ViewMode } from './uistate';

export interface ContentDeps {
  src: ImageSource;
  api: Pick<Api, 'meta'>;
  prefs: UiPrefs;
  dpr(): number;
  /** The selection moved (or was cleared). */
  onSelect(item: BrowserItem | null): void;
  onOpen(item: BrowserItem): void;
  /** Dimensions arrived for some images. */
  onDims(): void;
  /** Ctrl+wheel on the grid. */
  zoomThumb(delta: number): void;
  /** A header was clicked in the details list. */
  sortBy(key: UiState['sort']['key']): void;
}

export class ContentHost {
  readonly el = document.createElement('div');
  readonly meta: MetaCache;
  private items: readonly BrowserItem[] = [];
  private cursor = -1;
  private view: ContentView;
  private kind: ViewMode;
  private readonly ahead = new TypeAhead();

  constructor(private readonly d: ContentDeps) {
    this.el.className = 'br-content';
    this.meta = new MetaCache(d.api, () => { this.details?.refreshDims(); d.onDims(); });
    this.kind = d.prefs.get().view;
    this.view = this.create();
    this.el.append(this.view.el);
  }

  get selected(): BrowserItem | null { return this.items[this.cursor] ?? null; }
  get count(): number { return this.items.length; }
  /** Rendered cell/row elements (tests assert the virtualization). */
  get domCount(): number { return this.view.cellCount; }
  get grid(): GridView | null { return this.view instanceof GridView ? this.view : null; }
  get details(): DetailsView | null { return this.view instanceof DetailsView ? this.view : null; }

  private create(): ContentView {
    const p = this.d.prefs.get();
    const hooks = { select: (i: number) => this.select(i), open: (i: number) => this.open(i) };
    this.kind = p.view;
    if (p.view === 'details') {
      return new DetailsView(hooks, { sort: p.sort, meta: this.meta, onSort: this.d.sortBy });
    }
    return new GridView(this.d.src, hooks, {
      size: p.thumb, names: p.names, dpr: this.d.dpr, zoom: this.d.zoomThumb,
    });
  }

  /** `select`: a path to select, or an index; default is the first item. */
  setItems(items: readonly BrowserItem[], o: { keepScroll?: boolean; select?: string | number | null } = {}): void {
    this.items = items;
    this.view.setItems(items, o.keepScroll);
    const want = o.select;
    const at = typeof want === 'string' ? items.findIndex((x) => x.path === want) : (want ?? 0);
    this.select(items.length === 0 ? -1 : Math.min(items.length - 1, Math.max(0, at)));
  }

  select(i: number, o: { reveal?: boolean; center?: boolean } = {}): void {
    this.cursor = i;
    this.view.setSelected(i);
    if (o.center) this.view.center(i);
    else if (o.reveal !== false) this.view.reveal(i);
    const it = this.selected;
    if (it?.kind === 'img' && this.kind === 'grid') this.meta.want([it.path]);
    this.d.onSelect(it);
  }

  open(i: number): void {
    const it = this.items[i];
    if (it) this.d.onOpen(it);
  }

  /** Handle a content-area key; true when it was ours. */
  command(cmd: BrowserCmd, now: number): boolean {
    switch (cmd.t) {
      case 'move': this.move(cmd.key); return true;
      case 'open': this.open(this.cursor); return true;
      case 'type': return this.type(cmd.ch, now);
      default: return false;
    }
  }

  private move(key: MoveKey): void {
    this.ahead.reset();
    const to = moveIndex(this.cursor, key, this.view.cols, this.items.length, this.view.pageRows, this.kind === 'grid');
    if (to >= 0 && to !== this.cursor) this.select(to);
  }

  /** Letters jump to the next name with that prefix; a space only continues a word. */
  private type(ch: string, now: number): boolean {
    const prefix = this.ahead.push(ch, now);
    if (ch === ' ' && prefix.trim() === '') { this.ahead.reset(); return false; }
    const at = findPrefix(this.items.map((x) => x.name), prefix, this.cursor);
    if (at >= 0) this.select(at);
    return true;
  }

  /** Switch view kind / names / header marks to match the prefs. */
  applyPrefs(p: UiState, prev: UiState): void {
    if (p.view !== prev.view) return this.switchView();
    if (p.names !== prev.names) this.grid?.setNames(p.names);
    this.details?.setSort(p.sort.key, p.sort.dir);
  }

  private switchView(): void {
    const had = this.view.el.contains(document.activeElement);
    this.view.dispose();
    this.view = this.create();
    this.el.append(this.view.el);
    this.view.setItems(this.items);
    this.select(this.cursor, { center: true });
    if (had) this.view.focus();
  }

  setThumb(px: number, commit: boolean): void { this.grid?.setSize(px, commit); }
  focus(): void { this.view.focus(); }
  center(): void { this.view.center(this.cursor); }

  dispose(): void {
    this.meta.dispose();
    this.view.dispose();
    this.el.remove();
  }
}
