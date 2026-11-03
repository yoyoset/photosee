import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Api, Format, ImageItem, ImageSource, Meta, Session } from '../src/api';
import type { Ctx } from '../src/ctx';
import { setLang } from '../src/i18n';
import { mountHelp } from '../src/help';
import { formatBytes, formatTaken, infoRows, mountInfo } from '../src/info';
import { EscStack, Store, initialState } from '../src/state';

const item = (name: string, size = 2048): ImageItem => ({
  name, path: `/d/${name}`, fmt: 'jpeg' as Format, size, mtime_ms: 0,
});

const fullMeta = (over: Partial<Meta> = {}): Meta => ({
  fmt: 'jpeg', width: 4000, height: 3000, orientation: 'normal', animated: false,
  size: 5_242_880, mtime_ms: 0,
  exif: {
    camera: 'Canon EOS R5', lens: 'RF 24-70mm', iso: 400, exposure: '1/250', f_number: 4,
    focal_mm: 35, taken: '2024:05:01 13:22:10',
  },
  ...over,
});

describe('formatting', () => {
  it('formats byte sizes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5_242_880)).toBe('5.0 MB');
    expect(formatBytes(250 * 1024 * 1024)).toBe('250 MB');
    expect(formatBytes(3 * 1024 ** 3)).toBe('3.0 GB');
  });
  it('turns the EXIF date into a readable one', () => {
    expect(formatTaken('2024:05:01 13:22:10')).toBe('2024-05-01 13:22:10');
    expect(formatTaken('not a date')).toBe('not a date');
  });
});

describe('infoRows', () => {
  beforeEach(() => setLang('en'));

  it('lists everything the meta has, in reading order', () => {
    const rows = infoRows(item('a.jpg'), fullMeta());
    expect(rows.map((r) => r.value)).toEqual([
      '4000 × 3000', '5.0 MB', '2024-05-01 13:22:10', 'Canon EOS R5', 'RF 24-70mm',
      '1/250 s', 'f/4', '400', '35 mm',
    ]);
  });

  it('drops missing EXIF fields instead of showing empty rows', () => {
    const meta = fullMeta({
      exif: { camera: null, lens: null, iso: 100, exposure: null, f_number: 2.8, focal_mm: null, taken: null },
    });
    const rows = infoRows(item('a.jpg'), meta);
    expect(rows.map((r) => r.label)).toEqual(['info.dimensions', 'info.file_size', 'info.f_number', 'info.iso']);
    expect(rows.find((r) => r.label === 'info.f_number')?.value).toBe('f/2.8');
  });

  it('before /meta arrives, shows only what the listing knows', () => {
    expect(infoRows(item('a.jpg', 1536), null)).toEqual([{ label: 'info.file_size', value: '1.5 KB' }]);
    expect(infoRows(null, null)).toEqual([]);
  });

  it('skips dimensions the server could not determine', () => {
    const rows = infoRows(item('a.jpg'), fullMeta({ width: 0, height: 0 }));
    expect(rows.some((r) => r.label === 'info.dimensions')).toBe(false);
  });
});

function setup() {
  const root = document.body.appendChild(document.createElement('div'));
  const store = new Store({
    ...initialState, index: 0,
    listing: { dir: '/d', parent: null, images: [item('a.jpg'), item('b.jpg')], dirs: [] },
  });
  const pending: Array<{ path: string; resolve: (m: Meta) => void; reject: (e: Error) => void }> = [];
  const meta = vi.fn((path: string) => new Promise<Meta>((resolve, reject) => {
    pending.push({ path, resolve, reject });
  }));
  const esc = new EscStack();
  const ctx: Ctx = {
    store, api: { meta } as unknown as Api, src: {} as ImageSource, esc,
    session: { platform: 'windows' } as Session, dispatch: vi.fn(),
  };
  const panel = mountInfo(root, ctx);
  const help = mountHelp(root, ctx);
  const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));
  return { root, store, meta, pending, esc, panel, help, flush };
}

beforeEach(() => setLang('en'));
afterEach(() => { document.body.replaceChildren(); });

describe('mountInfo', () => {
  it('does not touch the network while hidden', async () => {
    const { store, meta, flush } = setup();
    store.set({ index: 1 });
    await flush();
    expect(meta).not.toHaveBeenCalled();
  });

  it('fetches when shown, renders the answer with textContent', async () => {
    const { root, panel, meta, pending, flush } = setup();
    panel.open();
    await flush();
    expect(meta).toHaveBeenCalledWith('/d/a.jpg');
    expect(root.textContent).toContain('a.jpg');
    expect(root.querySelector('dd')?.textContent).toBe('2.0 KB'); // listing size until /meta lands
    pending[0]?.resolve(fullMeta({ exif: { ...fullMeta().exif, camera: '<b>Cam</b>' } }));
    await flush();
    const dd = [...root.querySelectorAll('dd')].map((e) => e.textContent);
    expect(dd).toContain('4000 × 3000');
    expect(dd).toContain('<b>Cam</b>');
    expect(root.querySelector('b')).toBeNull();
    const dt = [...root.querySelectorAll('dt')].map((e) => e.textContent);
    expect(dt).toContain('Camera');
  });

});

describe('mountInfo: staleness and lifecycle', () => {
  it('ignores a stale /meta response after the image changed', async () => {
    const { root, panel, store, pending, flush } = setup();
    panel.open();
    await flush();
    store.set({ index: 1 });
    expect(pending.map((p) => p.path)).toEqual(['/d/a.jpg', '/d/b.jpg']);
    pending[1]?.resolve(fullMeta({ width: 200, height: 100 }));
    await flush();
    pending[0]?.resolve(fullMeta({ width: 9999, height: 9999 }));
    await flush();
    expect(root.textContent).toContain('b.jpg');
    expect(root.textContent).toContain('200 × 100');
    expect(root.textContent).not.toContain('9999');
  });

  it('ignores a response that arrives after the panel was closed', async () => {
    const { root, panel, pending, flush } = setup();
    panel.open();
    await flush();
    panel.close();
    await flush();
    const before = root.innerHTML;
    pending[0]?.resolve(fullMeta());
    await flush();
    expect(root.innerHTML).toBe(before);
  });
});

describe('mountInfo: failure and reuse', () => {
  it('keeps showing name and size when /meta fails', async () => {
    const { root, panel, pending, flush } = setup();
    panel.open();
    await flush();
    pending[0]?.reject(new Error('boom'));
    await flush();
    expect(root.textContent).toContain('a.jpg');
  });

  it('reuses what it already loaded when reopened on the same image', async () => {
    const { root, panel, meta, pending, flush } = setup();
    panel.open();
    await flush();
    pending[0]?.resolve(fullMeta());
    await flush();
    panel.close();
    await flush();
    panel.open();
    await flush();
    expect(meta).toHaveBeenCalledTimes(1);
    expect(root.textContent).toContain('4000 × 3000');
  });

});

describe('mountInfo: panels', () => {
  it('is an Esc layer "panel" only while open, and Esc closes it', async () => {
    const { panel, esc } = setup();
    expect(esc.handle()).toBeNull();
    panel.open();
    expect(esc.handle()).toBe('panel');
    expect(panel.isOpen()).toBe(false);
    expect(esc.handle()).toBeNull();
    panel.open();
    panel.close();
    expect(esc.handle()).toBeNull();
  });

  it('opening info closes help and vice versa', () => {
    const { panel, help, esc } = setup();
    help.open();
    panel.open();
    expect(help.isOpen()).toBe(false);
    expect(panel.isOpen()).toBe(true);
    help.open();
    expect(panel.isOpen()).toBe(false);
    expect(esc.handle()).toBe('panel');
    expect(esc.handle()).toBeNull();
  });

  it('dispose stops reacting and removes its DOM', async () => {
    const { root, store, panel, meta, flush } = setup();
    panel.open();
    panel.dispose();
    store.set({ index: 1 });
    await flush();
    expect(meta).toHaveBeenCalledTimes(1);
    expect(root.querySelector('.ps-info')).toBeNull();
  });
});
