import { beforeEach, describe, expect, it } from 'vitest';
import type { Session } from '../src/api';
import type { Ctx } from '../src/ctx';
import { helpRows, mountHelp, touchRows } from '../src/help';
import { setLang } from '../src/i18n';
import { EscStack, Store } from '../src/state';

function ctxFor(platform: Session['platform']): Ctx {
  return {
    store: new Store(), esc: new EscStack(), session: { platform } as Session,
    api: {}, src: {}, dispatch: () => {},
  } as unknown as Ctx;
}

beforeEach(() => {
  setLang('en');
  document.body.replaceChildren();
});

describe('help: touch', () => {
  it('lists every gesture in words, none of them a key', () => {
    const rows = touchRows();
    expect(rows.length).toBeGreaterThanOrEqual(7);
    for (const r of rows) {
      expect(r.label).not.toBe('');
      expect(r.keys.join(' ')).not.toMatch(/Ctrl|⌘|Esc/);
    }
    expect(rows.map((r) => r.keys).flat()).toContain('Tap the right side');
  });

  it('the panel shows both tables, touch first on a touch screen', () => {
    const mq = window.matchMedia;
    window.matchMedia = ((q: string) => ({ matches: q === '(pointer: coarse)' })) as unknown as typeof window.matchMedia;
    try {
      mountHelp(document.body, ctxFor('linux')).open();
      const heads = [...document.querySelectorAll('.ps-help-sub')].map((h) => h.textContent);
      expect(heads).toEqual(['Touch', 'Keyboard']);
    } finally {
      window.matchMedia = mq;
    }
  });
});

describe('help', () => {
  it('macOS and Windows tables differ in the modifier', () => {
    const win = helpRows('windows').flatMap((r) => r.keys);
    const mac = helpRows('macos').flatMap((r) => r.keys);
    expect(win).toContain('Ctrl+→');
    expect(win).toContain('F11');
    expect(mac).toContain('⌘→');
    expect(mac).not.toContain('Ctrl+→');
    expect(mac).not.toContain('F11');
    expect(helpRows('linux')).toEqual(helpRows('windows'));
    expect(helpRows('macos').length).toBe(helpRows('windows').length);
  });

  it('every row has a label', () => {
    for (const p of ['windows', 'macos'] as const) {
      for (const r of helpRows(p)) expect(r.label, r.keys.join()).not.toBe('');
    }
  });

  it('renders the table, opens and closes through the Esc layer', () => {
    const ctx = ctxFor('macos');
    const help = mountHelp(document.body, ctx);
    expect(document.querySelector('.ps-backdrop')?.hasAttribute('hidden')).toBe(true);
    help.open();
    expect(help.isOpen()).toBe(true);
    expect(document.querySelectorAll('kbd').length).toBeGreaterThan(10);
    expect(document.body.textContent).toContain('⌘←');
    expect(ctx.esc.handle()).toBe('panel');
    expect(help.isOpen()).toBe(false);
    expect(ctx.esc.handle()).toBeNull(); // handler is gone while closed
  });

  it('toggle works and a second panel closes the first (one Esc handler per layer)', () => {
    const ctx = ctxFor('windows');
    const a = mountHelp(document.body, ctx);
    const b = mountHelp(document.body, ctx);
    a.toggle();
    expect(a.isOpen()).toBe(true);
    b.open();
    expect(a.isOpen()).toBe(false);
    expect(ctx.esc.handle()).toBe('panel');
    expect(b.isOpen()).toBe(false);
    a.toggle();
    a.toggle();
    expect(a.isOpen()).toBe(false);
  });
});
