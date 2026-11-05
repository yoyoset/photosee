// "Users": the admin creates readers (name + PIN), resets a PIN, deletes an
// account. Folders are not set here: each reader picks their own under
// "My folders". The list is the server's users.json; this page only calls
// the admin API and redraws from its answer.

import type { AccountRow, Api } from './api';
import { button, errorLine, failText, openDialog, type Dialog } from './dialog';
import { t } from './i18n';

export interface AdminDeps {
  doc: Document;
  api: Api;
  /** The admin's own name: no delete button on their own row. */
  me: string;
}

function input(doc: Document, label: string, pin = false): HTMLInputElement {
  const i = Object.assign(doc.createElement('input'), { className: 'dlg-in', placeholder: label, autocomplete: 'off' });
  i.setAttribute('aria-label', label);
  if (pin) Object.assign(i, { type: 'password', inputMode: 'numeric', maxLength: 12 });
  return i;
}

export function openAdmin(d: AdminDeps): Dialog {
  const dlg = openDialog(d.doc, t('acct.admin'));
  void redraw(d, dlg);
  return dlg;
}

async function redraw(d: AdminDeps, dlg: Dialog): Promise<void> {
  const err = errorLine(d.doc);
  let rows: AccountRow[] = [];
  try {
    rows = await d.api.accounts();
  } catch (e) {
    err.textContent = failText(e);
  }
  const act = (p: Promise<void>, el: HTMLButtonElement): void => {
    el.disabled = true;
    p.then(() => void redraw(d, dlg), (e: unknown) => { el.disabled = false; err.textContent = failText(e); });
  };
  dlg.body.replaceChildren(...rows.map((r) => accountRow(d, r, act)), newUser(d, act), err);
}

type Act = (p: Promise<void>, el: HTMLButtonElement) => void;

function accountRow(d: AdminDeps, r: AccountRow, act: Act): HTMLElement {
  const { doc } = d;
  const el = Object.assign(doc.createElement('div'), { className: 'dlg-row' });
  const name = Object.assign(doc.createElement('span'), { className: 'dlg-name', textContent: r.name });
  if (r.admin) name.append(Object.assign(doc.createElement('span'), { className: 'dlg-tag', textContent: t('adm.is_admin') }));
  const count = Object.assign(doc.createElement('span'), { className: 'dlg-dim', textContent: t('adm.folders', { n: r.folders }) });
  const pin = button(doc, t('adm.pin'), () => {
    const box = input(doc, t('login.pin'), true);
    const ok = button(doc, '✓', () => act(d.api.resetPin(r.name, box.value), ok), 'primary');
    ok.setAttribute('aria-label', t('adm.pin'));
    el.replaceChildren(name, box, ok);
    box.focus();
  }, 'quiet');
  el.append(name, count, pin);
  if (r.name !== d.me) {
    const del = button(doc, t('adm.delete'), () => {
      // Second tap confirms: deleting also logs that person out everywhere.
      if (del.dataset['armed']) return act(d.api.deleteAccount(r.name), del);
      del.dataset['armed'] = '1';
      del.textContent = t('adm.confirm', { name: r.name });
    }, 'quiet danger');
    el.append(del);
  }
  return el;
}

function newUser(d: AdminDeps, act: Act): HTMLElement {
  const { doc } = d;
  const form = Object.assign(doc.createElement('form'), { className: 'dlg-row' });
  const name = input(doc, t('login.name'));
  const pin = input(doc, t('login.pin'), true);
  const go = Object.assign(doc.createElement('button'), { type: 'submit', className: 'dlg-btn primary', textContent: t('adm.create') });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    act(d.api.createAccount(name.value.trim(), pin.value), go);
  });
  form.append(name, pin, go);
  const wrap = Object.assign(doc.createElement('div'), { className: 'dlg-body' });
  wrap.style.padding = '8px 0 0';
  wrap.append(Object.assign(doc.createElement('div'), { className: 'dlg-dim', textContent: t('adm.new') }), form);
  return wrap;
}
