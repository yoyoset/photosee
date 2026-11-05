import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import config from '../../contracts/config.json';
import session from '../../contracts/session.json';
import { ApiError, type AccountRow, type Api, type Session, type Settings, type VolumeListing } from '../src/api';
import { boot, type App } from '../src/controller-boot';
import type { Host } from '../src/host';
import { setLang, t } from '../src/i18n';
import { defaultUiState } from '../src/uistate';

setLang('en');
const flush = async (): Promise<void> => { for (let i = 0; i < 40; i++) await Promise.resolve(); };

const VOLS: Record<string, VolumeListing> = {
  '/': { dir: '/', parent: null, dirs: [{ name: 'disk1', path: '/disk1' }] },
  '/disk1': { dir: '/disk1', parent: '/', dirs: [{ name: 'Comics', path: '/disk1/Comics' }] },
  '/disk1/Comics': { dir: '/disk1/Comics', parent: '/disk1', dirs: [] },
};

function setup(o: { users?: string[]; roots?: Session['roots']; admin?: boolean; loggedIn?: boolean } = {}) {
  document.body.innerHTML = '<main id="stage"></main><div id="overlay"></div><div id="browser"></div><div id="live"></div>';
  let loggedIn = o.loggedIn ?? true;
  const accounts: AccountRow[] = [{ name: 'dad', admin: true, folders: 0 }];
  const api = {
    session: vi.fn(async (): Promise<Session> => {
      if (!loggedIn) throw new ApiError('unauthorized', 'login required');
      return { ...(session as Session), start: null, roots: o.roots ?? [], user: 'dad', admin: o.admin ?? false };
    }),
    config: vi.fn(async (): Promise<Settings> => ({ ...(config as Settings), start_fullscreen: false })),
    list: vi.fn(async () => ({ dir: '/x', parent: null, images: [], dirs: [] })),
    uiState: vi.fn(async () => defaultUiState()),
    saveUiState: vi.fn(async () => {}),
    nav: vi.fn(), meta: vi.fn(() => new Promise(() => {})),
    users: vi.fn(async () => o.users ?? ['dad']),
    setup: vi.fn(async () => { loggedIn = true; }),
    volumes: vi.fn(async (dir = '/') => VOLS[dir] as VolumeListing),
    mount: vi.fn(async () => {}),
    unmount: vi.fn(async () => {}),
    accounts: vi.fn(async () => accounts.slice()),
    createAccount: vi.fn(async (name: string) => {
      if (accounts.some((a) => a.name === name)) throw new ApiError('forbidden', 'name already taken');
      accounts.push({ name, admin: false, folders: 0 });
    }),
    resetPin: vi.fn(async () => {}),
    deleteAccount: vi.fn(async () => {}),
    logout: vi.fn(async () => {}),
  };
  const host: Host = { setFullscreen() {}, isFullscreen: () => false, close: vi.fn() };
  return { api, run: (): Promise<App> => boot({ api: api as unknown as Api, host }) };
}

const q = (s: string): HTMLElement => document.querySelector(s) as HTMLElement;
const byText = (sel: string, text: string): HTMLElement =>
  [...document.querySelectorAll<HTMLElement>(sel)].find((e) => e.textContent === text) as HTMLElement;
const menu = (label: string): void => {
  q('.tb-user').click();
  byText('.br-menu-item', label).click();
};

let app: App | null = null;
beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}); });
afterEach(() => { app?.dispose(); app = null; document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('first run', () => {
  it('with no accounts the login screen creates the admin', async () => {
    const s = setup({ users: [], loggedIn: false });
    const booting = s.run();
    await flush();
    expect(q('.lg-title').textContent).toBe(t('login.setup'));
    (q('.lg-text') as HTMLInputElement).value = 'dad';
    (document.querySelectorAll<HTMLInputElement>('.lg-pin')[1] as HTMLInputElement).value = '1234';
    (q('.lg-form') as HTMLFormElement).requestSubmit();
    app = await booting;
    await app.started;
    expect(s.api.setup).toHaveBeenCalledWith('dad', '1234');
  });
});

describe('my folders', () => {
  it('a reader with no folders lands on the dialog and mounts one from the picker', async () => {
    const s = setup({ roots: [] });
    app = await s.run();
    await app.started;
    expect(q('.dlg-title').textContent).toBe(t('acct.folders'));
    expect(q('.dlg-body .dlg-dim').textContent).toBe(t('fold.none'));
    byText('.dlg-btn', t('fold.add')).click();
    await flush();
    expect(byText('.dlg-btn', t('fold.pick')).hasAttribute('disabled')).toBe(true); // the volume list itself
    byText('.dlg-item', 'disk1').click();
    await flush();
    byText('.dlg-item', 'Comics').click();
    await flush();
    expect(q('.dlg-path').textContent).toBe('/disk1/Comics');
    byText('.dlg-btn', t('fold.pick')).click();
    await flush();
    expect(s.api.mount).toHaveBeenCalledWith('/disk1/Comics');
  });

  it('lists mounted folders and removes one by its root name', async () => {
    const s = setup({ roots: [{ name: 'Comics', path: '/Comics', kind: 'root' }] });
    app = await s.run();
    await app.started;
    expect(document.querySelector('.dlg')).toBeNull(); // has folders: no auto-open
    menu(t('acct.folders'));
    byText('.dlg-btn', t('fold.remove')).click();
    await flush();
    expect(s.api.unmount).toHaveBeenCalledWith('Comics');
  });
});

describe('users (admin)', () => {
  it('only admins see it; create shows the new row and a refusal says why', async () => {
    const s = setup({ roots: [{ name: 'a', path: '/a', kind: 'root' }], admin: true });
    app = await s.run();
    await app.started;
    menu(t('acct.admin'));
    await flush();
    expect(byText('.dlg-name', 'dad' + t('adm.is_admin'))).toBeTruthy();
    expect(byText('.dlg-btn', t('adm.delete'))).toBeUndefined(); // never yourself
    const [name, pin] = [...document.querySelectorAll<HTMLInputElement>('.dlg-in')];
    (name as HTMLInputElement).value = 'kid';
    (pin as HTMLInputElement).value = '2468';
    (q('form.dlg-row') as HTMLFormElement).requestSubmit();
    await flush();
    expect(s.api.createAccount).toHaveBeenCalledWith('kid', '2468');
    expect(byText('.dlg-name', 'kid')).toBeTruthy();
    const again = [...document.querySelectorAll<HTMLInputElement>('.dlg-in')];
    (again[0] as HTMLInputElement).value = 'kid';
    (q('form.dlg-row') as HTMLFormElement).requestSubmit();
    await flush();
    expect(q('.dlg-err').textContent).toContain('name already taken');
  });

  it('delete needs a second tap', async () => {
    const s = setup({ roots: [{ name: 'a', path: '/a', kind: 'root' }], admin: true });
    s.api.accounts.mockResolvedValue([{ name: 'dad', admin: true, folders: 1 }, { name: 'kid', admin: false, folders: 0 }]);
    app = await s.run();
    await app.started;
    menu(t('acct.admin'));
    await flush();
    const del = byText('.dlg-btn', t('adm.delete'));
    del.click();
    expect(s.api.deleteAccount).not.toHaveBeenCalled();
    expect(del.textContent).toBe(t('adm.confirm', { name: 'kid' }));
    del.click();
    expect(s.api.deleteAccount).toHaveBeenCalledWith('kid');
  });

  it('a reader has no Users entry', async () => {
    app = await setup({ roots: [{ name: 'a', path: '/a', kind: 'root' }] }).run();
    await app.started;
    q('.tb-user').click();
    expect(byText('.br-menu-item', t('acct.admin'))).toBeUndefined();
  });
});
