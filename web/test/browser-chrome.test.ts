import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type Api, type ImageItem, type Listing, type Session, type Settings } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang, t } from '../src/i18n';
import { defaultUiState } from '../src/uistate';

setLang('en');
const flush = async (): Promise<void> => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
const img = (dir: string, n: string): ImageItem => ({ name: n, path: `${dir}/${n}`, fmt: 'jpeg', size: 1500, mtime_ms: 0 });
const L = (dir: string, names: string[], dirs: string[], parent: string | null): Listing => ({
  dir, parent, images: names.map((n) => img(dir, n)), dirs: dirs.map((d) => ({ name: d, path: `${dir}/${d}` })),
});
const TREE: Record<string, Listing> = {
  '/photos': L('/photos', ['b.jpg', 'a.jpg'], ['2024'], null),
  '/photos/2024': L('/photos/2024', ['x.jpg'], ['may'], '/photos'),
  '/photos/2024/may': L('/photos/2024/may', [], [], '/photos/2024'),
};

function setup(opts: { pick?: Host['pickFolder']; meta?: Api['meta'] } = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  const api = {
    session: vi.fn(async (): Promise<Session> => ({ ...(session as Session), mode: 'desktop', start: { dir: '/photos', file: null } })),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async (dir: string) => {
      const l = TREE[dir];
      if (!l) throw new ApiError('not_found', dir);
      return l;
    }),
    uiState: vi.fn(async () => defaultUiState()),
    saveUiState: vi.fn<(s: Record<string, unknown>) => Promise<void>>(async () => {}),
    nav: vi.fn(),
    meta: vi.fn(opts.meta ?? (() => new Promise(() => {}))),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  if (opts.pick) host.pickFolder = opts.pick;
  return { api, host, run: (): Promise<App> => boot({ api: api as unknown as Api, host }) };
}

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); vi.useRealTimers(); });

const press = (el: Element, k: string, init: KeyboardEventInit = {}): void => {
  el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...init }));
};
const input = (): HTMLInputElement | null => document.querySelector('.addr-in');

describe('address bar', () => {
  it('shows breadcrumbs; a click on one navigates', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    await app.modes.peekBrowser()?.nav.go('/photos/2024');
    expect([...document.querySelectorAll('.crumb')].map((c) => c.textContent)).toEqual(['photos', '2024']);
    (document.querySelector('.crumb') as HTMLElement).click();
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos');
  });

  it('Ctrl+L, or a click on empty space, turns it into a field; Enter navigates to the typed path', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'l', ctrlKey: true, cancelable: true }));
    const field = input() as HTMLInputElement;
    expect(field.value).toBe('/photos');
    field.value = '/photos/2024/may';
    press(field, 'Enter');
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024/may');
    expect(input()).toBeNull(); // back to breadcrumbs
    expect(app.ctx.store.get().root).toBe('/photos');
    expect(document.querySelector('.addr-crumbs')?.textContent).toContain('may');
  });

  it('a path that fails shows the reason inline, keeps the text, and opens no dialog or banner', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    (document.querySelector('.addr-crumbs') as HTMLElement).click(); // empty space
    const field = input() as HTMLInputElement;
    field.value = '/nowhere';
    press(field, 'Enter');
    await flush();
    const err = document.querySelector<HTMLElement>('.addr-err');
    expect(err?.hidden).toBe(false);
    expect(err?.textContent).toBe(t('err.not_found'));
    expect(input()?.value).toBe('/nowhere');
    expect(document.querySelector('.ps-banner, .ps-toast')).toBeNull();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos'); // the old folder stays up
    field.value = '/photos';
    field.dispatchEvent(new Event('input'));
    expect(err?.hidden).toBe(true); // typing clears the message
  });

});

describe('address bar: editing', () => {
  it('Esc cancels editing; a UNC path that opens joins the network places', async () => {
    const s = setup();
    TREE['\\\\nas\\pics'] = L('\\\\nas\\pics', ['n.jpg'], [], null);
    app = await s.run();
    await app.started;
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'l', ctrlKey: true }));
    press(input() as HTMLInputElement, 'Escape');
    expect(input()).toBeNull();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'l', ctrlKey: true }));
    const field = input() as HTMLInputElement;
    field.value = '\\\\nas\\pics';
    press(field, 'Enter');
    await flush();
    expect(app.prefs.get().network).toEqual(['\\\\nas\\pics']);
    expect(app.prefs.get().recent[0]).toBe('\\\\nas\\pics');
    expect([...document.querySelectorAll('.tr-head')].map((h) => h.textContent)).toContain(t('br.section.network'));
    delete TREE['\\\\nas\\pics'];
  });
});

describe('tree', () => {
  const listCalls = (s: ReturnType<typeof setup>, dir: string): number =>
    s.api.list.mock.calls.filter((c) => c[0] === dir).length;

  it('expands lazily: one /list per expansion, none for a collapse and re-open', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    await flush();
    expect(listCalls(s, '/photos')).toBe(1); // the folder being shown
    const chev = (): HTMLElement => document.querySelector('.tr-row .tr-chev:not(.none)') as HTMLElement;
    chev().click();
    await flush();
    expect(listCalls(s, '/photos')).toBe(2);
    expect([...document.querySelectorAll('.tr-lbl')].map((l) => l.textContent)).toContain('2024');
    chev().click(); // collapse
    chev().click(); // open again, from memory
    await flush();
    expect(listCalls(s, '/photos')).toBe(2);
    expect(s.api.list).toHaveBeenCalledTimes(2);
  });

  it('opens the way to the current folder and highlights it', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    await app.modes.peekBrowser()?.nav.go('/photos/2024/may');
    await flush();
    const cur = document.querySelector('.tr-row.cur .tr-lbl');
    expect(cur?.textContent).toBe('may');
    expect(listCalls(s, '/photos')).toBeLessThanOrEqual(3);
  });

});

describe('tree: opening', () => {
  it('click opens the folder; the recent list gets it and a remove button drops it', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    (document.querySelector('.tr-row .tr-chev:not(.none)') as HTMLElement).click();
    await flush();
    const row = [...document.querySelectorAll<HTMLElement>('.tr-row')].find((r) => r.textContent === '2024') as HTMLElement;
    row.click();
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
    expect(app.prefs.get().recent[0]).toBe('/photos/2024');
    const recent = document.querySelector('.tr-row [aria-label]')?.closest('.tr-row') as HTMLElement;
    (recent.querySelector('.tr-x') as HTMLElement).click();
    expect(app.prefs.get().recent).not.toContain('/photos/2024');
  });

  it('arrow keys move, Right expands, Enter opens', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    await flush();
    const tree = document.querySelector('.tr') as HTMLElement;
    press(tree, 'ArrowRight');
    await flush();
    press(tree, 'ArrowDown');
    press(tree, 'Enter');
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
  });
});

describe('toolbar', () => {
  const pickBtn = (): Element | null => document.querySelector(`.tbar button[aria-label="${t('br.pick')}"]`);

  it('hides the folder picker when the host has none, shows it otherwise', async () => {
    let s = setup();
    app = await s.run();
    expect(pickBtn()).toBeNull();
    app.dispose();
    s = setup({ pick: vi.fn(async () => '/photos/2024') });
    app = await s.run();
    await app.started;
    (pickBtn() as HTMLElement).click();
    await flush();
    expect(app.ctx.store.get().listing?.dir).toBe('/photos/2024');
  });

  it('sort changes re-order the listing and keep the selected image', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    expect(app.ctx.store.get().listing?.images.map((i) => i.name)).toEqual(['a.jpg', 'b.jpg']);
    app.prefs.set({ sort: { key: 'name', dir: 'desc' } });
    expect(app.ctx.store.get().listing?.images.map((i) => i.name)).toEqual(['b.jpg', 'a.jpg']);
    expect(document.querySelector('.gc.sel')?.getAttribute('title')).toBe('2024');
  });

});

describe('toolbar: size and layout', () => {
  it('the thumbnail slider and Ctrl+wheel resize cells and persist after 500 ms', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    vi.useFakeTimers();
    const wheel = new WheelEvent('wheel', { ctrlKey: true, deltaY: -100, cancelable: true, bubbles: true });
    document.querySelector('.vv')?.dispatchEvent(wheel);
    expect(wheel.defaultPrevented).toBe(true);
    expect(app.prefs.get().thumb).toBe(176);
    expect((document.querySelector('.vv') as HTMLElement).style.getPropertyValue('--th')).toBe('176px');
    await vi.advanceTimersByTimeAsync(499);
    expect(s.api.saveUiState).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2);
    expect(s.api.saveUiState).toHaveBeenCalledOnce();
    expect(s.api.saveUiState.mock.calls[0]?.[0]).toMatchObject({ version: 1, thumb: 176 });
  });

  it('the splitter resizes the tree within 160..480 px by keyboard', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    const split = document.querySelector('.br-split') as HTMLElement;
    press(split, 'ArrowRight');
    expect(app.prefs.get().tree.width).toBe(236);
    for (let i = 0; i < 30; i++) press(split, 'ArrowRight');
    expect(app.prefs.get().tree.width).toBe(480);
    for (let i = 0; i < 40; i++) press(split, 'ArrowLeft');
    expect(app.prefs.get().tree.width).toBe(160);
    expect((document.querySelector('#browser') as HTMLElement).style.getPropertyValue('--tree-w')).toBe('160px');
  });

  it('the tree toggle hides the pane and remembers it', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    (document.querySelector(`.tbar button[aria-label="${t('br.tree')}"]`) as HTMLElement).click();
    expect((document.querySelector('.br-tree') as HTMLElement).hidden).toBe(true);
    expect(app.prefs.get().tree.visible).toBe(false);
  });
});

describe('details list', () => {
  it('switches view, sorts by a header click and reverses on the second click', async () => {
    const s = setup();
    app = await s.run();
    await app.started;
    app.prefs.set({ view: 'details' });
    expect(document.querySelectorAll('.dr').length).toBe(3); // one folder, two images
    (document.querySelector('.dh-name') as HTMLElement).click();
    expect(app.prefs.get().sort).toMatchObject({ key: 'name', dir: 'desc' });
    (document.querySelector('.dh-size') as HTMLElement).click();
    expect(app.prefs.get().sort).toMatchObject({ key: 'size', dir: 'asc' });
    expect(document.querySelector('.dh-size')?.getAttribute('aria-sort')).toBe('ascending');
  });

  it('fills dimensions from /meta for visible rows only, after the debounce', async () => {
    const s = setup({ meta: async () => ({ width: 6000, height: 4000 }) as never });
    app = await s.run();
    await app.started;
    vi.useFakeTimers();
    app.prefs.set({ view: 'details' });
    expect(s.api.meta).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(200);
    expect(s.api.meta).toHaveBeenCalledTimes(2); // the two images; folders have none
    expect(document.querySelector('.dr .dc-dims')?.textContent).toBe('');
    expect([...document.querySelectorAll('.dc-dims')].map((c) => c.textContent)).toContain('6000×4000');
  });
});
