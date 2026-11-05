// The details list: one virtualized row per file, columns as in Explorer.
// Dimensions are the one column the listing cannot give; they are filled from
// /meta for the rows in view only (see MetaCache for the request discipline).

import { DETAILS_ROW_PX } from './browser-consts';
import { icon } from './browser-icons';
import type { BrowserItem } from './browser-items';
import type { MetaCache } from './browser-meta';
import type { SortDir, SortKey } from './browser-sort';
import { formatBytes, formatDate, formatDims, typeLabel } from './browser-status';
import { bindMenu, middleLabel, type ContentHooks, type ContentView } from './grid';
import { listLayout } from './grid-geometry';
import { VirtualView } from './grid-virtual';
import { t, type Key } from './i18n';
import { bindTapOpen } from './touch-tap';

export interface DetailsOpts {
  sort: { key: SortKey; dir: SortDir };
  meta: MetaCache;
  /** A header was clicked: the browser decides asc/desc and re-sorts. */
  onSort(key: SortKey): void;
}

const COLUMNS: { id: string; sort: SortKey | null; label: Key }[] = [
  { id: 'name', sort: 'name', label: 'br.sort.name' },
  { id: 'date', sort: 'date', label: 'br.sort.date' },
  { id: 'type', sort: 'type', label: 'br.sort.type' },
  { id: 'size', sort: 'size', label: 'br.sort.size' },
  { id: 'dims', sort: null, label: 'br.col.dims' },
];

export class DetailsView implements ContentView {
  readonly el = document.createElement('div');
  private readonly head = document.createElement('div');
  private readonly view: VirtualView;
  private items: readonly BrowserItem[] = [];

  constructor(hooks: ContentHooks, private readonly o: DetailsOpts) {
    this.el.className = 'dt';
    this.head.className = 'dt-head';
    this.head.addEventListener('click', (e) => {
      const key = (e.target as Element).closest<HTMLElement>('[data-sort]')?.dataset['sort'] as SortKey | undefined;
      if (key) o.onSort(key);
    });
    this.view = new VirtualView({
      layout: (w) => listLayout(w, DETAILS_ROW_PX),
      make: (i) => this.makeRow(i),
      onRender: (r) => {
        // The header sits outside the scroller: leave room for its scrollbar so columns line up.
        this.head.style.paddingRight = `${this.view.el.offsetWidth - this.view.el.clientWidth}px`;
        o.meta.want(this.items.slice(r.start, r.end).flatMap((x) => (x.kind === 'img' ? [x.path] : [])));
      },
    });
    this.view.el.classList.add('details');
    this.view.el.setAttribute('role', 'listbox');
    bindTapOpen(this.view.el, (e) => this.hit(e, hooks.select), (e) => this.hit(e, hooks.open));
    bindMenu(this.view.el, (t) => this.view.indexAt(t), hooks);
    this.el.append(this.head, this.view.el);
    this.setSort(o.sort.key, o.sort.dir);
  }

  get cols(): number { return 1; }
  get pageRows(): number { return this.view.pageRows; }
  get cellCount(): number { return this.view.cellCount; }

  setItems(items: readonly BrowserItem[], keepScroll = false): void {
    this.items = items;
    if (!keepScroll) this.view.el.scrollTop = 0;
    this.view.setCount(items.length);
  }

  setSelected(i: number): void { this.view.select(i); }
  reveal(i: number): void { this.view.reveal(i); }
  center(i: number): void { this.view.center(i); }
  focus(): void { this.view.el.focus({ preventScroll: true }); }
  dispose(): void { this.view.dispose(); this.el.remove(); }

  /** Header buttons show the active column and direction (also read by screen readers). */
  setSort(key: SortKey, dir: SortDir): void {
    this.head.replaceChildren(...COLUMNS.map((c) => {
      const b = document.createElement(c.sort ? 'button' : 'span');
      b.className = `dh dh-${c.id}`;
      b.textContent = t(c.label);
      if (c.sort) {
        (b as HTMLButtonElement).type = 'button';
        b.dataset['sort'] = c.sort;
        if (c.sort === key) {
          b.classList.add('on');
          b.setAttribute('aria-sort', dir === 'asc' ? 'ascending' : 'descending');
          b.append(icon(dir === 'asc' ? 'asc' : 'desc'));
        }
      }
      return b;
    }));
  }

  /** Dimensions arrived for some rows: rewrite the visible cells in place. */
  refreshDims(): void {
    for (const row of this.view.el.querySelectorAll<HTMLElement>('.dr[data-i]')) {
      const it = this.items[Number(row.dataset['i'])];
      const cell = row.querySelector('.dc-dims');
      if (it?.kind === 'img' && cell) cell.textContent = formatDims(this.o.meta.get(it.path));
    }
  }

  private makeRow(i: number): HTMLElement {
    const it = this.items[i] as BrowserItem;
    const row = document.createElement('div');
    row.className = 'dr';
    const name = document.createElement('span');
    name.className = 'dc dc-name';
    name.append(icon(it.kind === 'dir' ? 'folder' : 'image'), middleLabel(it.name));
    const img = it.kind === 'img' ? it.item : null;
    row.append(
      name,
      cell('date', img ? formatDate(img.mtime_ms) : ''),
      cell('type', img ? typeLabel(it.name) : t('br.folder')),
      cell('size', img ? formatBytes(img.size) : ''),
      cell('dims', img ? formatDims(this.o.meta.get(it.path)) : ''),
    );
    return row;
  }

  private hit(e: MouseEvent, f: (i: number) => void): void {
    const i = this.view.indexAt(e.target);
    if (i >= 0) f(i);
  }
}

function cell(id: string, text: string): HTMLElement {
  const c = document.createElement('span');
  c.className = `dc dc-${id}`;
  c.textContent = text;
  return c;
}
