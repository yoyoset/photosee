// Wheel, mouse and touch input -> actions (docs/ux-spec.md §Mouse, §Touch).
// Package K. Emits Actions only; the one store write is the overlay toggle.
// `zoom_by.anchor` is CSS px from the top-left of the bound element; `pan_by`
// is how far the *image* moves (finger right: dx > 0; wheel down: dy < 0).
//
// Live drag feedback for the viewer (package I): CSS custom properties on the
// bound element while a one-finger touch/pen drag runs in fit mode, removed on
// release or cancel (read them with fallbacks, `var(--drag-x, 0px)`):
//   --drag-x  px, signed: horizontal swipe, the image follows the finger
//   --drag-y  px, >= 0: pull-down offset
//   --pull    0..1: progress toward PULL_CLOSE_PX, for fading the backdrop
// and `data-dragging="x" | "y"`, so the viewer can drop its CSS transition
// while the finger is down and use it again for snap-back.

import type { Action } from './actions';
import type { Ctx } from './ctx';
import {
  DOUBLE_TAP_MS, EDGE_CLICK_FRACTION, PULL_CLOSE_PX, PULL_CLOSE_PX_PER_MS,
  SWIPE_FLICK_PX_PER_MS, SWIPE_PAGE_FRACTION, WHEEL_COOLDOWN_MS, WHEEL_PAGE_DELTA, ZOOM_STEP,
} from './constants';

// Not in constants.ts (contract file); only local geometry that has no UX meaning.
/** Finger/cursor wobble that still counts as a tap or click. */
const TAP_SLOP_PX = 10;
/** A second tap this close to the first pairs with it. */
const DOUBLE_TAP_DIST_PX = 40;
/** A flick must still travel this far, or a 5 ms twitch reads as 2 px/ms. */
const FLICK_MIN_PX = 30;
/** Firefox reports lines: a 3-line notch must clear WHEEL_PAGE_DELTA. */
const LINE_PX = 40;

export interface Pt { x: number; y: number }
export interface Sample extends Pt { t: number }

/** Turns a stream of wheel deltas into discrete page turns (trackpads emit
 * dozens of events per swipe). Pure: time is passed in. +1 = next. */
export class WheelAccumulator {
  private sum = 0;
  private lastT = -Infinity;
  private cooldownUntil = -Infinity;

  feed(deltaY: number, nowMs: number): -1 | 0 | 1 {
    // Stale partial sums would turn a page on a lone tiny nudge seconds later.
    const gap = nowMs - this.lastT;
    this.lastT = nowMs;
    if (nowMs < this.cooldownUntil) return 0;
    if (gap > WHEEL_COOLDOWN_MS || Math.sign(deltaY) !== Math.sign(this.sum)) this.sum = 0;
    this.sum += deltaY;
    if (Math.abs(this.sum) < WHEEL_PAGE_DELTA) return 0;
    const dir = this.sum > 0 ? 1 : -1;
    this.sum = 0;
    this.cooldownUntil = nowMs + WHEEL_COOLDOWN_MS;
    return dir;
  }
}

export type Release =
  | { kind: 'tap' }
  | { kind: 'swipe'; dir: 1 | -1 } // +1 = next (finger moved left)
  | { kind: 'pull' }
  | { kind: 'cancel' };

const CANCEL: Release = { kind: 'cancel' };

/** Classify a finished one-pointer gesture from its samples. Pure. When
 * zoomed, a drag is a pan (handled live) so only a tap is recognised. */
export function classifyRelease(s: readonly Sample[], width: number, zoomed: boolean): Release {
  const [a, b] = [s[0], s[s.length - 1]];
  if (!a || !b) return CANCEL;
  // Reach, not net displacement: out-and-back is still a drag, not a tap.
  const reach = Math.max(...s.map((p) => Math.hypot(p.x - a.x, p.y - a.y)));
  if (reach <= TAP_SLOP_PX) return { kind: 'tap' };
  if (zoomed) return CANCEL;
  const [dx, dy] = [b.x - a.x, b.y - a.y];
  const dt = Math.max(b.t - a.t, 1);
  if (Math.abs(dx) > Math.abs(dy)) {
    const far = width > 0 && Math.abs(dx) >= SWIPE_PAGE_FRACTION * width;
    const fast = Math.abs(dx) >= FLICK_MIN_PX && Math.abs(dx) / dt >= SWIPE_FLICK_PX_PER_MS;
    return far || fast ? { kind: 'swipe', dir: dx < 0 ? 1 : -1 } : CANCEL;
  }
  const far = dy >= PULL_CLOSE_PX;
  const fast = dy >= FLICK_MIN_PX && dy / dt >= PULL_CLOSE_PX_PER_MS;
  return far || fast ? { kind: 'pull' } : CANCEL;
}

/** Pairs taps into double taps. Pure: time and position are passed in. */
export class TapTracker {
  private last: Sample | null = null;

  /** 'double' completes a pair; 'single' may still become the first of one. */
  feed(x: number, y: number, t: number): 'single' | 'double' {
    const l = this.last;
    if (l && t - l.t <= DOUBLE_TAP_MS && Math.hypot(x - l.x, y - l.y) <= DOUBLE_TAP_DIST_PX) {
      this.last = null;
      return 'double';
    }
    this.last = { x, y, t };
    return 'single';
  }
  reset(): void {
    this.last = null;
  }
}

/** Incremental pinch between two frames: scale ratio and the new midpoint.
 * null when the fingers were (nearly) touching, where a ratio is meaningless. */
export function pinchStep(
  prev: readonly [Pt, Pt], cur: readonly [Pt, Pt],
): { factor: number; mid: Pt } | null {
  const d0 = Math.hypot(prev[0].x - prev[1].x, prev[0].y - prev[1].y);
  const d1 = Math.hypot(cur[0].x - cur[1].x, cur[0].y - cur[1].y);
  if (d0 < 1) return null;
  return { factor: d1 / d0, mid: { x: (cur[0].x + cur[1].x) / 2, y: (cur[0].y + cur[1].y) / 2 } };
}

const NO_GESTURE = 'button, a, input, select, textarea, [data-no-gesture]';

class Gestures {
  private readonly ptrs = new Map<number, Pt>();
  private samples: Sample[] = [];
  private readonly wheel = new WheelAccumulator();
  private readonly taps = new TapTracker();
  private pending: { timer: ReturnType<typeof setTimeout>; run: () => void } | null = null;
  private zoomed = false;
  private panning = false;
  private pinching = false; // set once a second finger lands; ends when all lift
  private panFrom: Pt = { x: 0, y: 0 };

  constructor(private readonly el: HTMLElement, private readonly ctx: Ctx) {}

  private emit(a: Action): void { this.ctx.dispatch(a); }
  private rel(e: { clientX: number; clientY: number }): Pt {
    const r = this.el.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  onWheel = (e: WheelEvent): void => {
    if ((e.target as Element | null)?.closest?.('[data-no-gesture]')) return;
    e.preventDefault(); // else Ctrl+wheel zooms the whole webview
    const unit = e.deltaMode === 1 ? LINE_PX : e.deltaMode === 2 ? this.el.clientHeight : 1;
    const [dx, dy] = [e.deltaX * unit, e.deltaY * unit];
    const st = this.ctx.store.get();
    if (e.ctrlKey || st.settings?.wheel === 'zoom') {
      if (dy === 0) return;
      // Proportional: a notch (100) is exactly ZOOM_STEP, pinch streams stay smooth.
      const notches = Math.max(-1, Math.min(1, -dy / WHEEL_PAGE_DELTA));
      this.emit({ type: 'zoom_by', factor: ZOOM_STEP ** notches, anchor: this.rel(e) });
    } else if (st.zoom.mode !== 'fit') {
      this.emit({ type: 'pan_by', dx: -dx, dy: -dy });
    } else {
      const dir = this.wheel.feed(Math.abs(dy) >= Math.abs(dx) ? dy : dx, e.timeStamp);
      if (dir) this.emit({ type: dir > 0 ? 'next' : 'prev' });
    }
  };

  onDown = (e: PointerEvent): void => {
    if ((e.target as Element | null)?.closest?.(NO_GESTURE)) return;
    if (e.pointerType === 'mouse' && e.button !== 0) return;
    this.el.setPointerCapture?.(e.pointerId);
    this.ptrs.set(e.pointerId, this.rel(e));
    if (this.ptrs.size === 1) {
      const p = this.rel(e);
      this.samples = [{ ...p, t: e.timeStamp }];
      this.zoomed = this.ctx.store.get().zoom.mode !== 'fit';
      this.panning = false;
      this.pinching = false;
      this.panFrom = p;
    } else {
      this.pinching = true;
      this.clearFeedback();
    }
  };

  onMove = (e: PointerEvent): void => {
    const prev = this.ptrs.get(e.pointerId);
    if (!prev) return;
    const cur = this.rel(e);
    if (this.pinching) {
      const other = [...this.ptrs.entries()].find(([id]) => id !== e.pointerId)?.[1];
      this.ptrs.set(e.pointerId, cur);
      const step = other && pinchStep([prev, other], [cur, other]);
      if (step && Math.abs(step.factor - 1) > 0.001) {
        this.emit({ type: 'zoom_by', factor: step.factor, anchor: step.mid });
      }
      return;
    }
    this.ptrs.set(e.pointerId, cur);
    this.samples.push({ ...cur, t: e.timeStamp });
    if (this.zoomed) this.pan(cur);
    else if (e.pointerType !== 'mouse') this.followFinger();
  };

  onUp = (e: PointerEvent): void => {
    if (!this.ptrs.delete(e.pointerId)) return;
    if (this.pinching) {
      if (this.ptrs.size === 0) this.pinching = false;
      return;
    }
    const end = { ...this.rel(e), t: e.timeStamp };
    this.samples.push(end);
    this.clearFeedback();
    const r = classifyRelease(this.samples, this.el.getBoundingClientRect().width, this.zoomed);
    this.samples = [];
    // Mouse drags in fit mode do nothing; swipe and pull are touch/pen only.
    if (r.kind === 'tap') this.onTap(e.pointerType === 'mouse', end);
    else if (e.pointerType === 'mouse') return;
    else if (r.kind === 'swipe') this.emit({ type: r.dir > 0 ? 'next' : 'prev' });
    else if (r.kind === 'pull') this.emit({ type: 'escape' });
  };

  onCancel = (e: PointerEvent): void => {
    if (!this.ptrs.delete(e.pointerId)) return;
    this.pinching = this.pinching && this.ptrs.size > 0;
    this.samples = [];
    this.clearFeedback();
  };

  private pan(cur: Pt): void {
    const a = this.samples[0];
    this.panning ||= !!a && Math.hypot(cur.x - a.x, cur.y - a.y) > TAP_SLOP_PX;
    if (!this.panning) return;
    this.emit({ type: 'pan_by', dx: cur.x - this.panFrom.x, dy: cur.y - this.panFrom.y });
    this.panFrom = cur;
  }

  private followFinger(): void {
    const [a, b] = [this.samples[0], this.samples[this.samples.length - 1]];
    if (!a || !b) return;
    const [dx, dy] = [b.x - a.x, b.y - a.y];
    if (Math.hypot(dx, dy) <= TAP_SLOP_PX) return;
    const horizontal = Math.abs(dx) > Math.abs(dy);
    const pull = horizontal ? 0 : Math.max(0, dy);
    this.el.dataset.dragging = horizontal ? 'x' : 'y';
    this.el.style.setProperty('--drag-x', horizontal ? `${dx}px` : '0px');
    this.el.style.setProperty('--drag-y', `${pull}px`);
    this.el.style.setProperty('--pull', String(Math.min(1, pull / PULL_CLOSE_PX)));
  }

  private clearFeedback(): void {
    delete this.el.dataset.dragging;
    for (const p of ['--drag-x', '--drag-y', '--pull']) this.el.style.removeProperty(p);
  }

  /** Run `fn` after the double-tap window unless a second tap cancels it. */
  private defer(fn: () => void): void {
    this.flush();
    this.pending = { timer: setTimeout(() => { this.pending = null; fn(); }, DOUBLE_TAP_MS), run: fn };
  }

  private cancelPending(): void {
    if (this.pending) clearTimeout(this.pending.timer);
    this.pending = null;
  }

  /** A different tap arrived: the earlier one was a single after all. */
  private flush(): void { const p = this.pending; this.cancelPending(); p?.run(); }

  private onTap(mouse: boolean, p: Sample): void {
    const kind = this.taps.feed(p.x, p.y, p.t);
    const w = this.el.getBoundingClientRect().width;
    const side = p.x < w * EDGE_CLICK_FRACTION ? -1 : p.x > w * (1 - EDGE_CLICK_FRACTION) ? 1 : 0;
    const edge = this.zoomed ? 0 : side;
    if (mouse && edge) {
      // Immediate, and every click counts: clicking fast must page fast.
      // Waiting DOUBLE_TAP_MS put 300 ms on every edge page turn; edge zones
      // simply never zoom, so there is nothing to disambiguate.
      this.cancelPending();
      this.taps.reset();
      this.emit({ type: edge > 0 ? 'next' : 'prev' });
      return;
    }
    if (kind === 'double') {
      this.cancelPending();
      this.emit({ type: 'zoom_toggle' });
      return;
    }
    if (!mouse) {
      const { store } = this.ctx;
      this.defer(() => store.set({ overlayVisible: !store.get().overlayVisible }));
    }
  }

  dispose(): void { this.cancelPending(); this.clearFeedback(); }
}

export function bindGestures(el: HTMLElement, ctx: Ctx): () => void {
  const g = new Gestures(el, ctx);
  const prevTouchAction = el.style.touchAction;
  el.style.touchAction = 'none'; // else the webview claims swipes/pinch for itself
  const handlers = {
    wheel: g.onWheel, pointerdown: g.onDown, pointermove: g.onMove, pointerup: g.onUp,
    pointercancel: g.onCancel,
    dragstart: (e: Event) => e.preventDefault(), // native image drag would eat pans
  } as unknown as Record<string, EventListener>;
  // passive:false matters for wheel only: without it preventDefault is ignored
  // and the webview zooms/scrolls itself.
  for (const [type, h] of Object.entries(handlers)) el.addEventListener(type, h, { passive: false });
  return () => {
    g.dispose();
    el.style.touchAction = prevTouchAction;
    for (const [type, h] of Object.entries(handlers)) el.removeEventListener(type, h);
  };
}
