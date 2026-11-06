// Single-image view of the browser: the selected image large on a black pane,
// a filmstrip that floats over the bottom edge and hides itself. It is the
// grid's neighbour, not the viewer: no zoom, no pan, no overlay buttons; Enter
// or a double-click opens the real viewer.

import { OVERLAY_HIDE_MS } from './constants';
import { icon } from './browser-icons';
import type { BrowserItem } from './browser-items';
import type { Ctx } from './ctx';
import { mountFilmstrip } from './filmstrip';
import { bindMenu, type ContentHooks, type ContentView } from './grid';
import { bindTapOpen } from './touch-tap';
import {
  SingleLoader, WheelStepper, displaySize, neighbours, paneBox, type LoadImage,
} from './single-load';

export interface SingleHooks extends ContentHooks {
  /** Wheel: one image back (-1) or forward (+1). */
  step(d: -1 | 1): void;
}

export interface SingleOpts {
  ctx: Ctx;
  dpr(): number;
  /** Test injection: how an image URL becomes a decoded element. */
  load?: LoadImage;
  now?(): number;
}

// jsdom has no layout; these keep the arithmetic meaningful there.
const FALLBACK_W = 800;
const FALLBACK_H = 600;
const RESIZE_DEBOUNCE_MS = 150;

export class SingleView implements ContentView {
  readonly el = document.createElement('div');
  readonly cols = 1;
  readonly pageRows = 1;
  private readonly stage = document.createElement('div');
  private readonly strip = document.createElement('div');
  private readonly loader: SingleLoader;
  private readonly wheel = new WheelStepper();
  private readonly offs: (() => void)[] = [];
  private items: readonly BrowserItem[] = [];
  private sel = -1;
  /** Bumped by every request: an answer for an older one is dropped. */
  private token = 0;
  private shownUrl = '';
  private shown: HTMLImageElement | null = null;
  private hideTimer: ReturnType<typeof setTimeout> | undefined;
  private resizeTimer: ReturnType<typeof setTimeout> | undefined;
  private hovered = false;

  constructor(private readonly hooks: SingleHooks, private readonly o: SingleOpts) {
    this.loader = new SingleLoader(o.ctx.src, o.load);
    this.el.className = 'sv';
    this.el.tabIndex = 0;
    this.stage.className = 'sv-stage';
    this.strip.className = 'sv-strip';
    this.el.append(this.stage, this.strip);
    this.offs.push(mountFilmstrip(this.strip, o.ctx));
    this.bind();
    this.wake();
  }

  get cellCount(): number { return this.shown ? 1 : 0; }

  private bind(): void {
    const { el, strip } = this;
    el.addEventListener('pointermove', () => this.wake());
    strip.addEventListener('pointerenter', () => { this.hovered = true; this.wake(); });
    strip.addEventListener('pointerleave', () => { this.hovered = false; this.wake(); });
    // A finger has no double-click: a tap on the picture opens the viewer
    // (phone review, 2026-11-06); a tap on the filmstrip only picks.
    bindTapOpen(el, () => this.focus(), (e) => {
      if ((e.target as Element).closest('.sv-strip')) this.focus();
      else this.hooks.open(this.sel);
    });
    el.addEventListener('wheel', (e) => {
      if (e.ctrlKey) return;
      e.preventDefault();
      const d = this.wheel.push(e.deltaY, this.o.now?.() ?? Date.now());
      if (d !== 0) this.hooks.step(d);
    }, { passive: false });
    bindMenu(el, () => this.sel, this.hooks);
    if (typeof ResizeObserver === 'function') {
      const ro = new ResizeObserver(() => this.onResize());
      ro.observe(el);
      this.offs.push(() => ro.disconnect());
    }
  }

  setItems(items: readonly BrowserItem[]): void {
    this.items = items;
    this.sel = -1;
    this.clear();
  }

  setSelected(i: number): void {
    this.sel = i;
    this.wake(); // moving through images shows where you are
    void this.show();
  }

  reveal(): void {}
  center(): void {}
  focus(): void { this.el.focus({ preventScroll: true }); }

  private paneCss(): { w: number; h: number } {
    return { w: this.el.clientWidth || FALLBACK_W, h: this.el.clientHeight || FALLBACK_H };
  }

  private async show(): Promise<void> {
    const mine = ++this.token;
    const it = this.items[this.sel];
    if (it?.kind !== 'img') return this.clear();
    const pane = this.paneCss();
    const box = paneBox(pane.w, pane.h, this.o.dpr());
    const url = this.loader.urlFor(it.path, box);
    if (url !== this.shownUrl) {
      this.el.classList.add('loading');
      try {
        const im = await this.loader.fetch(url);
        if (mine !== this.token) return; // the user moved on while this was in flight
        this.display(im, url);
      } catch {
        if (mine === this.token) this.fail();
      }
    }
    if (mine !== this.token) return;
    this.el.classList.remove('loading');
    this.loader.prefetch(neighbours(this.sel, this.items.length)
      .flatMap((j) => { const n = this.items[j]; return n?.kind === 'img' ? [n.path] : []; }), box);
  }

  private display(im: HTMLImageElement, url: string): void {
    this.el.classList.remove('bad');
    this.shown = im;
    this.shownUrl = url;
    im.className = 'sv-img';
    im.draggable = false;
    im.alt = '';
    this.layout();
    this.stage.replaceChildren(im);
  }

  /** Size the shown image for the pane as it is now. */
  private layout(): void {
    const im = this.shown;
    if (!im) return;
    const s = displaySize({ w: im.naturalWidth, h: im.naturalHeight }, this.o.dpr(), this.paneCss());
    // No natural size (a test double, or an SVG without one): let CSS fit it.
    im.style.width = s.w > 0 ? `${s.w}px` : '';
    im.style.height = s.h > 0 ? `${s.h}px` : '';
  }

  private fail(): void {
    this.el.classList.remove('loading');
    this.el.classList.add('bad');
    this.shown = null;
    this.shownUrl = '';
    this.stage.replaceChildren(icon('broken'));
  }

  private clear(): void {
    this.el.classList.remove('loading', 'bad');
    this.shown = null;
    this.shownUrl = '';
    this.stage.replaceChildren();
  }

  private onResize(): void {
    this.layout();
    clearTimeout(this.resizeTimer);
    this.resizeTimer = setTimeout(() => void this.show(), RESIZE_DEBOUNCE_MS);
  }

  /** Show the filmstrip; hide it again after OVERLAY_HIDE_MS of stillness, never while hovered. */
  private wake(): void {
    this.strip.classList.remove('hide');
    clearTimeout(this.hideTimer);
    if (!this.hovered) this.hideTimer = setTimeout(() => this.strip.classList.add('hide'), OVERLAY_HIDE_MS);
  }

  dispose(): void {
    clearTimeout(this.hideTimer);
    clearTimeout(this.resizeTimer);
    this.token++;
    for (const off of this.offs) off();
    this.loader.clear();
    this.el.remove();
  }
}
