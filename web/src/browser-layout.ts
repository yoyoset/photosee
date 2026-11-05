// The static skeleton of browser mode: toolbar, tree pane, splitter, content
// pane with its empty-folder text, status bar. Built once; browser.ts fills
// and updates it.

import { TreeDrawer } from './browser-drawer';
import { t } from './i18n';

export interface Skeleton {
  /** Owns the tree pane, the splitter and the narrow-window scrim. */
  drawer: TreeDrawer;
  empty: HTMLElement;
  statusL: HTMLElement;
  statusR: HTMLElement;
}

const div = (cls: string): HTMLElement => Object.assign(document.createElement('div'), { className: cls });

export function buildSkeleton(
  root: HTMLElement, toolbar: HTMLElement, treeEl: HTMLElement, content: HTMLElement,
): Skeleton {
  root.classList.add('br');
  root.hidden = true;
  const s: Skeleton = {
    drawer: new TreeDrawer(div('br-tree'), div('br-split')), empty: div('br-empty'),
    statusL: div('st-l'), statusR: div('st-r'),
  };
  s.drawer.tree$.append(treeEl);
  s.empty.textContent = t('br.empty');
  s.empty.hidden = true;
  const pane = div('br-pane');
  pane.append(content, s.empty);
  const main = div('br-main');
  main.append(s.drawer.tree$, s.drawer.split, s.drawer.scrim, pane);
  const status = div('br-status');
  status.append(s.statusL, s.statusR);
  root.replaceChildren(toolbar, main, status);
  return s;
}
