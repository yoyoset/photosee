// The account button at the end of the browser toolbar (multi-user server):
// shows who is logged in and opens a short menu: My folders, Users (admins
// only), Switch user. One button instead of three keeps the phone toolbar
// at two rows.

import { openAdmin } from './admin';
import type { Api, Session } from './api';
import { icon } from './browser-icons';
import { showMenu, type MenuEntry } from './browser-menu';
import { openFolders } from './folders';
import { t } from './i18n';

export interface AccountDeps {
  doc: Document;
  api: Api;
  session: Session;
  reload: () => void;
}

export function mountAccount(toolbar: HTMLElement, d: AccountDeps): void {
  const name = d.session.user ?? '';
  const b = d.doc.createElement('button');
  b.type = 'button';
  b.className = 'tb wide tb-user';
  b.title = t('acct.menu', { name });
  b.setAttribute('aria-label', b.title);
  b.setAttribute('aria-haspopup', 'menu');
  b.append(icon('user'), Object.assign(d.doc.createElement('span'), { textContent: name }));
  b.addEventListener('click', () => {
    const r = b.getBoundingClientRect();
    showMenu(entries(d, name), r.left, r.bottom + 2, d.doc);
  });
  toolbar.append(b);
  // Nothing to browse yet: a new reader lands straight on "add folder".
  if (d.session.roots.length === 0) folders(d);
}

const folders = (d: AccountDeps): void => void openFolders({ doc: d.doc, api: d.api, roots: d.session.roots, reload: d.reload });

function entries(d: AccountDeps, me: string): MenuEntry[] {
  const out: MenuEntry[] = [{ label: t('acct.folders'), run: () => folders(d) }];
  if (d.session.admin) out.push({ label: t('acct.admin'), run: () => void openAdmin({ doc: d.doc, api: d.api, me }) });
  // A failed logout (server gone) still reloads: the login screen or the
  // connection error then says what is wrong, instead of a dead button.
  out.push({ label: t('acct.switch'), run: () => void d.api.logout().catch(() => {}).finally(d.reload) });
  return out;
}
