// The browser shows folders and images in one sequence (folders first); the
// store only knows images. This is the mapping between the two.

import type { ImageItem, Listing } from './api';

export type BrowserItem =
  | { kind: 'dir'; name: string; path: string }
  | { kind: 'img'; name: string; path: string; item: ImageItem; index: number };

export function itemsOf(l: Listing | null): BrowserItem[] {
  if (!l) return [];
  const dirs = l.dirs.map((d): BrowserItem => ({ kind: 'dir', name: d.name, path: d.path }));
  const imgs = l.images.map((item, index): BrowserItem => ({ kind: 'img', name: item.name, path: item.path, item, index }));
  return [...dirs, ...imgs];
}

/** Last chars kept visible when a name is cut in the middle: the extension
 * plus a couple of characters before it, so IMG_0012.jpg stays recognisable. */
export function splitMiddle(name: string): { head: string; tail: string } {
  const dot = name.lastIndexOf('.');
  const ext = dot > 0 ? name.length - dot : 0;
  const keep = Math.max(ext + 2, 6);
  if (name.length <= keep + 4) return { head: name, tail: '' };
  return { head: name.slice(0, name.length - keep), tail: name.slice(name.length - keep) };
}
