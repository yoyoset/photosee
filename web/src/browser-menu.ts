// Right-click menu for a grid cell or list row. Deliberately short: no file
// operations exist (docs/non-goals.md). Keyboard: ↑ ↓ to move, Enter to run,
// Esc or a click elsewhere to close.

import type { BrowserItem } from './browser-items';
import { t } from './i18n';

export interface MenuEntry {
  label: string;
  run(): void;
}

let current: (() => void) | null = null;

/** Closes the menu that is showing, if any. */
export function closeMenu(): void {
  current?.();
}

/** Shows `entries` at viewport point (x, y), kept inside the window. */
export function showMenu(entries: readonly MenuEntry[], x: number, y: number, doc: Document = document): void {
  closeMenu();
  const el = doc.createElement('div');
  el.className = 'br-menu';
  el.setAttribute('role', 'menu');
  const items = entries.map((e) => item(doc, e));
  el.append(...items);
  doc.body.append(el);
  place(el, x, y, doc);
  const off = bind(el, items, doc);
  current = () => { off(); el.remove(); current = null; };
  items[0]?.focus();
}

function item(doc: Document, e: MenuEntry): HTMLButtonElement {
  const b = doc.createElement('button');
  b.type = 'button';
  b.className = 'br-menu-item';
  b.setAttribute('role', 'menuitem');
  b.textContent = e.label;
  b.addEventListener('click', () => { closeMenu(); e.run(); });
  return b;
}

function place(el: HTMLElement, x: number, y: number, doc: Document): void {
  const view = doc.documentElement;
  const r = el.getBoundingClientRect();
  // Flip toward the inside near the right and bottom edges, like native menus.
  const left = x + r.width > view.clientWidth ? Math.max(0, x - r.width) : x;
  const top = y + r.height > view.clientHeight ? Math.max(0, y - r.height) : y;
  el.style.left = `${left}px`;
  el.style.top = `${top}px`;
}

function bind(el: HTMLElement, items: HTMLButtonElement[], doc: Document): () => void {
  const onKey = (e: KeyboardEvent): void => {
    const i = items.indexOf(doc.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') closeMenu();
    else if (e.key === 'ArrowDown') items[(i + 1) % items.length]?.focus();
    else if (e.key === 'ArrowUp') items[(i - 1 + items.length) % items.length]?.focus();
    else return;
    // Ours: the browser's own keys (and Esc layers) must not see it.
    e.preventDefault();
    e.stopPropagation();
  };
  const onDown = (e: Event): void => { if (!el.contains(e.target as Node)) closeMenu(); };
  doc.addEventListener('keydown', onKey, true);
  doc.addEventListener('pointerdown', onDown, true);
  const win = doc.defaultView;
  win?.addEventListener('blur', closeMenu);
  win?.addEventListener('resize', closeMenu);
  return () => {
    doc.removeEventListener('keydown', onKey, true);
    doc.removeEventListener('pointerdown', onDown, true);
    win?.removeEventListener('blur', closeMenu);
    win?.removeEventListener('resize', closeMenu);
  };
}

/** Entries for one grid/list item: open it, and for an image, start the
 * slideshow from it. */
export function itemEntries(it: BrowserItem, open: () => void, slideshow: () => void): MenuEntry[] {
  const out: MenuEntry[] = [{ label: t('br.menu.open'), run: open }];
  if (it.kind === 'img') out.push({ label: t('br.menu.slideshow'), run: slideshow });
  return out;
}
