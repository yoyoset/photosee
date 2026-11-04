import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { ImageItem } from '../src/api';
import { itemsOf, splitMiddle } from '../src/browser-items';
import { browserKey } from '../src/browser-keys';
import { countsText, formatBytes, formatDate, selectionText } from '../src/browser-status';
import { TypeAhead, findPrefix } from '../src/browser-typeahead';
import { setLang } from '../src/i18n';

const ev = (key: string, mods: Partial<Record<'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey', boolean>> = {}) =>
  ({ key, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...mods });

describe('type-ahead', () => {
  const names = ['alpha', 'Beta', 'bravo', 'charlie', 'apple'];

  it('a single letter moves to the next match and wraps', () => {
    expect(findPrefix(names, 'b', 0)).toBe(1);
    expect(findPrefix(names, 'b', 1)).toBe(2);
    expect(findPrefix(names, 'b', 2)).toBe(1);
    expect(findPrefix(names, 'a', 4)).toBe(0);
  });

  it('a longer prefix narrows from the current item, including it', () => {
    expect(findPrefix(names, 'br', 1)).toBe(2);
    expect(findPrefix(names, 'bra', 2)).toBe(2);
    expect(findPrefix(names, 'zz', 0)).toBe(-1);
  });

  it('the typed word resets after a pause of more than one second', () => {
    const ta = new TypeAhead();
    expect(ta.push('b', 1000)).toBe('b');
    expect(ta.push('r', 1900)).toBe('br');
    expect(ta.push('x', 3000)).toBe('x');
    ta.reset();
    expect(ta.push('Q', 3100)).toBe('q');
  });
});

describe('items and labels', () => {
  it('lists folders first, images carry their store index', () => {
    const image = (n: string): ImageItem => ({ name: n, path: `/p/${n}`, fmt: 'jpeg', size: 1, mtime_ms: 0 });
    const out = itemsOf({ dir: '/p', parent: null, dirs: [{ name: 'd', path: '/p/d' }], images: [image('a.jpg'), image('b.jpg')] });
    expect(out.map((i) => `${i.kind}:${i.name}`)).toEqual(['dir:d', 'img:a.jpg', 'img:b.jpg']);
    expect(out[2]).toMatchObject({ index: 1 });
    expect(itemsOf(null)).toEqual([]);
  });

  it('cuts names in the middle so the extension stays', () => {
    expect(splitMiddle('a.jpg')).toEqual({ head: 'a.jpg', tail: '' });
    const { head, tail } = splitMiddle('IMG_20240501_132200_HDR.jpeg');
    expect(tail.endsWith('.jpeg')).toBe(true);
    expect(head + tail).toBe('IMG_20240501_132200_HDR.jpeg');
    expect(splitMiddle('a-very-long-folder-name-without-extension').tail).toHaveLength(6);
  });
});

describe('status text', () => {
  beforeEach(() => setLang('en'));
  afterEach(() => setLang('en'));

  it('formats sizes', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(8.2 * 1024 * 1024)).toBe('8.2 MB');
    expect(formatBytes(3 * 1024 ** 3)).toBe('3.00 GB');
  });

  it('shows no date for unknown mtime and a fixed layout otherwise', () => {
    expect(formatDate(0)).toBe('');
    expect(formatDate(new Date(2024, 4, 1, 13, 22).getTime())).toBe('2024-05-01 13:22');
  });

  it('counts images and folders, with singular forms', () => {
    const l = (i: number, d: number) => ({
      dir: '/p', parent: null,
      images: Array.from({ length: i }, (_, k) => ({ name: `${k}.jpg`, path: `${k}`, fmt: 'jpeg' as const, size: 1, mtime_ms: 0 })),
      dirs: Array.from({ length: d }, (_, k) => ({ name: `d${k}`, path: `d${k}` })),
    });
    expect(countsText(l(412, 3))).toBe('412 images · 3 folders');
    expect(countsText(l(1, 1))).toBe('1 image · 1 folder');
    expect(countsText(l(2, 0))).toBe('2 images');
    expect(countsText(null)).toBe('');
  });

  it('describes the selection, adding dimensions only when known', () => {
    const it = itemsOf({
      dir: '/p', parent: null, dirs: [],
      images: [{ name: 'IMG_0012.jpg', path: '/p/IMG_0012.jpg', fmt: 'jpeg', size: 2048, mtime_ms: new Date(2024, 4, 1, 13, 22).getTime() }],
    })[0] ?? null;
    expect(selectionText(it, undefined)).toBe('Selected: IMG_0012.jpg · 2.0 KB · 2024-05-01 13:22');
    expect(selectionText(it, { w: 6000, h: 4000 })).toBe('Selected: IMG_0012.jpg · 6000×4000 · 2.0 KB · 2024-05-01 13:22');
    expect(selectionText(null, undefined)).toBe('');
  });
});

describe('browser keys', () => {
  it('maps navigation, open and up', () => {
    expect(browserKey(ev('ArrowDown'))).toEqual({ t: 'move', key: 'down' });
    expect(browserKey(ev('PageUp'))).toEqual({ t: 'move', key: 'pageup' });
    expect(browserKey(ev('Enter'))).toEqual({ t: 'open' });
    expect(browserKey(ev('Backspace'))).toEqual({ t: 'up' });
    expect(browserKey(ev('ArrowUp', { altKey: true }))).toEqual({ t: 'up' });
    expect(browserKey(ev('ArrowLeft', { altKey: true }))).toEqual({ t: 'back' });
    expect(browserKey(ev('ArrowRight', { altKey: true }))).toEqual({ t: 'forward' });
  });

  it('Ctrl+L (and Cmd+L, Alt+D) focus the address bar; other chords are left alone', () => {
    expect(browserKey(ev('l', { ctrlKey: true }))).toEqual({ t: 'address' });
    expect(browserKey(ev('L', { metaKey: true }))).toEqual({ t: 'address' });
    expect(browserKey(ev('d', { altKey: true }))).toEqual({ t: 'address' });
    expect(browserKey(ev('r', { ctrlKey: true }))).toBeNull();
    expect(browserKey(ev('F5'))).toBeNull();
  });

  it('plain characters are type-ahead, shifted ones too', () => {
    expect(browserKey(ev('a'))).toEqual({ t: 'type', ch: 'a' });
    expect(browserKey(ev('A', { shiftKey: true }))).toEqual({ t: 'type', ch: 'A' });
  });
});
