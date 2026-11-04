import { describe, expect, it, vi } from 'vitest';
import type { DirItem, Place } from '../src/api';
import { setLang } from '../src/i18n';
import { TreeModel, placeLabel, treeKey, type ListFn } from '../src/tree-model';

setLang('en');

const dirs = (...paths: string[]): DirItem[] => paths.map((p) => ({ name: p.split(/[\\/]/).pop() as string, path: p }));
const places: Place[] = [
  { name: 'Pictures', path: 'C:\\Users\\me\\Pictures', kind: 'folder' },
  { name: 'C:', path: 'C:\\', kind: 'drive' },
];

function fake(tree: Record<string, string[]>): { list: ListFn; calls: string[] } {
  const calls: string[] = [];
  const list = vi.fn(async (dir: string) => {
    calls.push(dir);
    const kids = tree[dir];
    if (!kids) throw new Error('no such dir');
    return { dirs: dirs(...kids) };
  });
  return { list, calls };
}

describe('tree sections', () => {
  it('labels known folders through i18n and leaves drives and roots alone', () => {
    expect(placeLabel(places[0] as Place)).toBe('Pictures');
    setLang('zh');
    expect(placeLabel(places[0] as Place)).toBe('图片');
    expect(placeLabel({ name: 'Desktop', path: 'x', kind: 'folder' })).toBe('桌面');
    expect(placeLabel({ name: 'Weird', path: 'x', kind: 'folder' })).toBe('Weird');
    expect(placeLabel(places[1] as Place)).toBe('C:');
    setLang('en');
  });

  it('orders recent, places, network; only Recent shows when empty (its box has a fixed size)', () => {
    const m = new TreeModel(places, fake({}).list);
    expect(m.rows().filter((r) => r.kind === 'header').map((r) => r.kind === 'header' && r.label)).toEqual(['Recent', 'Places']);
    m.setRecent(['D:\\a', 'D:\\b']);
    m.setNetwork(['\\\\nas\\x']);
    const labels = m.rows().map((r) => (r.kind === 'header' ? `# ${r.label}` : r.node.label));
    expect(labels).toEqual(['# Recent', 'a', 'b', '# Places', 'Pictures', 'C:', '# Network places', '\\\\nas\\x']);
  });

  it('recent entries are shortcuts and removable; places are not', () => {
    const m = new TreeModel(places, fake({}).list);
    m.setRecent(['D:\\a']);
    const nodes = m.rows().flatMap((r) => (r.kind === 'node' ? [r.node] : []));
    expect(nodes.map((n) => [n.label, n.expandable, n.removable])).toEqual([
      ['a', false, true], ['Pictures', true, false], ['C:', true, false],
    ]);
  });
});

describe('lazy expansion', () => {
  it('lists once per expansion and never again for a loaded node', async () => {
    const f = fake({ 'C:\\': ['C:\\Users', 'C:\\Temp'], 'C:\\Users': ['C:\\Users\\me'] });
    const m = new TreeModel(places, f.list);
    const c = m.nodeFor('C:\\') as NonNullable<ReturnType<TreeModel['nodeFor']>>;
    expect(f.calls).toEqual([]); // building the tree lists nothing
    await m.expand(c);
    expect(f.calls).toEqual(['C:\\']);
    m.collapse(c);
    await m.expand(c);
    expect(f.calls).toEqual(['C:\\']); // reopened from memory
    const users = c.children?.[0];
    await m.expand(users as NonNullable<typeof users>);
    expect(f.calls).toEqual(['C:\\', 'C:\\Users']);
    expect(m.rows().map((r) => (r.kind === 'node' ? `${r.depth}${r.node.label}` : '')).filter(Boolean))
      .toEqual(['0Pictures', '0C:', '1Users', '2me', '1Temp']);
  });

  it('a failing folder is marked, stays closed and is retried on the next try', async () => {
    const f = fake({});
    const m = new TreeModel(places, f.list);
    const c = m.nodeFor('C:\\') as NonNullable<ReturnType<TreeModel['nodeFor']>>;
    await m.expand(c);
    expect(c.failed).toBe(true);
    expect(c.expanded).toBe(false);
    await m.expand(c);
    expect(f.calls).toHaveLength(2);
  });

});

describe('tree reveal', () => {
  it('reveal opens each ancestor with one list, then nothing more on a repeat', async () => {
    const f = fake({
      'C:\\': ['C:\\Users'], 'C:\\Users': ['C:\\Users\\me'], 'C:\\Users\\me': ['C:\\Users\\me\\Pics'],
      'C:\\Users\\me\\Pics': [],
    });
    const m = new TreeModel([{ name: 'C:', path: 'C:\\', kind: 'drive' }], f.list);
    const node = await m.reveal('c:/users/me/pics');
    expect(node?.label).toBe('Pics');
    expect(f.calls).toEqual(['C:\\', 'C:\\Users', 'C:\\Users\\me']); // not the target itself
    expect(m.nodeFor('C:\\Users\\me\\Pics')).toBe(node);
    await m.reveal('C:\\Users\\me\\Pics');
    expect(f.calls).toHaveLength(3);
  });

  it('prefers the deepest place that contains the folder', async () => {
    const f = fake({ 'C:\\Users\\me\\Pictures': ['C:\\Users\\me\\Pictures\\2024'], 'C:\\': [] });
    const m = new TreeModel(places, f.list);
    const node = await m.reveal('C:\\Users\\me\\Pictures\\2024');
    expect(node?.section).toBe('places');
    expect(f.calls).toEqual(['C:\\Users\\me\\Pictures']);
  });

  it('a folder outside every place, or a stale request, reveals nothing', async () => {
    const m = new TreeModel(places, fake({ 'C:\\': ['C:\\a'] }).list);
    expect(await m.reveal('E:\\nowhere')).toBeNull();
    expect(await m.reveal('C:\\a\\b', () => true)).toBeNull();
  });

  it('keeps network entries expanded when the list changes', async () => {
    const f = fake({ '\\\\nas\\x': ['\\\\nas\\x\\a'] });
    const m = new TreeModel([], f.list);
    m.setNetwork(['\\\\nas\\x']);
    await m.expand(m.nodeFor('\\\\nas\\x') as NonNullable<ReturnType<TreeModel['nodeFor']>>);
    m.setNetwork(['\\\\nas\\x', '\\\\nas\\y']);
    expect(m.nodeFor('\\\\nas\\x')?.expanded).toBe(true);
    expect(f.calls).toHaveLength(1);
  });
});

describe('tree keyboard', () => {
  const setup = async () => {
    const m = new TreeModel(places, fake({ 'C:\\': ['C:\\a'] }).list);
    await m.expand(m.nodeFor('C:\\') as NonNullable<ReturnType<TreeModel['nodeFor']>>);
    return m;
  };

  it('Up / Down move the focus, clamped; Home / End jump', async () => {
    const m = await setup();
    const rows = m.rows();
    const id = (label: string): string => {
      const r = rows.find((x) => x.kind === 'node' && x.node.label === label);
      return r?.kind === 'node' ? r.node.id : '';
    };
    expect(treeKey(rows, id('Pictures'), 'down').focus).toBe(id('C:'));
    expect(treeKey(rows, id('Pictures'), 'up').focus).toBe(id('Pictures'));
    expect(treeKey(rows, id('C:'), 'end').focus).toBe(id('a'));
    expect(treeKey(rows, null, 'down').focus).toBe(id('Pictures'));
  });

  it('Right expands a closed node, then enters it; Left collapses, then goes to the parent', async () => {
    const m = await setup();
    const rows = m.rows();
    const pics = rows.find((x) => x.kind === 'node' && x.node.label === 'Pictures');
    const drive = rows.find((x) => x.kind === 'node' && x.node.label === 'C:');
    const child = rows.find((x) => x.kind === 'node' && x.node.label === 'a');
    if (pics?.kind !== 'node' || drive?.kind !== 'node' || child?.kind !== 'node') throw new Error('rows');
    expect(treeKey(rows, pics.node.id, 'right')).toEqual({ expand: pics.node.id });
    expect(treeKey(rows, drive.node.id, 'right')).toEqual({ focus: child.node.id });
    expect(treeKey(rows, drive.node.id, 'left')).toEqual({ collapse: drive.node.id });
    expect(treeKey(rows, child.node.id, 'left')).toEqual({ focus: drive.node.id });
    expect(treeKey(rows, pics.node.id, 'left')).toEqual({});
  });

  it('Enter opens the focused node', async () => {
    const m = await setup();
    const first = m.rows().find((r) => r.kind === 'node');
    if (first?.kind !== 'node') throw new Error('rows');
    expect(treeKey(m.rows(), first.node.id, 'enter')).toEqual({ open: first.node.id });
  });
});
