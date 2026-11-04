// Sort order of a folder's contents. Folders always come first (Explorer,
// XnView); the order inside each group follows the key.

import type { DirItem, ImageItem, Listing } from './api';

export type SortKey = 'name' | 'date' | 'size' | 'type';
export type SortDir = 'asc' | 'desc';
export const SORT_KEYS: readonly SortKey[] = ['name', 'date', 'size', 'type'];

// numeric: img2 < img10. 'accent' keeps a/A together but still separates é.
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'accent' });

export const compareName = (a: string, b: string): number =>
  collator.compare(a, b) || (a < b ? -1 : a > b ? 1 : 0);

/** Lower-case extension without the dot; '' for none and for ".hidden". */
export function extOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot <= 0 ? '' : name.slice(dot + 1).toLowerCase();
}

function byKey(key: SortKey): (a: ImageItem, b: ImageItem) => number {
  switch (key) {
    case 'date': return (a, b) => a.mtime_ms - b.mtime_ms;
    case 'size': return (a, b) => a.size - b.size;
    case 'type': return (a, b) => compareName(extOf(a.name), extOf(b.name));
    case 'name': return (a, b) => compareName(a.name, b.name);
  }
}

/** Ties fall back to name ascending in both directions, so the order is
 * total and re-sorting an already sorted list never reshuffles equal items. */
export function sortImages(images: readonly ImageItem[], key: SortKey, dir: SortDir): ImageItem[] {
  const cmp = byKey(key);
  const sign = dir === 'asc' ? 1 : -1;
  return [...images].sort((a, b) => sign * cmp(a, b) || compareName(a.name, b.name));
}

/** Folders carry no size, date or type: those keys list them by name ascending. */
export function sortDirs(dirs: readonly DirItem[], key: SortKey, dir: SortDir): DirItem[] {
  const sign = key === 'name' && dir === 'desc' ? -1 : 1;
  return [...dirs].sort((a, b) => sign * compareName(a.name, b.name));
}

export function sortListing(l: Listing, key: SortKey, dir: SortDir): Listing {
  return { ...l, images: sortImages(l.images, key, dir), dirs: sortDirs(l.dirs, key, dir) };
}
