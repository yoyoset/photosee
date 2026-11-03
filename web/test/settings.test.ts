import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError, type Api, type Session, type Settings } from '../src/api';
import type { Ctx } from '../src/ctx';
import { setLang, t } from '../src/i18n';
import { mountSettings } from '../src/settings';
import { EscStack, Store, initialState } from '../src/state';

const BASE: Settings = {
  interval_s: 5, on_end: 'next_folder', shuffle: false,
  transition: 'none', start_fullscreen: true, wheel: 'navigate',
};

function setup(save: (s: Settings) => Promise<Settings> = async (s) => s) {
  const saveConfig = vi.fn(save);
  const ctx = {
    store: new Store({ ...initialState, settings: BASE }),
    esc: new EscStack(),
    api: { saveConfig } as unknown as Api,
    session: { platform: 'windows' } as Session,
    src: {}, dispatch: vi.fn(),
  } as unknown as Ctx;
  const opener = document.body.appendChild(document.createElement('button'));
  const panel = mountSettings(document.body, ctx);
  const q = <T extends HTMLElement>(sel: string): T => {
    const e = document.querySelector<T>(sel);
    if (!e) throw new Error(`missing ${sel}`);
    return e;
  };
  const fire = (e: HTMLElement, type = 'change'): void => void e.dispatchEvent(new Event(type, { bubbles: true }));
  return { ctx, saveConfig, opener, panel, q, fire };
}

beforeEach(() => {
  setLang('en');
  document.body.replaceChildren();
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe('settings panel: content', () => {
  it('has exactly the six settings with labelled native controls', () => {
    const { panel } = setup();
    panel.open();
    const names = new Set(Array.from(document.querySelectorAll('input[name]')).map((i) => i.getAttribute('name')));
    expect([...names].sort()).toEqual(['interval_s', 'on_end', 'shuffle', 'start_fullscreen', 'transition', 'wheel']);
    expect(document.querySelectorAll('input[type=radio]').length).toBe(3 + 2 + 2);
    expect(document.querySelectorAll('input[type=checkbox]').length).toBe(2);
    expect(document.querySelectorAll('input[type=number], input[type=range]').length).toBe(2);
    for (const i of Array.from(document.querySelectorAll('input'))) {
      const named = i.labels?.length || i.getAttribute('aria-label');
      expect(named, i.outerHTML).toBeTruthy();
    }
    // every radio label resolved to a real string (the key is built at runtime)
    expect(document.querySelector('form')?.textContent).not.toContain('undefined');
  });

  it('shows the stored values on open', () => {
    const { panel, q } = setup();
    panel.open();
    expect(q<HTMLInputElement>('input[name=interval_s]').value).toBe('5');
    expect(q<HTMLInputElement>('input[name=on_end][value=next_folder]').checked).toBe(true);
    expect(q<HTMLInputElement>('input[name=start_fullscreen]').checked).toBe(true);
    expect(q<HTMLInputElement>('input[name=shuffle]').checked).toBe(false);
  });

});

describe('settings panel: saving', () => {
  it('several changes produce one debounced save with the full settings', async () => {
    const { panel, q, fire, saveConfig } = setup();
    panel.open();
    q<HTMLInputElement>('input[name=shuffle]').checked = true;
    fire(q('input[name=shuffle]'));
    await vi.advanceTimersByTimeAsync(200);
    q<HTMLInputElement>('input[name=transition][value=fade]').checked = true;
    fire(q('input[name=transition][value=fade]'));
    await vi.advanceTimersByTimeAsync(299);
    expect(saveConfig).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(saveConfig).toHaveBeenCalledOnce();
    expect(saveConfig).toHaveBeenCalledWith({ ...BASE, shuffle: true, transition: 'fade' });
  });

  it('writes the normalized server value back to the store and the control', async () => {
    const { panel, q, fire, ctx } = setup(async (s) => ({ ...s, interval_s: 25 }));
    panel.open();
    q<HTMLInputElement>('input[name=interval_s]').value = '30';
    fire(q('input[name=interval_s]'), 'input');
    await vi.advanceTimersByTimeAsync(300);
    expect(ctx.store.get().settings?.interval_s).toBe(25);
    expect(q<HTMLInputElement>('input[name=interval_s]').value).toBe('25');
    expect(q<HTMLInputElement>('input[type=range]').value).toBe('25');
  });

  it('the slider drives the number and saves', async () => {
    const { panel, q, fire, saveConfig } = setup();
    panel.open();
    q<HTMLInputElement>('input[type=range]').value = '12';
    fire(q('input[type=range]'), 'input');
    expect(q<HTMLInputElement>('input[name=interval_s]').value).toBe('12');
    await vi.advanceTimersByTimeAsync(300);
    expect(saveConfig).toHaveBeenCalledWith({ ...BASE, interval_s: 12 });
  });

});

describe('settings panel: failures and validation', () => {
  it('a failed save shows the error inline and keeps the user value', async () => {
    const { panel, q, fire, ctx } = setup(async () => {
      throw new ApiError('network', 'ECONNREFUSED');
    });
    panel.open();
    q<HTMLInputElement>('input[name=shuffle]').checked = true;
    fire(q('input[name=shuffle]'));
    await vi.advanceTimersByTimeAsync(300);
    expect(q('.ps-error').textContent).toBe(`${t('settings.save_failed')}: ${t('err.network')}`);
    expect(q<HTMLInputElement>('input[name=shuffle]').checked).toBe(true);
    expect(ctx.store.get().settings).toEqual(BASE); // not claimed as saved
    panel.close();
    panel.open(); // reopening must not revert the unsaved value
    expect(q<HTMLInputElement>('input[name=shuffle]').checked).toBe(true);
  });

  it('an invalid interval is flagged and not saved; blur clamps it', async () => {
    const { panel, q, fire, saveConfig } = setup();
    panel.open();
    const num = q<HTMLInputElement>('input[name=interval_s]');
    num.value = '99';
    fire(num, 'input');
    expect(num.getAttribute('aria-invalid')).toBe('true');
    await vi.advanceTimersByTimeAsync(300);
    expect(saveConfig).not.toHaveBeenCalled();
    fire(num, 'change');
    expect(num.value).toBe('60');
    expect(num.getAttribute('aria-invalid')).toBe('false');
    await vi.advanceTimersByTimeAsync(300);
    expect(saveConfig).toHaveBeenLastCalledWith({ ...BASE, interval_s: 60 });
  });

  it('closing inside the debounce still saves', async () => {
    const { panel, q, fire, saveConfig } = setup();
    panel.open();
    q<HTMLInputElement>('input[name=wheel][value=zoom]').checked = true;
    fire(q('input[name=wheel][value=zoom]'));
    panel.close();
    await vi.advanceTimersByTimeAsync(0);
    expect(saveConfig).toHaveBeenCalledWith({ ...BASE, wheel: 'zoom' });
  });

});

describe('settings panel: modal behaviour', () => {
  it('Esc closes through the panel layer and focus returns to the opener', () => {
    const { panel, ctx, opener, q } = setup();
    opener.focus();
    panel.open();
    expect(document.activeElement).not.toBe(opener);
    expect(q('.ps-backdrop').contains(document.activeElement)).toBe(true);
    expect(ctx.esc.handle()).toBe('panel');
    expect(panel.isOpen()).toBe(false);
    expect(document.activeElement).toBe(opener);
  });

  it('Tab wraps inside the panel in both directions', () => {
    const { panel, q } = setup();
    panel.open();
    const items = Array.from(q('[role=dialog]').querySelectorAll<HTMLElement>('input, button'));
    const first = items[0];
    const last = items[items.length - 1];
    last?.focus();
    const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    last?.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(first);
    const back = new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true });
    first?.dispatchEvent(back);
    expect(document.activeElement).toBe(last);
    expect(q('[role=dialog]').getAttribute('aria-modal')).toBe('true');
  });
});
