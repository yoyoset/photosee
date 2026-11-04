// The toolbar: tree toggle, folder picker, history buttons, address bar,
// view / sort / size controls. It holds no state of its own: `sync` redraws
// the controls from the prefs, so the persisted layout is the single truth.

import { AddressBar, type AddressHooks } from './browser-address';
import { THUMB_MAX_PX, THUMB_MIN_PX } from './browser-consts';
import { icon, type IconName } from './browser-icons';
import { SORT_KEYS, type SortKey } from './browser-sort';
import { t, type Key } from './i18n';
import type { UiState, ViewMode } from './uistate';

export interface ToolbarHooks extends AddressHooks {
  toggleTree(): void;
  /** Present only when the host has a native folder picker. */
  pick?(): void;
  back(): void;
  forward(): void;
  up(): void;
  setView(v: ViewMode): void;
  setSortKey(k: SortKey): void;
  toggleSortDir(): void;
  /** `commit` is false while the slider is dragged, true on release. */
  setThumb(px: number, commit: boolean): void;
  toggleNames(): void;
}

export interface NavState { canBack: boolean; canForward: boolean; canUp: boolean }

function button(name: IconName, label: Key, onClick: () => void, text?: string): HTMLButtonElement {
  const b = document.createElement('button');
  b.type = 'button';
  b.className = 'tb';
  b.title = t(label);
  b.setAttribute('aria-label', t(label));
  b.append(icon(name));
  if (text) {
    const s = document.createElement('span');
    s.textContent = text;
    b.append(s);
    b.classList.add('wide');
  }
  b.addEventListener('click', onClick);
  return b;
}

const sep = (): HTMLElement => Object.assign(document.createElement('span'), { className: 'tb-sep' });

export class Toolbar {
  readonly el = document.createElement('div');
  readonly address: AddressBar;
  private readonly back: HTMLButtonElement;
  private readonly forward: HTMLButtonElement;
  private readonly up: HTMLButtonElement;
  private readonly tree: HTMLButtonElement;
  private readonly views: Record<ViewMode, HTMLButtonElement>;
  private readonly sortKey = document.createElement('select');
  private readonly sortDir: HTMLButtonElement;
  private readonly thumb = document.createElement('input');
  private readonly names: HTMLButtonElement;

  constructor(h: ToolbarHooks) {
    this.el.className = 'tbar';
    this.el.setAttribute('role', 'toolbar');
    // A mouse click on a button must not steal the keyboard from the grid:
    // after "details" the user expects the arrow keys and Enter to keep
    // working on the files. Tab and Enter on a focused button still work.
    this.el.addEventListener('mousedown', (e) => {
      if ((e.target as Element).closest('button')) e.preventDefault();
    });
    this.tree = button('panel', 'br.tree', h.toggleTree);
    this.back = button('back', 'br.back', h.back);
    this.forward = button('forward', 'br.forward', h.forward);
    this.up = button('up', 'br.up', h.up);
    this.address = new AddressBar(h);
    this.views = {
      grid: button('grid', 'br.view.grid', () => h.setView('grid')),
      details: button('list', 'br.view.details', () => h.setView('details')),
    };
    this.sortDir = button('asc', 'br.sort.asc', h.toggleSortDir);
    this.names = button('names', 'br.names', h.toggleNames);
    this.buildSort(h);
    this.buildSlider(h);
    const left = [this.tree, ...(h.pick ? [button('folder', 'br.pick', h.pick, t('br.pick'))] : [])];
    this.el.append(
      ...left, sep(), this.back, this.forward, this.up, this.address.el, sep(),
      this.views.grid, this.views.details, sep(), this.sortKey, this.sortDir, sep(), this.thumb, this.names,
    );
  }

  private buildSort(h: ToolbarHooks): void {
    this.sortKey.className = 'tb-sel';
    this.sortKey.title = t('br.sort');
    this.sortKey.setAttribute('aria-label', t('br.sort'));
    for (const k of SORT_KEYS) this.sortKey.append(new Option(t(`br.sort.${k}`), k));
    this.sortKey.addEventListener('change', () => h.setSortKey(this.sortKey.value as SortKey));
  }

  private buildSlider(h: ToolbarHooks): void {
    const s = this.thumb;
    s.type = 'range';
    s.className = 'tb-range';
    s.min = String(THUMB_MIN_PX);
    s.max = String(THUMB_MAX_PX);
    s.step = '8';
    s.title = t('br.thumb_size');
    s.setAttribute('aria-label', t('br.thumb_size'));
    s.addEventListener('input', () => h.setThumb(Number(s.value), false));
    s.addEventListener('change', () => h.setThumb(Number(s.value), true));
  }

  sync(p: UiState, nav: NavState): void {
    this.tree.setAttribute('aria-pressed', String(p.tree.visible));
    this.back.disabled = !nav.canBack;
    this.forward.disabled = !nav.canForward;
    this.up.disabled = !nav.canUp;
    for (const v of ['grid', 'details'] as const) this.views[v].setAttribute('aria-pressed', String(p.view === v));
    this.sortKey.value = p.sort.key;
    this.sortDir.replaceChildren(icon(p.sort.dir === 'asc' ? 'asc' : 'desc'));
    const dirLabel = t(p.sort.dir === 'asc' ? 'br.sort.asc' : 'br.sort.desc');
    this.sortDir.title = dirLabel;
    this.sortDir.setAttribute('aria-label', dirLabel);
    this.thumb.value = String(p.thumb);
    this.names.setAttribute('aria-pressed', String(p.names));
    // Sizes and names only mean something for thumbnails.
    this.thumb.disabled = this.names.disabled = p.view !== 'grid';
  }
}
