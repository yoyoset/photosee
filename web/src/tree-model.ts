// The folder tree as data: sections, lazily loaded nodes, keyboard movement.
// No DOM here; tree.ts draws it. `list` is injected so tests count calls.

import type { DirItem, Listing, Place } from './api';
import type { IconName } from './browser-icons';
import { ancestorChain, isUnder, normPath, samePath } from './browser-path';
import { t, type Key } from './i18n';

export type SectionId = 'recent' | 'places' | 'network';

export interface TreeNode {
  id: string;
  label: string;
  path: string;
  icon: IconName;
  section: SectionId;
  /** Recent entries are shortcuts: opened, never expanded. */
  expandable: boolean;
  /** Recent and network entries can be taken off the list. */
  removable: boolean;
  /** null until the first expansion; then the subfolders (may be empty). */
  children: TreeNode[] | null;
  expanded: boolean;
  loading: boolean;
  /** Listing failed; shown dimmed, retried on the next expansion. */
  failed: boolean;
}

export type Row =
  | { kind: 'header'; id: string; label: string }
  | { kind: 'node'; node: TreeNode; depth: number; parent: TreeNode | null };

export type ListFn = (dir: string) => Promise<Pick<Listing, 'dirs'>>;

const PLACE_KEYS: Record<string, Key> = {
  pictures: 'br.place.pictures', desktop: 'br.place.desktop', documents: 'br.place.documents',
  downloads: 'br.place.downloads', home: 'br.place.home',
};

/** `folder` places carry a stable English key ("Pictures"); everything else
 * (Docker roots, drives, shares) is already a name the user knows. */
export function placeLabel(p: Place): string {
  const key = p.kind === 'folder' ? PLACE_KEYS[p.name.toLowerCase()] : undefined;
  return key ? t(key) : p.name;
}

const placeIcon = (k: Place['kind']): IconName =>
  k === 'drive' ? 'drive' : k === 'network' ? 'network' : 'folder';

function lastSegment(path: string): string {
  const parts = path.split(/[\\/]+/).filter((s) => s !== '');
  return parts[parts.length - 1] ?? path;
}

function node(section: SectionId, parentId: string, path: string, label: string, icon: IconName, o: Partial<TreeNode> = {}): TreeNode {
  return {
    id: `${parentId}>${path}`, label, path, icon, section, expandable: true, removable: false,
    children: null, expanded: false, loading: false, failed: false, ...o,
  };
}

export class TreeModel {
  private readonly places: TreeNode[];
  private recent: TreeNode[] = [];
  private network: TreeNode[] = [];

  constructor(roots: readonly Place[], private readonly list: ListFn) {
    this.places = roots.map((p) => node('places', 'places', p.path, placeLabel(p), placeIcon(p.kind)));
  }

  setRecent(paths: readonly string[]): void {
    this.recent = paths.map((p) => node('recent', 'recent', p, lastSegment(p), 'clock', { expandable: false, removable: true }));
  }

  /** Keeps the expansion state of entries that stay. */
  setNetwork(paths: readonly string[]): void {
    const old = new Map(this.network.map((n) => [normPath(n.path), n]));
    this.network = paths.map((p) => old.get(normPath(p))
      ?? node('network', 'network', p, p, 'network', { removable: true }));
  }

  rows(): Row[] {
    const out: Row[] = [];
    const sections: [SectionId, Key, TreeNode[]][] = [
      ['recent', 'br.section.recent', this.recent],
      ['places', 'br.section.places', this.places],
      ['network', 'br.section.network', this.network],
    ];
    for (const [id, label, nodes] of sections) {
      if (nodes.length === 0 && id !== 'recent') continue; // Recent always shows: its box has a fixed size
      out.push({ kind: 'header', id, label: t(label) });
      for (const n of nodes) walk(n, 0, null, out);
    }
    return out;
  }

  find(id: string): TreeNode | null {
    for (const r of this.rows()) if (r.kind === 'node' && r.node.id === id) return r.node;
    return null;
  }

  /** One /list the first time (or after a failure); a no-op once loaded. */
  async expand(n: TreeNode): Promise<void> {
    if (!n.expandable || n.loading) return;
    if (n.children === null || n.failed) {
      n.loading = true;
      n.failed = false;
      try {
        n.children = childNodes(n, (await this.list(n.path)).dirs);
      } catch {
        n.children = [];
        n.failed = true;
      }
      n.loading = false;
    }
    n.expanded = !n.failed;
  }

  collapse(n: TreeNode): void { n.expanded = false; }

  /** Expand every ancestor of `dir` below the place that contains it, loading
   * only what is not loaded yet. Returns the node for `dir` if it is reachable. */
  async reveal(dir: string, isStale: () => boolean = () => false): Promise<TreeNode | null> {
    const roots = [...this.places, ...this.network].filter((n) => isUnder(n.path, dir));
    roots.sort((a, b) => normPath(b.path).length - normPath(a.path).length);
    const root = roots[0];
    if (!root) return null;
    const chain = ancestorChain(root.path, dir);
    let cur = root;
    for (const next of chain.slice(1)) {
      if (!cur.expanded) await this.expand(cur);
      if (isStale()) return null;
      const child: TreeNode | undefined = cur.children?.find((c) => samePath(c.path, next));
      if (!child) return null;
      cur = child;
    }
    return cur;
  }

  /** The node that stands for `dir`: places, then network places, then recents. */
  nodeFor(dir: string | null): TreeNode | null {
    if (dir === null) return null;
    const rows = this.rows();
    for (const section of ['places', 'network', 'recent'] as const) {
      for (const r of rows) {
        if (r.kind === 'node' && r.node.section === section && samePath(r.node.path, dir)) return r.node;
      }
    }
    return null;
  }
}

function walk(n: TreeNode, depth: number, parent: TreeNode | null, out: Row[]): void {
  out.push({ kind: 'node', node: n, depth, parent });
  if (n.expanded) for (const c of n.children ?? []) walk(c, depth + 1, n, out);
}

function childNodes(parent: TreeNode, dirs: readonly DirItem[]): TreeNode[] {
  return dirs.map((d) => node(parent.section, parent.id, d.path, d.name, 'folder'));
}

export type TreeKey = 'up' | 'down' | 'left' | 'right' | 'home' | 'end' | 'enter';
export interface TreeMove { focus?: string; expand?: string; collapse?: string; open?: string }

/** Keyboard behaviour (WAI-ARIA tree): arrows move, Left/Right fold, Enter opens. */
export function treeKey(rows: readonly Row[], focusId: string | null, key: TreeKey): TreeMove {
  const nodes = rows.filter((r): r is Extract<Row, { kind: 'node' }> => r.kind === 'node');
  if (nodes.length === 0) return {};
  const at = nodes.findIndex((r) => r.node.id === focusId);
  const cur = nodes[at];
  const go = (i: number): TreeMove => ({ focus: (nodes[Math.min(nodes.length - 1, Math.max(0, i))] as typeof nodes[number]).node.id });
  if (!cur) return key === 'end' ? go(nodes.length - 1) : go(0);
  switch (key) {
    case 'up': return go(at - 1);
    case 'down': return go(at + 1);
    case 'home': return go(0);
    case 'end': return go(nodes.length - 1);
    case 'enter': return { open: cur.node.id };
    case 'right':
      if (!cur.node.expandable) return {};
      if (!cur.node.expanded) return { expand: cur.node.id };
      return cur.node.children?.length ? go(at + 1) : {};
    case 'left':
      if (cur.node.expanded) return { collapse: cur.node.id };
      return cur.parent ? { focus: cur.parent.id } : {};
  }
}
