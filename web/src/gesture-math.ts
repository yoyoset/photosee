// Pure pieces of gestures.ts: thresholds, wheel accumulation, release
// classification, tap pairing, pinch. No DOM, time is passed in.

import {
  DOUBLE_TAP_MS, PULL_CLOSE_PX, PULL_CLOSE_PX_PER_MS, WHEEL_COOLDOWN_MS, WHEEL_PAGE_DELTA,
} from './constants';

// Not in constants.ts (contract file); only local geometry that has no UX meaning.
/** Finger/cursor wobble that still counts as a tap or click. */
export const TAP_SLOP_PX = 10;
/** A second tap this close to the first pairs with it. */
const DOUBLE_TAP_DIST_PX = 40;
/** A flick must still travel this far, or a 5 ms twitch reads as 2 px/ms. */
const FLICK_MIN_PX = 30;
/** Firefox reports lines: a 3-line notch must clear WHEEL_PAGE_DELTA. */
export const LINE_PX = 40;

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

export type Release = { kind: 'tap' } | { kind: 'pull' } | { kind: 'cancel' };

const CANCEL: Release = { kind: 'cancel' };

/** Classify a finished one-pointer gesture from its samples. Pure. When the
 * drag moved the image (`pans`: zoomed, or fitted bigger than the view) only
 * a tap is recognised; a sideways drag is nothing (see the file header). */
export function classifyRelease(s: readonly Sample[], pans: boolean): Release {
  const [a, b] = [s[0], s[s.length - 1]];
  if (!a || !b) return CANCEL;
  // Reach, not net displacement: out-and-back is still a drag, not a tap.
  const reach = Math.max(...s.map((p) => Math.hypot(p.x - a.x, p.y - a.y)));
  if (reach <= TAP_SLOP_PX) return { kind: 'tap' };
  const [dx, dy] = [b.x - a.x, b.y - a.y];
  if (pans || Math.abs(dx) >= Math.abs(dy)) return CANCEL;
  const dt = Math.max(b.t - a.t, 1);
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
  reset(): void { this.last = null; }
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
