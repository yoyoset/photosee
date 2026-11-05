// A plain modal for the account pages (my folders, users): a dimmed page,
// a box with a title, a close button, Esc and a click outside to close.
// Not <dialog>.showModal(): jsdom (the tests) does not implement it.

import { ApiError } from './api';
import { errorSentence } from './errors';
import { t } from './i18n';
import './dialog.css';

export interface Dialog {
  body: HTMLElement;
  close(): void;
}

export function openDialog(doc: Document, title: string, onClose: () => void = () => {}): Dialog {
  const scrim = Object.assign(doc.createElement('div'), { className: 'dlg-scrim' });
  const box = Object.assign(doc.createElement('div'), { className: 'dlg' });
  box.setAttribute('role', 'dialog');
  box.setAttribute('aria-modal', 'true');
  box.setAttribute('aria-label', title);
  const head = Object.assign(doc.createElement('div'), { className: 'dlg-head' });
  const h = Object.assign(doc.createElement('h2'), { className: 'dlg-title', textContent: title });
  const x = Object.assign(doc.createElement('button'), { type: 'button', className: 'dlg-x', textContent: '×' });
  x.setAttribute('aria-label', t('dlg.close'));
  const body = Object.assign(doc.createElement('div'), { className: 'dlg-body' });
  head.append(h, x);
  box.append(head, body);
  scrim.append(box);
  doc.body.append(scrim);
  const key = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    e.stopPropagation(); // the viewer's Esc stack must not also act
    close();
  };
  const close = (): void => {
    doc.removeEventListener('keydown', key, true);
    scrim.remove();
    onClose();
  };
  x.addEventListener('click', close);
  scrim.addEventListener('click', (e) => { if (e.target === scrim) close(); });
  doc.addEventListener('keydown', key, true);
  x.focus();
  return { body, close };
}

/** A small button; `kind` picks the look ("primary" or "quiet"). */
export function button(doc: Document, label: string, run: () => void, kind = ''): HTMLButtonElement {
  const b = Object.assign(doc.createElement('button'), { type: 'button', className: `dlg-btn ${kind}`.trim(), textContent: label });
  b.addEventListener('click', run);
  return b;
}

/** Localized sentence plus the server's detail ("name already taken"), so a
 * refused change says why instead of failing silently. */
export function failText(e: unknown): string {
  return e instanceof ApiError ? `${errorSentence(e)} (${e.detail})` : String(e);
}

/** A line of red text for a failed call; empty text hides it. */
export function errorLine(doc: Document): HTMLElement {
  const p = Object.assign(doc.createElement('p'), { className: 'dlg-err' });
  p.setAttribute('role', 'alert');
  return p;
}
