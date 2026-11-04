// The folder tree pane. TreeModel owns the data and the lazy loading; this
// file draws it (a flat list of rows, indent by depth) and routes input.

import { icon } from './browser-icons';
import { t } from './i18n';
import { treeKey, type Row, type TreeKey, type TreeModel, type TreeNode } from './tree-model';

export interface TreeHooks {
  /** Navigate to a folder (click or Enter). */
  open(path: string): void;
  /** Take a recent or network entry off its list. */
  remove(node: TreeNode): void;
}

const KEYS: Record<string, TreeKey> = {
  ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
  Home: 'home', End: 'end', Enter: 'enter',
};
const INDENT_PX = 14;

export class TreeView {
  readonly el = document.createElement('div');
  private focusId: string | null = null;
  private current: string | null = null;
  /** Bumped per reveal, so an older (slower) one cannot expand over a newer one. */
  private seq = 0;

  constructor(private readonly model: TreeModel, private readonly hooks: TreeHooks) {
    this.el.className = 'tr';
    this.el.tabIndex = 0;
    this.el.setAttribute('role', 'tree');
    this.el.setAttribute('aria-label', t('br.tree'));
    this.el.addEventListener('click', (e) => this.onClick(e));
    this.el.addEventListener('dblclick', (e) => this.onDouble(e));
    this.el.addEventListener('keydown', (e) => this.onKey(e));
  }

  /** The folder now showing: highlight it and open the way to it. */
  setCurrent(dir: string | null): void {
    this.current = dir;
    this.render();
    if (dir === null) return;
    const mine = ++this.seq;
    void this.model.reveal(dir, () => mine !== this.seq).then((n) => {
      if (mine !== this.seq) return;
      if (n) this.focusId = n.id;
      this.render();
      this.el.querySelector('.cur')?.scrollIntoView?.({ block: 'nearest' });
    });
  }

  render(): void {
    const cur = this.model.nodeFor(this.current);
    const rows = this.model.rows();
    this.el.replaceChildren(...rows.map((r) => (r.kind === 'header' ? this.header(r) : this.row(r, cur))));
  }

  private header(r: Extract<Row, { kind: 'header' }>): HTMLElement {
    const h = document.createElement('div');
    h.className = 'tr-head';
    h.textContent = r.label;
    h.setAttribute('role', 'presentation');
    return h;
  }

  private row(r: Extract<Row, { kind: 'node' }>, cur: TreeNode | null): HTMLElement {
    const n = r.node;
    const row = document.createElement('div');
    row.className = 'tr-row';
    row.setAttribute('role', 'treeitem');
    row.setAttribute('aria-level', String(r.depth + 1));
    row.setAttribute('aria-selected', String(n === cur));
    if (n.expandable) row.setAttribute('aria-expanded', String(n.expanded));
    row.dataset['id'] = n.id;
    row.title = n.path;
    row.style.paddingLeft = `${6 + r.depth * INDENT_PX}px`;
    row.classList.toggle('cur', n === cur);
    row.classList.toggle('focus', n.id === this.focusId);
    row.classList.toggle('failed', n.failed);
    const chev = document.createElement('span');
    chev.className = `tr-chev${n.expandable ? '' : ' none'}${n.expanded ? ' open' : ''}${n.loading ? ' busy' : ''}`;
    if (n.expandable) chev.append(icon('chevron'));
    const label = document.createElement('span');
    label.className = 'tr-lbl';
    label.textContent = n.label;
    row.append(chev, icon(n.icon), label);
    if (n.removable) row.append(this.removeButton());
    return row;
  }

  private removeButton(): HTMLElement {
    const b = document.createElement('button');
    b.className = 'tr-x';
    b.type = 'button';
    b.tabIndex = -1;
    b.title = t('br.remove');
    b.setAttribute('aria-label', t('br.remove'));
    b.append(icon('close'));
    return b;
  }

  private nodeOf(e: Event): { node: TreeNode; target: Element } | null {
    const target = e.target as Element;
    const id = target.closest<HTMLElement>('.tr-row')?.dataset['id'];
    const node = id ? this.model.find(id) : null;
    return node ? { node, target } : null;
  }

  private onClick(e: MouseEvent): void {
    const hit = this.nodeOf(e);
    if (!hit) return;
    this.focusId = hit.node.id;
    this.el.focus({ preventScroll: true });
    if (hit.target.closest('.tr-x')) return this.hooks.remove(hit.node);
    if (hit.target.closest('.tr-chev') && hit.node.expandable) return void this.toggle(hit.node);
    this.render();
    this.hooks.open(hit.node.path);
  }

  private onDouble(e: MouseEvent): void {
    const hit = this.nodeOf(e);
    if (hit && !hit.target.closest('.tr-x, .tr-chev') && hit.node.expandable) void this.toggle(hit.node);
  }

  private async toggle(n: TreeNode, expand = !n.expanded): Promise<void> {
    if (expand) {
      const loading = this.model.expand(n); // runs up to its first await: `loading` is set
      this.render();
      await loading;
    } else {
      this.model.collapse(n);
    }
    this.render();
  }

  private onKey(e: KeyboardEvent): void {
    const key = KEYS[e.key];
    if (!key || e.altKey || e.ctrlKey || e.metaKey) return;
    e.preventDefault();
    e.stopPropagation();
    const move = treeKey(this.model.rows(), this.focusId, key);
    if (move.focus) this.focusId = move.focus;
    const target = move.expand ?? move.collapse ?? move.open;
    const node = target ? this.model.find(target) : null;
    if (node && move.expand) return void this.toggle(node, true);
    if (node && move.collapse) return void this.toggle(node, false);
    this.render();
    this.el.querySelector('.focus')?.scrollIntoView?.({ block: 'nearest' });
    if (node && move.open) this.hooks.open(node.path);
  }
}
