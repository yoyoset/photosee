import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Api } from '../src/api';
import { MetaCache } from '../src/browser-meta';

describe('MetaCache', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  const make = () => {
    const meta = vi.fn(async (p: string) => ({ width: p.length, height: 2 }));
    const updates = vi.fn();
    const cache = new MetaCache({ meta } as unknown as Pick<Api, 'meta'>, updates);
    return { meta, updates, cache };
  };

  it('waits for 150 ms of quiet, so scrolling does not start a request storm', async () => {
    const { meta, cache } = make();
    cache.want(['/a']);
    vi.advanceTimersByTime(100);
    cache.want(['/b', '/c']); // scrolled on: the first wish is replaced
    vi.advanceTimersByTime(100);
    expect(meta).not.toHaveBeenCalled();
    vi.advanceTimersByTime(60);
    await vi.runAllTimersAsync();
    expect(meta.mock.calls.map((c) => c[0])).toEqual(['/b', '/c']);
  });

  it('asks for each path once and keeps the answer', async () => {
    const { meta, updates, cache } = make();
    cache.want(['/a']);
    await vi.advanceTimersByTimeAsync(200);
    expect(cache.get('/a')).toEqual({ w: 2, h: 2 });
    expect(updates).toHaveBeenCalled();
    cache.want(['/a']);
    await vi.advanceTimersByTimeAsync(200);
    expect(meta).toHaveBeenCalledTimes(1);
  });

});

describe('MetaCache: limits', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('runs at most four requests at a time', async () => {
    let running = 0;
    let peak = 0;
    const meta = vi.fn(async () => {
      running++;
      peak = Math.max(peak, running);
      await new Promise((r) => setTimeout(r, 10));
      running--;
      return { width: 1, height: 1 };
    });
    const cache = new MetaCache({ meta } as unknown as Pick<Api, 'meta'>, () => {});
    cache.want(Array.from({ length: 12 }, (_, i) => `/f${i}`));
    await vi.advanceTimersByTimeAsync(500);
    expect(meta).toHaveBeenCalledTimes(12);
    expect(peak).toBe(4);
  });

  it('a failing file is not retried and leaves the cell empty', async () => {
    const meta = vi.fn(async () => { throw new Error('corrupt'); });
    const cache = new MetaCache({ meta } as unknown as Pick<Api, 'meta'>, () => {});
    cache.want(['/bad']);
    await vi.advanceTimersByTimeAsync(200);
    cache.want(['/bad']);
    await vi.advanceTimersByTimeAsync(200);
    expect(meta).toHaveBeenCalledTimes(1);
    expect(cache.get('/bad')).toBeUndefined();
  });
});
