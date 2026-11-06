// Wheel, mouse and touch input -> actions (docs/ux-spec.md §Mouse, §Touch).
// Package K. Emits Actions only; the one store write is the overlay toggle.
// `zoom_by.anchor` is CSS px from the top-left of the bound element; `pan_by`
// is how far the *image* moves (finger right: dx > 0; wheel down: dy < 0).
//
// Pages turn by tapping the left or right part of the screen, never by a
// sideways swipe: on phones that swipe is also the system's edge-swipe Back,
// and the user kept turning pages by accident (2026-11-06).
//
// Live drag feedback for the viewer (package I): CSS custom properties on the
// bound element while a one-finger touch/pen pull-down runs, removed on
// release or cancel (read them with fallbacks, `var(--drag-y, 0px)`):
//   --drag-y  px, >= 0: pull-down offset
//   --pull    0..1: progress toward PULL_CLOSE_PX, for fading the backdrop
// and `data-dragging="y"`, so the viewer can drop its CSS transition while
// the finger is down and use it again for snap-back.

import type { Action } from './actions';
import type { Ctx } from './ctx';
import {
  DOUBLE_TAP_MS, EDGE_CLICK_FRACTION, PULL_CLOSE_PX, TAP_TURN_FRACTION, WHEEL_PAGE_DELTA, ZOOM_STEP,
} from './constants';
import {
  LINE_PX, TAP_SLOP_PX, TapTracker, WheelAccumulator, classifyRelease, pinchStep, type Pt, type Sample,
} from './gesture-math';
export * from './gesture-math';

const NO_GESTURE = 'button, a, input, select, textarea, [data-no-gesture]';

class Gestures {
  private readonly ptrs = new Map<number, Pt>();
  private samples: Sample[] = [];
  private readonly wheel = new WheelAccumulator();
  private readonly taps = new TapTracker();
  private pending: { timer: ReturnType<typeof setTimeout>; run: () => void } | null = null;
  private zoomed = false; // zoom mode is not 'fit': taps never turn pages
  private pans = false; // a drag moves the image
  private panning = false;
  private pinching = false; // set once a second finger lands; ends when all lift
  private panFrom: Pt = { x: 0, y: 0 };

  constructor(private readonly el: HTMLElement, private readonly ctx: Ctx, private readonly overflows: () => boolean) {}

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
    } else if (!this.overflows() || !this.scrolled(-dx, -dy)) {
      // Fitted bigger than the view: the wheel scrolls the page first and
      // turns it only once that edge is reached, like a long comic page.
      const dir = this.wheel.feed(Math.abs(dy) >= Math.abs(dx) ? dy : dx, e.timeStamp);
      if (dir) this.emit({ type: dir > 0 ? 'next' : 'prev' });
    }
  };

  /** Pan; false when the image was already at that edge. */
  private scrolled(dx: number, dy: number): boolean {
    const before = this.ctx.store.get().zoom;
    this.emit({ type: 'pan_by', dx, dy });
    const after = this.ctx.store.get().zoom;
    return after.x !== before.x || after.y !== before.y;
  }

  onDown = (e: PointerEvent): void => {
    if ((e.target as Element | null)?.closest?.(NO_GESTURE)) return;
    if (e.pointerType === 'mouse' && e.button !== 0) return;
    this.el.setPointerCapture?.(e.pointerId);
    this.ptrs.set(e.pointerId, this.rel(e));
    if (this.ptrs.size === 1) {
      const p = this.rel(e);
      this.samples = [{ ...p, t: e.timeStamp }];
      this.zoomed = this.ctx.store.get().zoom.mode !== 'fit';
      this.pans = this.zoomed || this.overflows();
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
    if (this.pans) this.pan(cur);
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
    const r = classifyRelease(this.samples, this.pans);
    this.samples = [];
    // Pull-to-close is touch/pen only; a mouse drag in fit mode does nothing.
    if (r.kind === 'tap') this.onTap(e.pointerType === 'mouse', end);
    else if (r.kind === 'pull' && e.pointerType !== 'mouse') this.emit({ type: 'escape' });
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
    // Sideways does nothing, so the image must not follow it either.
    if (Math.abs(dx) >= Math.abs(dy)) return this.clearFeedback();
    const pull = Math.max(0, dy);
    this.el.dataset.dragging = 'y';
    this.el.style.setProperty('--drag-y', `${pull}px`);
    this.el.style.setProperty('--pull', String(Math.min(1, pull / PULL_CLOSE_PX)));
  }

  private clearFeedback(): void {
    delete this.el.dataset.dragging;
    for (const p of ['--drag-y', '--pull']) this.el.style.removeProperty(p);
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
    // A finger gets wide zones (phones have no keys or wheel); a mouse keeps
    // narrow edges so a click in the middle can still double-click to zoom.
    const f = mouse ? EDGE_CLICK_FRACTION : TAP_TURN_FRACTION;
    const side = p.x < w * f ? -1 : p.x > w * (1 - f) ? 1 : 0;
    const edge = this.zoomed ? 0 : side;
    if (edge) {
      // Immediate: waiting DOUBLE_TAP_MS put 300 ms on every edge page turn,
      // and edge zones never zoom, so there is nothing to disambiguate.
      this.cancelPending();
      this.taps.reset();
      this.emit({ type: edge > 0 ? 'next' : 'prev' });
      return;
    }
    if (kind === 'double') {
      this.cancelPending();
      this.emit({ type: 'zoom_toggle', anchor: { x: p.x, y: p.y } });
      return;
    }
    if (!mouse) {
      const { store } = this.ctx;
      this.defer(() => store.set({ overlayVisible: !store.get().overlayVisible }));
    }
  }

  dispose(): void { this.cancelPending(); this.clearFeedback(); }
}

/** `overflows`: the image in fit mode is bigger than the view right now
 * (fit width/height, a panorama), so drags and the wheel move it. */
export function bindGestures(el: HTMLElement, ctx: Ctx, overflows: () => boolean = () => false): () => void {
  const g = new Gestures(el, ctx, overflows);
  const prevTouchAction = el.style.touchAction;
  el.style.touchAction = 'none'; // else the webview claims drags/pinch for itself
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
