import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  UiPrefs, defaultUiState, loadUiState, parseUiState, pushRecent, removePath,
} from '../src/uistate';

describe('parseUiState', () => {
  it('falls back to the documented defaults for {} and for junk', () => {
    const want = { visible: true, width: 220 };
    for (const raw of [{}, null, 'x', [], 7, { version: 2 }]) {
      const s = parseUiState(raw);
      expect(s.tree).toEqual(want);
      expect(s.view).toBe('grid');
      expect(s.sort).toEqual({ key: 'name', dir: 'asc' });
      expect(s.thumb).toBe(160);
      expect(s.names).toBe(true);
      expect(s.recent).toEqual([]);
      expect(s.last).toBeNull();
    }
  });

  it('reads valid values', () => {
    const s = parseUiState({
      version: 1, tree: { visible: false, width: 300 }, view: 'details',
      sort: { key: 'date', dir: 'desc' }, thumb: 96, names: false,
      recent: ['/a', '/b'], network: ['\\\\nas\\x'], last: '/a',
    });
    expect(s).toMatchObject({
      tree: { visible: false, width: 300 }, view: 'details', sort: { key: 'date', dir: 'desc' },
      thumb: 96, names: false, recent: ['/a', '/b'], network: ['\\\\nas\\x'], last: '/a',
    });
  });

  it('clamps numbers and rejects wrong types field by field', () => {
    const s = parseUiState({
      version: 1, tree: { width: 99999 }, view: 'cube', sort: { key: 'colour', dir: 'up' },
      thumb: 'big', names: 'yes', recent: ['/ok', 5, '', null], last: 4,
    });
    expect(s.tree.width).toBe(4096); // only a sanity cap; the window limits it at runtime
    expect(s.view).toBe('grid');
    expect(s.sort).toEqual({ key: 'name', dir: 'asc' });
    expect(s.thumb).toBe(160);
    expect(s.names).toBe(true);
    expect(s.recent).toEqual(['/ok']);
    expect(s.last).toBeNull();
    expect(parseUiState({ version: 1, tree: { width: 5 }, thumb: 5000 })).toMatchObject({ tree: { width: 160 }, thumb: 640 });
  });

});

describe('parseUiState: fit', () => {
  it('keeps a known fit mode and falls back to the whole image', () => {
    expect(parseUiState({ version: 1, fit: 'width' }).fit).toBe('width');
    expect(parseUiState({ version: 1, fit: 'height' }).fit).toBe('height');
    expect(parseUiState({ version: 1, fit: 'zoom' }).fit).toBe('contain');
    expect(parseUiState({ version: 1 }).fit).toBe('contain');
  });
});

describe('parseUiState: lists and unknown fields', () => {
  it('caps recents at 10', () => {
    const recent = Array.from({ length: 30 }, (_, i) => `/r${i}`);
    expect(parseUiState({ version: 1, recent }).recent).toHaveLength(10);
  });

  it('keeps fields it does not know, at the top level and inside known objects', () => {
    const s = parseUiState({ version: 1, future: { a: 1 }, tree: { width: 200, pinned: true }, sort: { key: 'size', extra: 1 } });
    expect(s['future']).toEqual({ a: 1 });
    expect(s.tree).toEqual({ visible: true, width: 200, pinned: true });
    expect(s.sort).toMatchObject({ key: 'size', extra: 1 });
    expect(JSON.parse(JSON.stringify(s))['future']).toEqual({ a: 1 });
  });
});

describe('recent list', () => {
  it('puts the newest first without duplicates, up to 10', () => {
    let list: string[] = [];
    for (let i = 0; i < 12; i++) list = pushRecent(list, `/f${i}`);
    expect(list).toHaveLength(10);
    expect(list[0]).toBe('/f11');
    list = pushRecent(list, '/f5');
    expect(list[0]).toBe('/f5');
    expect(list.filter((p) => p === '/f5')).toHaveLength(1);
  });

  it('treats spellings of one Windows path as the same entry', () => {
    expect(pushRecent(['C:\\Pics'], 'c:/pics/')).toEqual(['c:/pics/']);
    expect(removePath(['C:\\Pics', 'D:\\x'], 'c:\\pics')).toEqual(['D:\\x']);
  });
});

describe('loadUiState', () => {
  it('uses defaults when the endpoint fails or is missing', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(await loadUiState({ uiState: async () => { throw new Error('down'); } })).toEqual(defaultUiState());
    expect((await loadUiState({} as never)).view).toBe('grid');
    warn.mockRestore();
  });
});

describe('UiPrefs saving', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('debounces: many changes, one PUT 500 ms after the last, with version 1', async () => {
    const save = vi.fn<(s: Record<string, unknown>) => Promise<void>>(async () => {});
    const p = new UiPrefs(defaultUiState(), { saveUiState: save });
    p.set({ thumb: 100 });
    vi.advanceTimersByTime(400);
    p.set({ thumb: 120 });
    vi.advanceTimersByTime(499);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(save).toHaveBeenCalledOnce();
    expect(save.mock.calls[0]?.[0]).toMatchObject({ version: 1, thumb: 120 });
  });

  it('writes nothing when the state ended up unchanged', async () => {
    const save = vi.fn<(s: Record<string, unknown>) => Promise<void>>(async () => {});
    const p = new UiPrefs(defaultUiState(), { saveUiState: save });
    p.set({ thumb: 200 });
    p.set({ thumb: 160 }); // back to where it was
    await vi.advanceTimersByTimeAsync(600);
    expect(save).not.toHaveBeenCalled();
  });

  it('a failed save is logged, not thrown', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const p = new UiPrefs(defaultUiState(), { saveUiState: async () => { throw new Error('read-only'); } });
    p.set({ names: false });
    await vi.advanceTimersByTimeAsync(600);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it('notifies subscribers synchronously', () => {
    const p = new UiPrefs(defaultUiState(), null);
    const seen: number[] = [];
    p.subscribe((s) => seen.push(s.thumb));
    p.set({ thumb: 77 });
    expect(seen).toEqual([77]);
  });
});
