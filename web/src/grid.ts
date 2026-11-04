// The thumbnail grid. Thumbnails only: a cell never asks for a full image
// (docs/packages/BR.md), and folders are glyph cells because a cover image
// would cost a NAS read per folder.

import type { ImageSource } from './api';
import { CELL_LABEL_PX, CELL_PAD_PX, THUMB_WHEEL_STEP_PX } from './browser-consts';
import { icon } from './browser-icons';
import { splitMiddle, type BrowserItem } from './browser-items';
import { gridLayout, thumbRequest } from './grid-geometry';
import { VirtualView } from './grid-virtual';

/** What the browser needs from either content view. */
export interface ContentView {
  readonly el: HTMLElement;
  /** `keepScroll`: same folder, new order (a re-sort) rather than a new folder. */
  setItems(items: readonly BrowserItem[], keepScroll?: boolean): void;
  setSelected(i: number): void;
  /** Scroll just enough to show item `i`. */
  reveal(i: number): void;
  /** Scroll so item `i` is in the middle. */
  center(i: number): void;
  readonly cols: number;
  readonly pageRows: number;
  /** Cells or rows currently in the DOM. */
  readonly cellCount: number;
  focus(): void;
  dispose(): void;
}

export interface ContentHooks {
  select(i: number): void;
  open(i: number): void;
  /** Right-click (or the menu key) on item `i`, at viewport point x, y. */
  menu(i: number, x: number, y: number): void;
}

/** Shared by grid and details: right-click on an item opens our menu. */
export function bindMenu(el: HTMLElement, indexAt: (t: EventTarget | null) => number, hooks: ContentHooks): void {
  el.addEventListener('contextmenu', (e) => {
    const i = indexAt(e.target);
    if (i < 0) return;
    e.preventDefault();
    hooks.menu(i, e.clientX, e.clientY);
  });
}

export interface GridOpts {
  size: number;
  names: boolean;
  dpr(): number;
  /** Ctrl+wheel: ask for a bigger (+) or smaller (-) thumbnail size. */
  zoom(delta: number): void;
}

export function middleLabel(name: string): HTMLElement {
  const { head, tail } = splitMiddle(name);
  const label = document.createElement('span');
  label.className = 'mid';
  label.title = name;
  const h = document.createElement('span');
  h.className = 'mid-h';
  h.textContent = head;
  label.append(h);
  if (tail) {
    const tl = document.createElement('span');
    tl.className = 'mid-t';
    tl.textContent = tail;
    label.append(tl);
  }
  return label;
}

export class GridView implements ContentView {
  private items: readonly BrowserItem[] = [];
  private size: number;
  private names: boolean;
  private readonly view: VirtualView;

  constructor(private readonly src: ImageSource, hooks: ContentHooks, private readonly o: GridOpts) {
    this.size = o.size;
    this.names = o.names;
    this.view = new VirtualView({
      layout: (w) => gridLayout(w, this.size, this.names),
      make: (i) => this.makeCell(i),
      release: (c) => c.querySelector('img')?.removeAttribute('src'),
    });
    const el = this.view.el;
    el.classList.add('grid');
    el.setAttribute('role', 'listbox');
    this.applyVars();
    el.addEventListener('click', (e) => this.hit(e, hooks.select));
    el.addEventListener('dblclick', (e) => this.hit(e, hooks.open));
    bindMenu(el, (t) => this.view.indexAt(t), hooks);
    el.addEventListener('wheel', (e) => this.onWheel(e), { passive: false });
    // 'error' does not bubble: one capture listener instead of one per image.
    el.addEventListener('error', (e) => this.onBroken(e.target), true);
  }

  get el(): HTMLElement { return this.view.el; }
  get cols(): number { return this.view.cols; }
  get pageRows(): number { return this.view.pageRows; }
  /** Cells currently in the DOM (tests assert this stays small). */
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
  dispose(): void { this.view.dispose(); }

  /** During a slider drag only the geometry changes; `commit` (on release)
   * also re-requests thumbnails, because the best edge may have changed. */
  setSize(px: number, commit: boolean): void {
    this.size = px;
    this.applyVars();
    if (commit) this.view.rebuild();
    else this.view.render();
  }

  setNames(on: boolean): void {
    this.names = on;
    this.applyVars();
    this.view.render();
  }

  private applyVars(): void {
    const s = this.view.el.style;
    s.setProperty('--th', `${this.size}px`);
    s.setProperty('--pad', `${CELL_PAD_PX}px`);
    s.setProperty('--label', `${this.names ? CELL_LABEL_PX : 0}px`);
  }

  private makeCell(i: number): HTMLElement {
    const it = this.items[i] as BrowserItem;
    const cell = document.createElement('div');
    cell.className = it.kind === 'dir' ? 'gc dir' : 'gc';
    const thumb = document.createElement('div');
    thumb.className = 'gc-thumb';
    if (it.kind === 'dir') {
      thumb.append(icon('folder'));
    } else {
      const img = document.createElement('img');
      img.alt = '';
      img.draggable = false;
      img.decoding = 'async';
      const req = thumbRequest(this.size, this.o.dpr());
      // 'prefetch': a wall of thumbnails must not outrank the image on screen.
      img.src = 'edge' in req
        ? this.src.thumbUrl(it.path, req.edge)
        : this.src.imageUrl(it.path, { w: req.box, h: req.box }, 'prefetch');
      thumb.append(img);
    }
    cell.append(thumb);
    cell.title = it.name;
    if (this.names) cell.append(this.nameLabel(it));
    return cell;
  }

  private nameLabel(it: BrowserItem): HTMLElement {
    const l = document.createElement('div');
    l.className = 'gc-name';
    l.append(middleLabel(it.name));
    return l;
  }

  private hit(e: MouseEvent, f: (i: number) => void): void {
    const i = this.view.indexAt(e.target);
    if (i >= 0) f(i);
  }

  private onWheel(e: WheelEvent): void {
    if (!e.ctrlKey) return;
    e.preventDefault();
    this.o.zoom(e.deltaY < 0 ? THUMB_WHEEL_STEP_PX : -THUMB_WHEEL_STEP_PX);
  }

  /** A thumbnail that failed (corrupt file, unsupported format) keeps its cell. */
  private onBroken(target: EventTarget | null): void {
    const thumb = (target as Element | null)?.closest('.gc-thumb');
    if (!thumb) return;
    thumb.classList.add('bad');
    thumb.replaceChildren(icon('broken'));
  }
}
