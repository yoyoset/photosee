// Wheel, mouse and touch input -> actions. Package K.

import type { Ctx } from './ctx';

/** Turns a stream of wheel deltas into discrete page turns (trackpads emit
 * dozens of events per swipe). Pure: time is passed in. */
export class WheelAccumulator {
  feed(deltaY: number, nowMs: number): -1 | 0 | 1 {
    void deltaY; void nowMs;
    return 0; // package K
  }
}

export function bindGestures(el: HTMLElement, ctx: Ctx): () => void {
  void el; void ctx;
  return () => {}; // package K
}
