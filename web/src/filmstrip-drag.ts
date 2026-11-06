// Dragging the filmstrip by finger or mouse, with a glide after a flick, and
// scrolling it with the wheel. The strip used to sit fixed around the current
// image ("the strip is dead", user, 2026-11-06), so on a phone only the few
// neighbours on screen could be reached.
//
// The drag only shifts the view (`shift`, px added to the centred offset);
// the current image does not change until a thumbnail is tapped. A press that
// travelled more than DRAG_SLOP_PX is a drag, and its click is swallowed so it
// does not also jump to whatever thumbnail it ended on.

import { FILMSTRIP_THUMB_PX } from './constants';

/** Travel before a press on the strip becomes a drag. Geometry, not UX. */
export const DRAG_SLOP_PX = 8;
/** Glide: speed is multiplied by this per ms (≈ 0.3 after 300 ms). */
const FRICTION_PER_MS = 0.996;
/** Glide stops below this speed, px/ms. */
const MIN_SPEED = 0.02;
/** Release speed is measured over the last part of the drag only. */
const SAMPLE_MS = 80;
/** Firefox reports wheel lines: one line in px. */
const LINE_PX = 40;

export interface Sample { x: number; t: number }

/** Keeps at least the first or the last cell at the centre at either end. */
export function clampOffset(off: number, viewportW: number, count: number, stride: number): number {
  const min = FILMSTRIP_THUMB_PX / 2 - viewportW / 2;
  const max = Math.max(min, (count - 1) * stride + min);
  return Math.min(max, Math.max(min, off));
}

/** px/ms over the last SAMPLE_MS of a drag; positive = finger moved right. */
export function releaseSpeed(samples: readonly Sample[]): number {
  const last = samples[samples.length - 1];
  if (!last) return 0;
  const first = samples.find((s) => last.t - s.t <= SAMPLE_MS) ?? last;
  const dt = last.t - first.t;
  return dt > 0 ? (last.x - first.x) / dt : 0;
}

export interface Shiftable {
  getShift(): number;
  /** Sets and renders (clamped); false when the strip did not move (an end). */
  setShift(v: number): boolean;
}

class StripDrag {
  private readonly win: Window;
  private drag: { x: number; base: number; moved: boolean; samples: Sample[] } | null = null;
  private swallow = false;
  private frame = 0;

  constructor(private readonly el: HTMLElement, private readonly host: Shiftable) {
    this.win = el.ownerDocument.defaultView as Window;
  }

  stopGlide = (): void => {
    if (this.frame) this.win.cancelAnimationFrame(this.frame);
    this.frame = 0;
  };

  private glide(v0: number): void {
    const { win, host } = this;
    const reduced = win.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    if (Math.abs(v0) < MIN_SPEED || reduced || !win.requestAnimationFrame) return;
    let v = v0;
    let last = win.performance.now();
    const step = (now: number): void => {
      const dt = Math.min(Math.max(now - last, 0), 50); // a background tab must not jump
      last = now;
      v *= FRICTION_PER_MS ** dt;
      const moved = Math.abs(v) >= MIN_SPEED && host.setShift(host.getShift() + v * dt);
      this.frame = moved ? win.requestAnimationFrame(step) : 0;
    };
    this.frame = win.requestAnimationFrame(step);
  }

  // Window listeners, not pointer capture: with capture the click would land
  // on the strip instead of the thumbnail and a plain tap could not pick it.
  onDown = (e: PointerEvent): void => {
    if (e.button !== 0) return;
    this.stopGlide();
    this.swallow = false;
    this.drag = { x: e.clientX, base: this.host.getShift(), moved: false, samples: [{ x: e.clientX, t: e.timeStamp }] };
    this.listen(true);
  };

  private listen(on: boolean): void {
    const f = on ? this.win.addEventListener.bind(this.win) : this.win.removeEventListener.bind(this.win);
    f('pointermove', this.onMove as EventListener);
    f('pointerup', this.onUp);
    f('pointercancel', this.onUp);
  }

  private onMove = (e: PointerEvent): void => {
    const d = this.drag;
    if (!d) return;
    const dx = e.clientX - d.x;
    if (!d.moved && Math.abs(dx) <= DRAG_SLOP_PX) return;
    d.moved = true;
    this.el.classList.add('dragging');
    this.host.setShift(d.base - dx);
    d.samples.push({ x: e.clientX, t: e.timeStamp });
    if (d.samples.length > 32) d.samples.shift();
  };

  onUp = (): void => {
    const d = this.drag;
    this.drag = null;
    this.listen(false);
    this.el.classList.remove('dragging');
    if (!d?.moved) return;
    this.swallow = true;
    this.glide(-releaseSpeed(d.samples)); // finger right = content right = smaller offset
  };

  onClick = (e: MouseEvent): void => {
    if (!this.swallow) return;
    this.swallow = false;
    e.stopPropagation();
    e.preventDefault();
  };

  onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    e.stopPropagation(); // the single view turns pages on the wheel; over the strip it scrolls the strip
    this.stopGlide();
    const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
    this.host.setShift(this.host.getShift() + d * (e.deltaMode === 1 ? LINE_PX : 1));
  };
}

export function bindStripDrag(el: HTMLElement, host: Shiftable): () => void {
  const d = new StripDrag(el, host);
  el.addEventListener('pointerdown', d.onDown);
  el.addEventListener('click', d.onClick, true);
  el.addEventListener('wheel', d.onWheel, { passive: false });
  return () => {
    d.stopGlide();
    d.onUp();
    el.removeEventListener('pointerdown', d.onDown);
    el.removeEventListener('click', d.onClick, true);
    el.removeEventListener('wheel', d.onWheel);
  };
}
