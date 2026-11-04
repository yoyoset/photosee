// Text for the status bar and the details columns: pure formatting.

import type { Listing } from './api';
import type { BrowserItem } from './browser-items';
import { extOf } from './browser-sort';
import { t } from './i18n';

export interface Dims { w: number; h: number }

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
  return `${v.toFixed(u >= 2 ? 2 : 1)} ${units[u]}`;
}

const pad = (n: number): string => String(n).padStart(2, '0');

/** Local time, fixed layout (not locale dependent, so columns line up).
 * mtime 0 means "unknown" in the contract, shown as empty. */
export function formatDate(ms: number): string {
  if (!ms) return '';
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export const formatDims = (d: Dims | undefined): string => (d ? `${d.w}×${d.h}` : '');

export const typeLabel = (name: string): string => extOf(name).toUpperCase();

export function countsText(l: Listing | null): string {
  if (!l) return '';
  const imgs = l.images.length;
  const dirs = l.dirs.length;
  const parts = [t(imgs === 1 ? 'status.images_one' : 'status.images', { n: imgs })];
  if (dirs > 0) parts.push(t(dirs === 1 ? 'status.folders_one' : 'status.folders', { n: dirs }));
  return parts.join(' · ');
}

export function selectionText(item: BrowserItem | null, dims: Dims | undefined): string {
  if (!item) return '';
  const parts = [t('status.selected', { name: item.name })];
  if (item.kind === 'dir') return parts[0] as string;
  if (dims) parts.push(formatDims(dims));
  parts.push(formatBytes(item.item.size));
  const date = formatDate(item.item.mtime_ms);
  if (date) parts.push(date);
  return parts.join(' · ');
}
