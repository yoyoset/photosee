// Virtualized scroller shared by the thumbnail grid and the details list.
// Only the rows in view (plus overscan) exist in the DOM, so 10 000 files cost
// the same as 100. Cells are absolutely positioned inside a sizer that gives
// the scroll bar its length, which keeps scrolling native (wheel, touch,
// drag of the bar) and the item -> position maths in grid-geometry.ts.

import { OVERSCAN_ROWS } from './browser-consts';
import {
  cellPos, scrollToReveal, totalHeight, visibleRange, type Layout, type Range,
} from './grid-geometry';

export interface VirtualOpts {
  /** Geometry for the current width; called on every render. */
  layout(width: number): Layout;
  /** Build the element for item `i`. Placement and `.sel` are set here. */
  make(i: number): HTMLElement;
  /** Called after every render with the visible (not overscanned) range. */
  onRender?(visible: Range): void;
  /** A cell is leaving the DOM: cancel what it started (e.g. an image load). */
  release?(cell: HTMLElement): void;
}

// jsdom has no layout, so sizes read as 0; these keep tests meaningful.
const FALLBACK_W = 800;
const FALLBACK_H = 600;

export class VirtualView {
  readonly el = document.createElement('div');
  private readonly sizer = document.createElement('div');
  private readonly cells = new Map<number, HTMLElement>();
  private count = 0;
  private sel = -1;
  private layout: Layout = { cols: 1, cellW: 1, cellH: 1, margin: 0 };
  private ro: ResizeObserver | null = null;
  private layoutKey = '';

  constructor(private readonly opts: VirtualOpts) {
    this.el.className = 'vv';
    this.el.tabIndex = 0;
    this.sizer.className = 'vv-sizer';
    this.el.append(this.sizer);
    this.el.addEventListener('scroll', () => this.render(), { passive: true });
    if (typeof ResizeObserver === 'function') {
      this.ro = new ResizeObserver(() => this.render());
      this.ro.observe(this.el);
    }
  }

  get cols(): number { return this.layout.cols; }
  get cellH(): number { return this.layout.cellH; }
  get cellCount(): number { return this.cells.size; }
  get viewH(): number { return this.el.clientHeight || FALLBACK_H; }
  /** Whole rows that fit, at least one: the step of PageUp/PageDown. */
  get pageRows(): number { return Math.max(1, Math.floor(this.viewH / this.layout.cellH) - 1); }

  setCount(n: number): void {
    this.count = n;
    this.rebuild();
  }

  /** Drop and recreate the visible cells (content depends on something that changed). */
  rebuild(): void {
    for (const c of this.cells.values()) this.drop(c);
    this.cells.clear();
    this.render();
  }

  select(i: number): void {
    const old = this.cells.get(this.sel);
    old?.classList.remove('sel');
    old?.setAttribute('aria-selected', 'false');
    this.sel = i;
    const c = this.cells.get(i);
    c?.classList.add('sel');
    c?.setAttribute('aria-selected', 'true');
  }

  reveal(i: number): void {
    if (i < 0 || i >= this.count) return;
    this.measure();
    const { y } = cellPos(this.layout, i);
    const top = scrollToReveal(this.el.scrollTop, this.viewH, y, this.layout.cellH, this.layout.margin);
    if (top !== this.el.scrollTop) this.el.scrollTop = top;
    this.render();
  }

  /** Put row `i` in the middle: for returning from the viewer to a far-away image. */
  center(i: number): void {
    if (i < 0 || i >= this.count) return;
    this.measure();
    const { y } = cellPos(this.layout, i);
    this.el.scrollTop = Math.max(0, y - (this.viewH - this.layout.cellH) / 2);
    this.render();
  }

  render(): void {
    this.measure();
    const l = this.layout;
    this.sizer.style.height = `${totalHeight(l, this.count)}px`;
    const { start, end } = visibleRange(l, this.count, this.el.scrollTop, this.viewH, OVERSCAN_ROWS);
    // Repositioning ~100 cells on every scroll tick would be wasted style work.
    const key = `${l.cols}/${l.cellW}/${l.cellH}`;
    const moved = key !== this.layoutKey;
    this.layoutKey = key;
    for (const [i, c] of this.cells) {
      if (i >= start && i < end) { if (moved) this.place(c, i); continue; }
      this.drop(c);
      this.cells.delete(i);
    }
    for (let i = start; i < end; i++) {
      if (this.cells.has(i)) continue;
      const c = this.opts.make(i);
      c.dataset['i'] = String(i);
      c.setAttribute('role', 'option');
      c.setAttribute('aria-selected', String(i === this.sel));
      c.classList.toggle('sel', i === this.sel);
      this.place(c, i);
      this.cells.set(i, c);
      this.el.append(c);
    }
    this.opts.onRender?.(visibleRange(l, this.count, this.el.scrollTop, this.viewH, 0));
  }

  private measure(): void {
    this.layout = this.opts.layout(this.el.clientWidth || FALLBACK_W);
  }

  private drop(c: HTMLElement): void {
    this.opts.release?.(c);
    c.remove();
  }

  private place(c: HTMLElement, i: number): void {
    const { x, y } = cellPos(this.layout, i);
    c.style.transform = `translate(${x}px, ${y}px)`;
    c.style.width = `${this.layout.cellW}px`;
    c.style.height = `${this.layout.cellH}px`;
  }

  indexAt(target: EventTarget | null): number {
    const v = (target as Element | null)?.closest<HTMLElement>('[data-i]')?.dataset['i'];
    return v === undefined ? -1 : Number(v);
  }

  dispose(): void {
    this.ro?.disconnect();
    this.el.remove();
  }
}
