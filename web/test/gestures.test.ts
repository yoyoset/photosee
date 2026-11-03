import { describe, expect, it } from 'vitest';
import {
  DOUBLE_TAP_MS, PULL_CLOSE_PX, SWIPE_FLICK_PX_PER_MS, WHEEL_COOLDOWN_MS,
} from '../src/constants';
import {
  classifyRelease, pinchStep, TapTracker, WheelAccumulator, type Sample,
} from '../src/gestures';

describe('WheelAccumulator', () => {
  it('trackpad stream of 30 events of 8 turns exactly one page', () => {
    const w = new WheelAccumulator();
    const out: number[] = [];
    for (let i = 0; i < 30; i++) out.push(w.feed(8, 1000 + i * 16));
    expect(out.filter((d) => d !== 0)).toEqual([1]);
    expect(out.indexOf(1)).toBe(12); // 13 * 8 = 104 >= 100
  });

  it('every mouse notch (100+) turns one page, in both directions', () => {
    const w = new WheelAccumulator();
    expect(w.feed(100, 0)).toBe(1);
    expect(w.feed(-100, 1000)).toBe(-1);
    expect(w.feed(125, 2000)).toBe(1);
    expect(w.feed(-240, 3000)).toBe(-1);
  });

  it('ignores everything during the cooldown, then works again', () => {
    const w = new WheelAccumulator();
    expect(w.feed(100, 0)).toBe(1);
    expect(w.feed(100, 100)).toBe(0);
    expect(w.feed(100, WHEEL_COOLDOWN_MS - 1)).toBe(0);
    // Ignored events leave no residue: one notch is still exactly one turn.
    expect(w.feed(100, WHEEL_COOLDOWN_MS)).toBe(1);
  });

  it('cooldown-time events are not banked towards the next page', () => {
    const w = new WheelAccumulator();
    w.feed(100, 0);
    for (let t = 10; t < WHEEL_COOLDOWN_MS; t += 10) w.feed(40, t);
    expect(w.feed(40, WHEEL_COOLDOWN_MS)).toBe(0);
  });

  it('a direction change resets the sum', () => {
    const w = new WheelAccumulator();
    expect(w.feed(60, 0)).toBe(0);
    expect(w.feed(-60, 10)).toBe(0); // restarts at -60, not 60 - 60 = 0
    expect(w.feed(-30, 20)).toBe(0);
    expect(w.feed(-40, 30)).toBe(-1); // -60-30-40 = -130
  });

  it('a stale partial sum does not turn a page later', () => {
    const w = new WheelAccumulator();
    expect(w.feed(90, 0)).toBe(0);
    expect(w.feed(20, 5000)).toBe(0);
  });
});

const line = (n: number, dx: number, dy: number, ms: number): Sample[] => (
  Array.from({ length: n + 1 }, (_, i) => ({ x: 500 + (dx * i) / n, y: 300 + (dy * i) / n, t: (ms * i) / n })));

describe('classifyRelease', () => {
  const W = 1000;
  it('swipe by distance: >= 30% of width, either way', () => {
    expect(classifyRelease(line(10, -300, 0, 2000), W, false)).toEqual({ kind: 'swipe', dir: 1 });
    expect(classifyRelease(line(10, 300, 10, 2000), W, false)).toEqual({ kind: 'swipe', dir: -1 });
    expect(classifyRelease(line(10, -290, 0, 2000), W, false)).toEqual({ kind: 'cancel' });
  });

  it('swipe by velocity: a short fast flick turns, a short slow drag does not', () => {
    const fast = 60 / (SWIPE_FLICK_PX_PER_MS * 1.2);
    expect(classifyRelease(line(4, -60, 0, fast), W, false)).toEqual({ kind: 'swipe', dir: 1 });
    expect(classifyRelease(line(4, -60, 0, fast * 4), W, false)).toEqual({ kind: 'cancel' });
    // A 5 ms twitch is fast but tiny.
    expect(classifyRelease(line(2, -20, 0, 5), W, false)).toEqual({ kind: 'cancel' });
  });

  it('pull down to close: by distance and by velocity, never upwards', () => {
    expect(classifyRelease(line(10, 0, PULL_CLOSE_PX, 3000), W, false)).toEqual({ kind: 'pull' });
    expect(classifyRelease(line(10, 0, PULL_CLOSE_PX - 1, 3000), W, false)).toEqual({ kind: 'cancel' });
    expect(classifyRelease(line(4, 5, 60, 60 / 0.2), W, false)).toEqual({ kind: 'pull' });
    expect(classifyRelease(line(4, 0, 60, 60 / 0.05), W, false)).toEqual({ kind: 'cancel' });
    expect(classifyRelease(line(10, 0, -400, 200), W, false)).toEqual({ kind: 'cancel' });
  });

  it('the dominant axis decides', () => {
    expect(classifyRelease(line(10, 150, 400, 300), W, false)).toEqual({ kind: 'pull' });
    expect(classifyRelease(line(10, 400, 150, 300), W, false)).toEqual({ kind: 'swipe', dir: -1 });
  });

  it('a tap stays within the slop, even with a small wobble', () => {
    expect(classifyRelease(line(3, 3, 2, 80), W, false)).toEqual({ kind: 'tap' });
    expect(classifyRelease(line(3, 3, 2, 80), W, true)).toEqual({ kind: 'tap' });
  });

  it('out-and-back is a drag, not a tap', () => {
    const s: Sample[] = [{ x: 0, y: 0, t: 0 }, { x: 80, y: 0, t: 50 }, { x: 0, y: 0, t: 100 }];
    expect(classifyRelease(s, W, false)).toEqual({ kind: 'cancel' });
  });

  it('nothing but taps while zoomed: swipe and pull are pans', () => {
    expect(classifyRelease(line(10, -600, 0, 100), W, true)).toEqual({ kind: 'cancel' });
    expect(classifyRelease(line(10, 0, 400, 100), W, true)).toEqual({ kind: 'cancel' });
  });

  it('empty input and zero width do not throw', () => {
    expect(classifyRelease([], W, false)).toEqual({ kind: 'cancel' });
    expect(classifyRelease(line(10, -300, 0, 2000), 0, false)).toEqual({ kind: 'cancel' });
  });
});

describe('TapTracker', () => {
  it('two close taps inside the window pair into a double', () => {
    const t = new TapTracker();
    expect(t.feed(100, 100, 0)).toBe('single');
    expect(t.feed(110, 105, DOUBLE_TAP_MS)).toBe('double');
    // A pair is consumed: the third tap starts over.
    expect(t.feed(110, 105, DOUBLE_TAP_MS + 50)).toBe('single');
  });

  it('too slow or too far apart is two singles', () => {
    const t = new TapTracker();
    t.feed(100, 100, 0);
    expect(t.feed(100, 100, DOUBLE_TAP_MS + 1)).toBe('single');
    expect(t.feed(400, 100, DOUBLE_TAP_MS + 50)).toBe('single');
  });
});

describe('pinchStep', () => {
  it('spreading by 2x gives factor 2 around the new midpoint', () => {
    const r = pinchStep([{ x: 100, y: 100 }, { x: 200, y: 100 }], [{ x: 50, y: 100 }, { x: 250, y: 100 }]);
    expect(r?.factor).toBeCloseTo(2);
    expect(r?.mid).toEqual({ x: 150, y: 100 });
  });

  it('pinching in gives < 1; unchanged distance gives 1', () => {
    const a = { x: 0, y: 0 };
    const b = { x: 100, y: 0 };
    expect(pinchStep([a, b], [{ x: 25, y: 0 }, { x: 75, y: 0 }])?.factor).toBeCloseTo(0.5);
    expect(pinchStep([a, b], [{ x: 0, y: 50 }, { x: 100, y: 50 }])?.factor).toBeCloseTo(1);
  });

  it('fingers on top of each other give no ratio', () => {
    const p = { x: 5, y: 5 };
    expect(pinchStep([p, p], [{ x: 0, y: 0 }, { x: 50, y: 0 }])).toBeNull();
  });
});
