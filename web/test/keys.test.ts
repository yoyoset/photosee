import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Action } from '../src/actions';
import { KEY_REPEAT_SETTLE_MS, ZOOM_STEP } from '../src/constants';
import { bindKeys, keyAction, type KeyInput } from '../src/keys';

const k = (key: string, m: Partial<KeyInput> = {}): KeyInput => (
  { key, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...m });
const ctrl = { ctrlKey: true };
const cmd = { metaKey: true };
const shift = { shiftKey: true };
const T = (type: Action['type']): Action => ({ type }) as Action;
const scrubs = (xs: Action[]): Action[] => xs.filter((a) => a.type === 'scrub');

describe('keyAction: full table', () => {
  const same: [string, Partial<KeyInput>, Action][] = [
    ['ArrowRight', {}, T('next')], ['ArrowLeft', {}, T('prev')],
    [' ', {}, T('next')], ['Backspace', {}, T('prev')],
    ['PageDown', {}, T('next')], ['PageUp', {}, T('prev')],
    ['Home', {}, T('first')], ['End', {}, T('last')],
    [']', {}, T('next_folder')], ['[', {}, T('prev_folder')],
    ['f', {}, T('toggle_fullscreen')], ['F', {}, T('toggle_fullscreen')],
    ['Enter', {}, T('toggle_fullscreen')],
    ['s', {}, T('toggle_slideshow')], ['i', {}, T('toggle_info')],
    ['r', {}, { type: 'rotate', quarterTurns: 1 }],
    ['R', shift, { type: 'rotate', quarterTurns: -1 }],
    ['r', shift, { type: 'rotate', quarterTurns: -1 }],
    ['0', {}, T('zoom_fit')], ['1', {}, T('zoom_actual')], ['w', {}, T('fit_cycle')],
    ['+', shift, { type: 'zoom_by', factor: ZOOM_STEP }],
    ['=', {}, { type: 'zoom_by', factor: ZOOM_STEP }],
    ['+', {}, { type: 'zoom_by', factor: ZOOM_STEP }],
    ['-', {}, { type: 'zoom_by', factor: 1 / ZOOM_STEP }],
    ['?', shift, T('toggle_help')], ['?', {}, T('toggle_help')],
    ['Escape', {}, T('escape')],
  ];
  for (const platform of ['windows', 'linux', 'macos', 'android', 'ios'] as const) {
    it(`shared keys on ${platform}`, () => {
      for (const [key, mods, want] of same) expect(keyAction(k(key, mods), platform), key).toEqual(want);
    });
  }

  it('folder navigation: Ctrl on windows/linux, Cmd on macOS', () => {
    for (const p of ['windows', 'linux'] as const) {
      expect(keyAction(k('ArrowRight', ctrl), p)).toEqual(T('next_folder'));
      expect(keyAction(k('ArrowLeft', ctrl), p)).toEqual(T('prev_folder'));
      expect(keyAction(k('ArrowRight', cmd), p)).toBeNull();
    }
    expect(keyAction(k('ArrowRight', cmd), 'macos')).toEqual(T('next_folder'));
    expect(keyAction(k('ArrowLeft', cmd), 'macos')).toEqual(T('prev_folder'));
    // Ctrl+arrows belong to Spaces on macOS.
    expect(keyAction(k('ArrowRight', ctrl), 'macos')).toBeNull();
  });

  it('fullscreen: F11 off macOS, Ctrl+Cmd+F on macOS only', () => {
    expect(keyAction(k('F11'), 'windows')).toEqual(T('toggle_fullscreen'));
    expect(keyAction(k('F11'), 'linux')).toEqual(T('toggle_fullscreen'));
    expect(keyAction(k('F11'), 'macos')).toBeNull();
    expect(keyAction(k('f', { ctrlKey: true, metaKey: true }), 'macos')).toEqual(T('toggle_fullscreen'));
    expect(keyAction(k('f', ctrl), 'windows')).toBeNull();
    expect(keyAction(k('f', cmd), 'macos')).toBeNull();
    expect(keyAction(k('f', ctrl), 'macos')).toBeNull();
  });
});

describe('keyAction: everything else stays with the browser', () => {
  it('unlisted modifiers return null', () => {
    expect(keyAction(k('r', ctrl), 'windows')).toBeNull(); // reload, not rotate
    expect(keyAction(k('r', cmd), 'macos')).toBeNull();
    expect(keyAction(k('ArrowLeft', { altKey: true }), 'windows')).toBeNull(); // browser back
    expect(keyAction(k('ArrowRight', { altKey: true }), 'macos')).toBeNull();
    expect(keyAction(k('ArrowRight', shift), 'windows')).toBeNull();
    expect(keyAction(k('ArrowRight', { ctrlKey: true, shiftKey: true }), 'windows')).toBeNull();
    expect(keyAction(k('ArrowRight', { metaKey: true }), 'windows')).toBeNull(); // Win key
    expect(keyAction(k('s', ctrl), 'windows')).toBeNull(); // save page
    expect(keyAction(k('0', ctrl), 'windows')).toBeNull(); // browser zoom reset
    expect(keyAction(k('+', ctrl), 'windows')).toBeNull();
    expect(keyAction(k('Enter', { altKey: true }), 'windows')).toBeNull();
    expect(keyAction(k('i', shift), 'windows')).toBeNull();
  });

  it('unmapped keys return null', () => {
    for (const key of ['a', 'Tab', 'F5', 'Shift', 'Control', 'ArrowUp', 'ArrowDown', 'Dead']) {
      expect(keyAction(k(key), 'windows'), key).toBeNull();
    }
  });
});

let seen: Action[];
let off: () => void;
const press = (key: string, o: KeyboardEventInit = {}, target: EventTarget = window): KeyboardEvent => {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...o });
  target.dispatchEvent(e);
  return e;
};
const release = (key: string): void => {
  window.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
};
beforeEach(() => {
  vi.useFakeTimers();
  seen = [];
  off = bindKeys(window, 'windows', (a) => seen.push(a));
});
afterEach(() => { off(); vi.useRealTimers(); document.body.innerHTML = ''; });


describe('bindKeys', () => {
  it('prevents default only for handled keys', () => {
    expect(press('ArrowRight').defaultPrevented).toBe(true);
    expect(press('a').defaultPrevented).toBe(false);
    expect(press('r', { ctrlKey: true }).defaultPrevented).toBe(false);
    expect(seen).toEqual([T('next')]);
  });

  it('ignores typing in input, select, textarea and contenteditable', () => {
    document.body.innerHTML =
      '<input id="i"><select id="s"></select><textarea id="t"></textarea>' +
      '<div contenteditable="true"><span id="c"></span></div>' +
      '<div contenteditable="true"><b contenteditable="false" id="n"></b></div>';
    for (const id of ['i', 's', 't', 'c']) {
      const e = press('ArrowRight', {}, document.getElementById(id) as HTMLElement);
      expect(e.defaultPrevented, id).toBe(false);
    }
    expect(seen).toEqual([]);
    // An opted-out subtree is not editable.
    press('ArrowRight', {}, document.getElementById('n') as HTMLElement);
    expect(seen).toEqual([T('next')]);
  });

  it('lets Space and Enter activate a focused button instead', () => {
    document.body.innerHTML = '<button id="b"></button>';
    const b = document.getElementById('b') as HTMLElement;
    expect(press(' ', {}, b).defaultPrevented).toBe(false);
    expect(press('Enter', {}, b).defaultPrevented).toBe(false);
    expect(press('ArrowRight', {}, b).defaultPrevented).toBe(true);
    expect(seen).toEqual([T('next')]);
  });

});

describe('bindKeys: held arrows scrub', () => {
  it('held arrow: scrub on once, pages keep turning, off on keyup', () => {
    press('ArrowRight');
    vi.advanceTimersByTime(400);
    expect(seen).toEqual([T('next')]); // the first press is a plain page turn
    for (let i = 0; i < 5; i++) { press('ArrowRight', { repeat: true }); vi.advanceTimersByTime(30); }
    expect(seen).toEqual([
      T('next'),
      { type: 'scrub', active: true }, T('next'),
      T('next'), T('next'), T('next'), T('next'),
    ]);
    seen.length = 0;
    release('ArrowRight');
    expect(seen).toEqual([{ type: 'scrub', active: false }]);
    vi.advanceTimersByTime(1000);
    expect(seen).toHaveLength(1); // no second scrub-off from the timer
  });

  it('held arrow: scrub off after KEY_REPEAT_SETTLE_MS without repeats', () => {
    press('ArrowLeft', { repeat: true });
    vi.advanceTimersByTime(KEY_REPEAT_SETTLE_MS - 1);
    expect(scrubs(seen)).toEqual([{ type: 'scrub', active: true }]);
    vi.advanceTimersByTime(1);
    expect(scrubs(seen)).toEqual([{ type: 'scrub', active: true }, { type: 'scrub', active: false }]);
    // A repeat inside the window re-arms the timer instead of stacking one.
    seen.length = 0;
    press('ArrowLeft', { repeat: true });
    vi.advanceTimersByTime(100);
    press('ArrowLeft', { repeat: true });
    vi.advanceTimersByTime(100);
    expect(scrubs(seen)).toEqual([{ type: 'scrub', active: true }]);
  });

  it('repeat of non-arrow keys and of Ctrl+arrow does not scrub', () => {
    press('PageDown', { repeat: true });
    press('ArrowRight', { repeat: true, ctrlKey: true });
    expect(scrubs(seen)).toEqual([]);
  });

  it('unbind ends an active scrub and removes listeners', () => {
    press('ArrowRight', { repeat: true });
    seen.length = 0;
    off();
    expect(seen).toEqual([{ type: 'scrub', active: false }]);
    press('ArrowRight');
    expect(seen).toHaveLength(1);
  });
});
