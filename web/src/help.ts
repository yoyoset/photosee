// Help panel (key ?): the shortcut table for this platform, plus the Modal
// shell that settings.ts reuses. Package L.
//
// Why Modal lives here: package L owns a fixed list of files
// (docs/ownership.toml), and EscStack keeps ONE handler per layer, so two
// panels each registering 'panel' would overwrite each other. Funnelling both
// through one class lets "open" close the other panel first.

import type { Session } from './api';
import type { Ctx } from './ctx';
import { el } from './errors';
import { t, type Key } from './i18n';
import './panels.css';

export interface ModalHooks {
  onOpen?: () => void;
  /** Runs before the panel hides (settings flushes its pending save here). */
  onClose?: () => void;
}

const FOCUSABLE =
  'input:not([disabled]),button:not([disabled]),a[href],summary,select,textarea,[tabindex]:not([tabindex="-1"])';

let active: Modal | null = null;
let nextId = 0;

function claim(m: Modal): void {
  active = m;
}

export class Modal {
  readonly el = el('div', 'ps-backdrop');
  readonly body = el('div', 'ps-panel-body');
  private readonly panel = el('section', 'ps-panel');
  private readonly closeBtn = el('button', 'ps-close', '×');
  private opener: HTMLElement | null = null;
  private unregister: (() => void) | null = null;

  constructor(
    root: HTMLElement, private readonly ctx: Ctx, titleKey: Key, cls: string,
    private readonly hooks: ModalHooks = {},
  ) {
    const id = `ps-panel-title-${nextId++}`;
    const head = el('header', 'ps-panel-head');
    const title = el('h2', '', t(titleKey));
    title.id = id;
    head.append(title, this.closeBtn);
    this.closeBtn.type = 'button';
    this.closeBtn.setAttribute('aria-label', t('action.close'));
    this.closeBtn.addEventListener('click', () => this.close());
    this.panel.classList.add(cls);
    this.panel.setAttribute('role', 'dialog');
    this.panel.setAttribute('aria-modal', 'true');
    this.panel.setAttribute('aria-labelledby', id);
    this.panel.append(head, this.body);
    this.el.append(this.panel);
    this.el.hidden = true;
    this.el.addEventListener('keydown', (e) => this.trapTab(e));
    // A click on the empty area around the panel dismisses it.
    this.el.addEventListener('mousedown', (e) => {
      if (e.target === this.el) this.close();
    });
    root.append(this.el);
  }

  isOpen(): boolean {
    return !this.el.hidden;
  }

  /** `opener` gets focus back on close; defaults to whatever has focus now. */
  open(opener?: HTMLElement | null): void {
    if (this.isOpen()) return;
    if (active && active !== this) active.close();
    claim(this);
    const cur = document.activeElement;
    this.opener = opener ?? (cur instanceof HTMLElement ? cur : null);
    this.el.hidden = false;
    this.unregister = this.ctx.esc.register('panel', () => {
      if (!this.isOpen()) return false;
      this.close();
      return true;
    });
    this.hooks.onOpen?.();
    (this.focusables().find((e) => e !== this.closeBtn) ?? this.closeBtn).focus();
  }

  close(): void {
    if (!this.isOpen()) return;
    this.hooks.onClose?.();
    this.el.hidden = true;
    this.unregister?.();
    this.unregister = null;
    if (active === this) active = null;
    if (this.opener?.isConnected) this.opener.focus();
    this.opener = null;
  }

  toggle(opener?: HTMLElement | null): void {
    if (this.isOpen()) this.close();
    else this.open(opener);
  }

  dispose(): void {
    this.close();
    this.el.remove();
  }

  private focusables(): HTMLElement[] {
    return Array.from(this.panel.querySelectorAll<HTMLElement>(FOCUSABLE));
  }

  /** Focus trap: Tab and Shift+Tab wrap inside the panel. */
  private trapTab(e: KeyboardEvent): void {
    if (e.key !== 'Tab') return;
    const items = this.focusables();
    const first = items[0];
    const last = items[items.length - 1];
    if (!first || !last) return e.preventDefault();
    const at = document.activeElement;
    if (e.shiftKey && (at === first || !this.panel.contains(at))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (at === last || !this.panel.contains(at))) {
      e.preventDefault();
      first.focus();
    }
  }
}

export type PanelHandle = Pick<Modal, 'open' | 'close' | 'toggle' | 'isOpen' | 'dispose'>;

interface Row {
  label: Key;
  keys: string[];
  /** Replaces `keys` on macOS. */
  mac?: string[];
}

// Mirrors docs/ux-spec.md §Keys. Existing button labels are reused where one fits.
const ROWS: readonly Row[] = [
  { label: 'btn.prev', keys: ['←', 'PgUp', 'Backspace'] },
  { label: 'btn.next', keys: ['→', 'Space', 'PgDn'] },
  { label: 'help.first_last', keys: ['Home', 'End'] },
  { label: 'help.folder', keys: ['Ctrl+→', 'Ctrl+←', ']', '['], mac: ['⌘→', '⌘←', ']', '['] },
  { label: 'btn.fullscreen', keys: ['F', 'F11', 'Enter'], mac: ['F', '⌃⌘F', 'Enter'] },
  { label: 'btn.play', keys: ['S'] },
  { label: 'btn.rotate_cw', keys: ['R'] },
  { label: 'btn.rotate_ccw', keys: ['Shift+R'] },
  { label: 'btn.fit', keys: ['0'] },
  { label: 'btn.actual', keys: ['1'] },
  { label: 'help.zoom', keys: ['+', '−'] },
  { label: 'help.info', keys: ['I'] },
  { label: 'help.title', keys: ['?'] },
  { label: 'action.close', keys: ['Esc'] },
];

/** Pure, for tests: what the table shows on this platform. */
export function helpRows(platform: Session['platform']): { label: string; keys: string[] }[] {
  const mac = platform === 'macos';
  return ROWS.map((r) => ({
    label: t(r.label),
    keys: mac && r.mac ? r.mac : r.keys,
  }));
}

function buildTable(platform: Session['platform']): HTMLTableElement {
  const table = el('table', 'ps-help-table');
  const tbody = el('tbody');
  for (const row of helpRows(platform)) {
    const tr = el('tr');
    const th = el('th', '', row.label);
    th.scope = 'row';
    const td = el('td');
    td.append(...row.keys.map((k) => el('kbd', '', k)));
    tr.append(th, td);
    tbody.append(tr);
  }
  table.append(tbody);
  return table;
}

export function mountHelp(root: HTMLElement, ctx: Ctx): PanelHandle {
  const modal = new Modal(root, ctx, 'help.title', 'ps-help');
  modal.body.append(buildTable(ctx.session.platform));
  return modal;
}
