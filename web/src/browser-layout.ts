// The static skeleton of browser mode: toolbar, tree pane, splitter, content
// pane with its empty-folder text, status bar. Built once; browser.ts fills
// and updates it.

import { t } from './i18n';

export interface Skeleton {
  tree$: HTMLElement;
  split: HTMLElement;
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
    tree$: div('br-tree'), split: div('br-split'), empty: div('br-empty'),
    statusL: div('st-l'), statusR: div('st-r'),
  };
  s.tree$.append(treeEl);
  s.empty.textContent = t('br.empty');
  s.empty.hidden = true;
  const pane = div('br-pane');
  pane.append(content, s.empty);
  const main = div('br-main');
  main.append(s.tree$, s.split, pane);
  const status = div('br-status');
  status.append(s.statusL, s.statusR);
  root.replaceChildren(toolbar, main, status);
  return s;
}
