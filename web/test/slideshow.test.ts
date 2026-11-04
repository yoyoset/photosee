import { describe, expect, it } from 'vitest';
import { idle, step, type SlideEffect, type SlideEvent, type SlideState } from '../src/slideshow';

type Cfg = { on_end: 'loop' | 'next_folder' | 'stop'; shuffle: boolean };
const plain: Cfg = { on_end: 'loop', shuffle: false };
const shuf: Cfg = { on_end: 'loop', shuffle: true };
const never = (): number => {
  throw new Error('rand must not be used');
};
const NONE: SlideEffect = { type: 'none' };

/** Small deterministic PRNG (mulberry32) so shuffles are reproducible. */
function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const playing = (order: number[], pos = 0): SlideState => ({ phase: 'playing', pos, order });
const run = (s: SlideState, ev: SlideEvent, cfg: Cfg = plain, rand: () => number = never) =>
  step(s, ev, cfg, rand);
const sorted = (a: number[]) => [...a].sort((x, y) => x - y);
const iota = (n: number) => Array.from({ length: n }, (_, i) => i);

describe('off', () => {
  it('start: plays from the current image in identity order, no effect', () => {
    const [s, e] = run(idle, { type: 'start', index: 2, count: 5 });
    expect(s).toEqual({ phase: 'playing', pos: 2, order: [0, 1, 2, 3, 4] });
    expect(e).toEqual(NONE);
    // The first tick shows the image after the current one, not image 1.
    expect(run(s, { type: 'tick' })[1]).toEqual({ type: 'show', index: 3 });
  });
  it('start with shuffle: [index, ...permutation of the rest]', () => {
    const [s, e] = run(idle, { type: 'start', index: 3, count: 6 }, shuf, seeded(1));
    expect(s.phase).toBe('playing');
    expect(s.pos).toBe(0);
    expect(s.order[0]).toBe(3);
    expect(sorted(s.order)).toEqual(iota(6));
    expect(e).toEqual(NONE);
  });
  it('start with count 0 stays off and reports stopped(end)', () => {
    const [s, e] = run(idle, { type: 'start', index: 0, count: 0 });
    expect(s).toEqual(idle);
    expect(e).toEqual({ type: 'stopped', reason: 'end' });
  });
  it('start clamps a bad index and treats NaN count as empty', () => {
    expect(run(idle, { type: 'start', index: 99, count: 3 })[0].order).toEqual([0, 1, 2]);
    expect(run(idle, { type: 'start', index: -4, count: 3 }, shuf, seeded(2))[0].order[0]).toBe(0);
    expect(run(idle, { type: 'start', index: NaN, count: 3 }, shuf, seeded(2))[0].order[0]).toBe(0);
    expect(run(idle, { type: 'start', index: 0, count: NaN })[1]).toEqual({ type: 'stopped', reason: 'end' });
  });
  it.each<SlideEvent>([
    { type: 'tick' }, { type: 'image_failed' }, { type: 'pause' }, { type: 'resume' },
    { type: 'stop' }, { type: 'folder_loaded', count: 3 }, { type: 'no_next_folder' },
  ])('ignores %o', (ev) => {
    expect(run(idle, ev)).toEqual([idle, NONE]);
  });
});

describe('playing: advance', () => {
  it.each<SlideEvent>([{ type: 'tick' }, { type: 'image_failed' }])('%o moves to pos+1 and shows order[pos+1]', (ev) => {
    const [s, e] = run(playing([4, 1, 3], 0), ev);
    expect(s).toEqual(playing([4, 1, 3], 1));
    expect(e).toEqual({ type: 'show', index: 1 });
  });
  it('pos indexes order, not the image list', () => {
    const [s, e] = run(playing([2, 0, 1], 1), { type: 'tick' });
    expect(s.pos).toBe(2);
    expect(e).toEqual({ type: 'show', index: 1 });
  });
});

describe('playing: last position', () => {
  const last = playing([2, 0, 1], 2);
  const both: SlideEvent[] = [{ type: 'tick' }, { type: 'image_failed' }];
  it.each(both)('on_end loop wraps to 0 keeping the order (%o)', (ev) => {
    const [s, e] = run(last, ev, { on_end: 'loop', shuffle: true });
    expect(s).toEqual(playing([2, 0, 1], 0));
    expect(e).toEqual({ type: 'show', index: 2 });
  });
  it.each(both)('on_end next_folder stays playing and asks for a folder (%o)', (ev) => {
    const [s, e] = run(last, ev, { on_end: 'next_folder', shuffle: false });
    expect(s).toEqual(last);
    expect(e).toEqual({ type: 'load_next_folder' });
  });
  it.each(both)('on_end stop goes off with stopped(end) (%o)', (ev) => {
    const [s, e] = run(last, ev, { on_end: 'stop', shuffle: false });
    expect(s).toEqual(idle);
    expect(e).toEqual({ type: 'stopped', reason: 'end' });
  });
  it('single image: loop shows it again, stop ends', () => {
    expect(run(playing([0]), { type: 'tick' })[1]).toEqual({ type: 'show', index: 0 });
    expect(run(playing([0]), { type: 'tick' }, { on_end: 'stop', shuffle: false })[1]).toEqual({
      type: 'stopped', reason: 'end',
    });
  });
});

describe('playing: folder_loaded / no_next_folder', () => {
  const waiting = playing([0, 1], 1);
  const nf: Cfg = { on_end: 'next_folder', shuffle: false };
  it('count > 0 resets to a fresh identity order and shows its first image', () => {
    const [s, e] = run(waiting, { type: 'folder_loaded', count: 3 }, nf);
    expect(s).toEqual(playing([0, 1, 2], 0));
    expect(e).toEqual({ type: 'show', index: 0 });
  });
  it('count > 0 with shuffle: a permutation, first shown is order[0]', () => {
    const [s, e] = run(waiting, { type: 'folder_loaded', count: 8 }, { ...nf, shuffle: true }, seeded(7));
    expect(sorted(s.order)).toEqual(iota(8));
    expect(s.pos).toBe(0);
    expect(e).toEqual({ type: 'show', index: s.order[0] });
  });
  it('shuffled new folder starts from a random image, not always 0', () => {
    const firsts = new Set<number>();
    for (let seed = 0; seed < 30; seed++) {
      firsts.add(run(waiting, { type: 'folder_loaded', count: 8 }, { ...nf, shuffle: true }, seeded(seed))[0].order[0] ?? -1);
    }
    expect(firsts.size).toBeGreaterThan(1);
  });
  it('count 0 asks for the next folder again, state unchanged', () => {
    const [s, e] = run(waiting, { type: 'folder_loaded', count: 0 }, nf);
    expect(s).toEqual(waiting);
    expect(e).toEqual({ type: 'load_next_folder' });
  });
  it('no_next_folder stops with stopped(end)', () => {
    const [s, e] = run(waiting, { type: 'no_next_folder' }, nf);
    expect(s).toEqual(idle);
    expect(e).toEqual({ type: 'stopped', reason: 'end' });
  });
});

describe('pause / resume / stop', () => {
  it('pause then resume keep pos and order; neither emits an effect', () => {
    const p = playing([1, 0, 2], 1);
    const [paused, e1] = run(p, { type: 'pause' });
    expect(paused).toEqual({ ...p, phase: 'paused' });
    expect(e1).toEqual(NONE);
    const [back, e2] = run(paused, { type: 'resume' });
    expect(back).toEqual(p);
    expect(e2).toEqual(NONE);
  });
  it.each<[string, SlideState]>([
    ['playing', playing([0, 1], 1)],
    ['paused', { phase: 'paused', pos: 1, order: [0, 1] }],
  ])('stop while %s goes off with stopped(user)', (_n, s) => {
    expect(run(s, { type: 'stop' })).toEqual([idle, { type: 'stopped', reason: 'user' }]);
  });
  it.each<SlideEvent>([
    { type: 'tick' }, { type: 'image_failed' }, { type: 'pause' }, { type: 'start', index: 0, count: 5 },
    { type: 'folder_loaded', count: 4 }, { type: 'no_next_folder' },
  ])('paused ignores %o', (ev) => {
    const paused: SlideState = { phase: 'paused', pos: 1, order: [0, 1, 2] };
    expect(run(paused, ev)).toEqual([paused, NONE]);
  });
  it.each<SlideEvent>([{ type: 'resume' }, { type: 'start', index: 0, count: 5 }])('playing ignores %o', (ev) => {
    const p = playing([0, 1, 2], 1);
    expect(run(p, ev)).toEqual([p, NONE]);
  });
  it('a folder_loaded dropped while paused self-heals after resume', () => {
    const cfg: Cfg = { on_end: 'next_folder', shuffle: false };
    const paused: SlideState = { phase: 'paused', pos: 1, order: [0, 1] };
    const [s1] = run(paused, { type: 'folder_loaded', count: 3 }, cfg);
    const [s2] = run(s1, { type: 'resume' }, cfg);
    expect(run(s2, { type: 'tick' }, cfg)[1]).toEqual({ type: 'load_next_folder' });
  });
});

describe('shuffle', () => {
  it.each([1, 2, 3, 10, 257])('is a permutation starting with the current index (count %i)', (count) => {
    for (let seed = 0; seed < 20; seed++) {
      const index = seed % count;
      const [s] = run(idle, { type: 'start', index, count }, shuf, seeded(seed));
      expect(s.order).toHaveLength(count);
      expect(s.order[0]).toBe(index);
      expect(sorted(s.order)).toEqual(iota(count));
    }
  });
  it('is deterministic for the same seed and varies across seeds', () => {
    const go = (seed: number) => run(idle, { type: 'start', index: 0, count: 20 }, shuf, seeded(seed))[0].order;
    expect(go(42)).toEqual(go(42));
    expect(go(42)).not.toEqual(go(43));
  });
  it('survives rand() returning exactly 1 or a negative number', () => {
    for (const r of [1, 0.9999999, -1, 0]) {
      const o = run(idle, { type: 'start', index: 2, count: 6 }, shuf, () => r)[0].order;
      expect(sorted(o)).toEqual(iota(6));
      expect(o[0]).toBe(2);
    }
  });
  it('shuffle off never calls rand', () => {
    expect(() => run(idle, { type: 'start', index: 1, count: 9 }, plain, never)).not.toThrow();
  });
});

describe('long run', () => {
  it.each([false, true])('1000 ticks with on_end loop stay valid (shuffle %s)', (shuffle) => {
    const cfg: Cfg = { on_end: 'loop', shuffle };
    const rand = seeded(5);
    let [s] = step(idle, { type: 'start', index: 4, count: 7 }, cfg, rand);
    const order = [...s.order];
    const p0 = s.pos; // 4 in identity order, 0 when shuffled
    for (let i = 0; i < 1000; i++) {
      let e: SlideEffect;
      [s, e] = step(s, { type: i % 3 === 0 ? 'image_failed' : 'tick' }, cfg, rand);
      expect(s.phase).toBe('playing');
      expect(s.order).toEqual(order); // loop never redraws the shuffle
      expect(s.pos).toBe((p0 + i + 1) % 7);
      expect(e).toEqual({ type: 'show', index: order[s.pos] ?? -1 });
    }
  });
  it('next_folder chain over folders of varying size ends cleanly on no_next_folder', () => {
    const cfg: Cfg = { on_end: 'next_folder', shuffle: true };
    const rand = seeded(9);
    let [s] = step(idle, { type: 'start', index: 0, count: 3 }, cfg, rand);
    for (const size of [2, 0, 5, 1]) {
      for (let i = 0; i < s.order.length - 1; i++) s = step(s, { type: 'tick' }, cfg, rand)[0];
      expect(step(s, { type: 'tick' }, cfg, rand)[1]).toEqual({ type: 'load_next_folder' });
      let e: SlideEffect;
      [s, e] = step(s, { type: 'folder_loaded', count: size }, cfg, rand);
      if (size === 0) {
        expect(e).toEqual({ type: 'load_next_folder' });
        [s] = step(s, { type: 'folder_loaded', count: 4 }, cfg, rand);
      }
      expect(s.phase).toBe('playing');
      expect(s.pos).toBe(0);
    }
    expect(step(s, { type: 'no_next_folder' }, cfg, rand)).toEqual([idle, { type: 'stopped', reason: 'end' }]);
  });
});
