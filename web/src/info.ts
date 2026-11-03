// Info panel (key I): file name, dimensions, size and the EXIF summary from
// /meta. Package J.
//
// Visibility contract (state.ts has no `infoVisible`, see QUESTIONS.md): the
// controller shows and hides the panel by toggling `root.hidden`; the panel
// watches that attribute. It only talks to /meta while it is shown.

import type { ImageItem, Meta } from './api';
import type { Ctx } from './ctx';
import { t, type Key } from './i18n';

export interface Row { label: Key; value: string }

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n;
  let u = -1;
  do { v /= 1024; u++; } while (v >= 1024 && u < units.length - 1);
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[u]}`;
}

/** EXIF writes dates as "2024:05:01 13:22:10"; show them the way people read dates. */
export function formatTaken(s: string): string {
  return s.replace(/^(\d{4}):(\d{2}):(\d{2})/, '$1-$2-$3');
}

const trim = (n: number): string => String(Number(n.toFixed(1)));

/** Rows to show, in order. Anything unknown is left out, never shown empty. */
export function infoRows(item: ImageItem | null, meta: Meta | null): Row[] {
  const e = meta?.exif;
  const size = meta?.size ?? item?.size;
  const candidates: Array<[Key, string | null | undefined]> = [
    ['info.dimensions', meta && meta.width > 0 && meta.height > 0 ? `${meta.width} × ${meta.height}` : null],
    ['info.file_size', size === undefined ? null : formatBytes(size)],
    ['info.taken', e?.taken ? formatTaken(e.taken) : null],
    ['info.camera', e?.camera],
    ['info.lens', e?.lens],
    ['info.exposure', e?.exposure ? `${e.exposure} s` : null],
    ['info.f_number', e?.f_number != null ? `f/${trim(e.f_number)}` : null],
    ['info.iso', e?.iso != null ? String(e.iso) : null],
    ['info.focal', e?.focal_mm != null ? `${trim(e.focal_mm)} mm` : null],
  ];
  return candidates.flatMap(([label, value]) => (value ? [{ label, value }] : []));
}

function render(root: HTMLElement, item: ImageItem | null, meta: Meta | null): void {
  const name = document.createElement('div');
  name.className = 'info-name';
  name.textContent = item?.name ?? '';
  const dl = document.createElement('dl');
  for (const r of infoRows(item, meta)) {
    const dt = document.createElement('dt');
    dt.textContent = t(r.label);
    const dd = document.createElement('dd');
    dd.textContent = r.value; // textContent: file names and EXIF strings are hostile input
    dl.append(dt, dd);
  }
  root.replaceChildren(name, dl);
}

export function mountInfo(root: HTMLElement, ctx: Ctx): () => void {
  root.classList.add('info');
  let unEsc: (() => void) | null = null;
  let loaded: { path: string; meta: Meta } | null = null;
  // Bumped whenever the answer to an in-flight request stops mattering.
  let seq = 0;

  const close = (): boolean => {
    if (root.hidden) return false;
    root.hidden = true;
    sync();
    return true;
  };

  function sync(): void {
    if (root.hidden) {
      unEsc?.();
      unEsc = null;
      seq++;
      return;
    }
    unEsc ??= ctx.esc.register('panel', close);
    const mine = ++seq;
    const item = ctx.store.current();
    const known = loaded && loaded.path === item?.path ? loaded.meta : null;
    render(root, item, known);
    if (!item || known) return;
    ctx.api.meta(item.path).then((meta) => {
      // The user may have moved on or closed the panel while this was in flight.
      if (mine !== seq) return;
      loaded = { path: item.path, meta };
      render(root, item, meta);
    }, () => { /* no EXIF is not an error worth a card: the name and size stay */ });
  }

  const unsub = ctx.store.subscribe((s, prev) => {
    if (s.index !== prev.index || s.listing !== prev.listing) sync();
  });
  const obs = new MutationObserver(sync);
  obs.observe(root, { attributes: true, attributeFilter: ['hidden'] });
  sync();

  return () => {
    obs.disconnect();
    unsub();
    unEsc?.();
    seq++;
    root.classList.remove('info');
    root.replaceChildren();
  };
}
