// "My folders": each reader mounts the folders they want from the disks,
// instead of everyone getting whole disks (the user's rule, 2026-11-06).
// Two views in one dialog: the list of mounted folders (remove), and a
// picker that walks the volumes, folders only (add).
//
// After a change the page reloads: the tree, places and listing caches all
// start from /session, and a reload rebuilds them from the new roots with
// no second code path to keep in sync.

import type { Api, Place } from './api';
import { icon } from './browser-icons';
import { button, errorLine, failText, openDialog, type Dialog } from './dialog';
import { t } from './i18n';

export interface FolderDeps {
  doc: Document;
  api: Api;
  roots: readonly Place[];
  reload: () => void;
}

const row = (doc: Document, cls: string): HTMLElement => Object.assign(doc.createElement('div'), { className: cls });

export function openFolders(d: FolderDeps): Dialog {
  const dlg = openDialog(d.doc, t('acct.folders'));
  showMounted(d, dlg);
  return dlg;
}

function showMounted(d: FolderDeps, dlg: Dialog): void {
  const { doc } = d;
  const err = errorLine(doc);
  const rows = d.roots.map((r) => {
    const el = row(doc, 'dlg-row');
    const name = Object.assign(doc.createElement('span'), { className: 'dlg-name', textContent: r.name });
    const rm = button(doc, t('fold.remove'), () => {
      rm.disabled = true;
      d.api.unmount(r.name).then(d.reload, (e: unknown) => { rm.disabled = false; err.textContent = failText(e); });
    }, 'quiet danger');
    el.append(icon('folder'), name, rm);
    return el;
  });
  const none = Object.assign(doc.createElement('p'), { className: 'dlg-dim', textContent: t('fold.none') });
  const add = button(doc, t('fold.add'), () => void showPicker(d, dlg, '/'), 'primary');
  dlg.body.replaceChildren(...(rows.length ? rows : [none]), add, err);
  if (!rows.length) add.focus();
}

async function showPicker(d: FolderDeps, dlg: Dialog, dir: string): Promise<void> {
  const { doc } = d;
  const err = errorLine(doc);
  let v;
  try {
    v = await d.api.volumes(dir);
  } catch (e) {
    err.textContent = failText(e);
    dlg.body.replaceChildren(err, button(doc, t('fold.up'), () => void showPicker(d, dlg, '/')));
    return;
  }
  const at = v.dir === '/';
  const path = Object.assign(doc.createElement('div'), { className: 'dlg-path', textContent: at ? '—' : v.dir });
  const list = row(doc, 'dlg-list');
  for (const f of v.dirs) {
    const b = Object.assign(doc.createElement('button'), { type: 'button', className: 'dlg-item' });
    b.append(icon('folder'), Object.assign(doc.createElement('span'), { textContent: f.name }));
    b.addEventListener('click', () => void showPicker(d, dlg, f.path));
    list.append(b);
  }
  if (!v.dirs.length) list.append(Object.assign(doc.createElement('p'), { className: 'dlg-dim dlg-item', textContent: t('fold.empty') }));
  const up = button(doc, t('fold.up'), () => void showPicker(d, dlg, v.parent ?? '/'));
  up.disabled = v.parent === null;
  const pick = button(doc, t('fold.pick'), () => {
    pick.disabled = true;
    d.api.mount(v.dir).then(d.reload, (e: unknown) => { pick.disabled = false; err.textContent = failText(e); });
  }, 'primary');
  pick.disabled = at; // a volume list is not a folder
  const bar = row(doc, 'dlg-row');
  bar.append(up, pick);
  dlg.body.replaceChildren(path, list, bar, err);
}
