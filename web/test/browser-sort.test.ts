import { describe, expect, it } from 'vitest';
import type { DirItem, ImageItem, Listing } from '../src/api';
import { compareName, extOf, sortDirs, sortImages, sortListing } from '../src/browser-sort';

const img = (name: string, size = 1, mtime_ms = 0): ImageItem => ({ name, path: `/p/${name}`, fmt: 'jpeg', size, mtime_ms });
const names = (l: { name: string }[]): string[] => l.map((x) => x.name);

describe('sort: name', () => {
  it('is natural: img2 before img10, case-insensitive', () => {
    const out = sortImages([img('img10.jpg'), img('IMG2.jpg'), img('img1.jpg')], 'name', 'asc');
    expect(names(out)).toEqual(['img1.jpg', 'IMG2.jpg', 'img10.jpg']);
  });

  it('descending reverses it', () => {
    const out = sortImages([img('a1.jpg'), img('a2.jpg'), img('a10.jpg')], 'name', 'desc');
    expect(names(out)).toEqual(['a10.jpg', 'a2.jpg', 'a1.jpg']);
  });

  it('is total: names equal ignoring case still have a fixed order', () => {
    expect(compareName('a', 'A')).not.toBe(0);
    expect(compareName('a', 'a')).toBe(0);
  });
});

describe('sort: date, size, type', () => {
  const set = [img('b.png', 30, 200), img('a.jpg', 10, 300), img('c.gif', 20, 100)];

  it('date ascending and descending', () => {
    expect(names(sortImages(set, 'date', 'asc'))).toEqual(['c.gif', 'b.png', 'a.jpg']);
    expect(names(sortImages(set, 'date', 'desc'))).toEqual(['a.jpg', 'b.png', 'c.gif']);
  });

  it('size ascending and descending', () => {
    expect(names(sortImages(set, 'size', 'asc'))).toEqual(['a.jpg', 'c.gif', 'b.png']);
    expect(names(sortImages(set, 'size', 'desc'))).toEqual(['b.png', 'c.gif', 'a.jpg']);
  });

  it('type groups by extension, then name', () => {
    const out = sortImages([img('z.png'), img('b.jpg'), img('a.png'), img('c.jpg')], 'type', 'asc');
    expect(names(out)).toEqual(['b.jpg', 'c.jpg', 'a.png', 'z.png']);
  });

  it('ties keep name ascending in both directions', () => {
    const same = [img('b.jpg', 5), img('a.jpg', 5), img('c.jpg', 5)];
    expect(names(sortImages(same, 'size', 'desc'))).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
  });

  it('does not mutate its input', () => {
    const input = [img('b.jpg'), img('a.jpg')];
    sortImages(input, 'name', 'asc');
    expect(names(input)).toEqual(['b.jpg', 'a.jpg']);
  });
});

describe('sort: folders and listings', () => {
  const dirs: DirItem[] = ['d10', 'd2', 'D1'].map((name) => ({ name, path: `/p/${name}` }));

  it('folders follow the name direction only for the name key', () => {
    expect(names(sortDirs(dirs, 'name', 'asc'))).toEqual(['D1', 'd2', 'd10']);
    expect(names(sortDirs(dirs, 'name', 'desc'))).toEqual(['d10', 'd2', 'D1']);
    expect(names(sortDirs(dirs, 'size', 'desc'))).toEqual(['D1', 'd2', 'd10']);
  });

  it('sorting a listing keeps dir/parent and is idempotent', () => {
    const l: Listing = { dir: '/p', parent: null, dirs, images: [img('b.jpg', 2), img('a.jpg', 1)] };
    const once = sortListing(l, 'size', 'desc');
    expect(once.dir).toBe('/p');
    expect(names(once.images)).toEqual(['b.jpg', 'a.jpg']);
    expect(sortListing(once, 'size', 'desc')).toEqual(once);
  });
});

describe('extOf', () => {
  it('lower-cases, and treats dotfiles and no-dot names as typeless', () => {
    expect(extOf('IMG.JPG')).toBe('jpg');
    expect(extOf('a.tar.gz')).toBe('gz');
    expect(extOf('.hidden')).toBe('');
    expect(extOf('README')).toBe('');
  });
});
