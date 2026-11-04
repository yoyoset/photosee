import { describe, expect, it } from 'vitest';
import { ancestorChain, breadcrumbs, isUnc, isUnder, normPath, samePath } from '../src/browser-path';
import { History, upTarget } from '../src/browser-history';
import type { Listing } from '../src/api';

describe('breadcrumbs', () => {
  it('splits a Windows drive path, the root keeping its backslash', () => {
    expect(breadcrumbs('C:\\Users\\me\\Pics')).toEqual([
      { name: 'C:', path: 'C:\\' },
      { name: 'Users', path: 'C:\\Users' },
      { name: 'me', path: 'C:\\Users\\me' },
      { name: 'Pics', path: 'C:\\Users\\me\\Pics' },
    ]);
  });

  it('starts a UNC path at \\\\host\\share', () => {
    expect(breadcrumbs('\\\\nas\\photos\\2024\\may')).toEqual([
      { name: '\\\\nas\\photos', path: '\\\\nas\\photos' },
      { name: '2024', path: '\\\\nas\\photos\\2024' },
      { name: 'may', path: '\\\\nas\\photos\\2024\\may' },
    ]);
  });

  it('splits a POSIX path without offering "/"', () => {
    expect(breadcrumbs('/photos/2024')).toEqual([
      { name: 'photos', path: '/photos' },
      { name: '2024', path: '/photos/2024' },
    ]);
  });

  it('is empty for an empty path', () => {
    expect(breadcrumbs('')).toEqual([]);
  });
});

describe('path comparison', () => {
  it('ignores separators, trailing separators and (for Windows) case', () => {
    expect(samePath('C:\\Pics\\', 'c:/pics')).toBe(true);
    expect(samePath('\\\\NAS\\Share\\a', '//nas/share/a')).toBe(true);
    expect(samePath('/Photos', '/photos')).toBe(false);
  });

  it('keeps a drive root distinct from an empty path', () => {
    expect(normPath('C:\\')).toBe('c:/');
    expect(isUnder('C:\\', 'C:\\Pics')).toBe(true);
  });

  it('isUnder needs a separator boundary', () => {
    expect(isUnder('/photos', '/photos/a')).toBe(true);
    expect(isUnder('/photos', '/photos')).toBe(true);
    expect(isUnder('/photos', '/photos2')).toBe(false);
  });

  it('detects UNC', () => {
    expect(isUnc('\\\\nas\\x')).toBe(true);
    expect(isUnc('C:\\x')).toBe(false);
  });

  it('ancestorChain lists root..dir, and nothing outside the root', () => {
    expect(ancestorChain('C:\\Users', 'C:\\Users\\me\\Pics')).toEqual(['C:\\Users', 'C:\\Users\\me', 'C:\\Users\\me\\Pics']);
    expect(ancestorChain('/photos', '/photos')).toEqual(['/photos']);
    expect(ancestorChain('/photos', '/other/a')).toEqual([]);
  });
});

describe('history', () => {
  it('goes back and forward, and a new visit drops the forward branch', () => {
    const h = new History();
    ['/a', '/b', '/c'].forEach((p) => h.push(p));
    expect(h.canBack).toBe(true);
    expect(h.canForward).toBe(false);
    expect(h.peek(-1)).toBe('/b');
    h.commit(-1);
    h.commit(-1);
    expect(h.current).toBe('/a');
    expect(h.peek(1)).toBe('/b');
    h.push('/x');
    expect(h.canForward).toBe(false);
    expect(h.peek(-1)).toBe('/a');
  });

  it('ignores a revisit of the current folder and never walks off the ends', () => {
    const h = new History();
    h.push('/a');
    h.push('/A/'.toLowerCase().replace('/a/', '/a'));
    expect(h.canBack).toBe(false);
    expect(h.peek(-1)).toBeNull();
    expect(h.peek(1)).toBeNull();
  });

  it('peek does not move: a failed open keeps the entry', () => {
    const h = new History();
    h.push('/a');
    h.push('/b');
    expect(h.peek(-1)).toBe('/a');
    expect(h.current).toBe('/b');
  });

  it('up is the listing parent, null at a root', () => {
    const l = { dir: '/p/a', parent: '/p', images: [], dirs: [] } as Listing;
    expect(upTarget(l)).toBe('/p');
    expect(upTarget({ ...l, parent: null })).toBeNull();
    expect(upTarget(null)).toBeNull();
  });
});
