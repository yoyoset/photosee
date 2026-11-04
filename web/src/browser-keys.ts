// Keyboard map of the content area (grid and details). The tree handles its
// own keys; the viewer's map (keys.ts) is not bound in browser mode.

import type { MoveKey } from './grid-geometry';
import type { KeyInput } from './keys';

export type BrowserCmd =
  | { t: 'move'; key: MoveKey }
  | { t: 'open' }
  | { t: 'up' }
  | { t: 'back' }
  | { t: 'forward' }
  | { t: 'address' }
  | { t: 'type'; ch: string };

const MOVES: Record<string, MoveKey> = {
  ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down',
  Home: 'home', End: 'end', PageUp: 'pageup', PageDown: 'pagedown',
};

export function browserKey(e: KeyInput): BrowserCmd | null {
  const mod = e.ctrlKey || e.metaKey;
  if (e.altKey && !mod) {
    if (e.key === 'ArrowLeft') return { t: 'back' };
    if (e.key === 'ArrowRight') return { t: 'forward' };
    if (e.key === 'ArrowUp') return { t: 'up' };
    return e.key.toLowerCase() === 'd' ? { t: 'address' } : null;
  }
  if (mod) return e.key.toLowerCase() === 'l' && !e.altKey ? { t: 'address' } : null;
  const move = MOVES[e.key];
  if (move) return { t: 'move', key: move };
  if (e.key === 'Enter') return { t: 'open' };
  if (e.key === 'Backspace') return { t: 'up' };
  // Shift only produces capitals here; anything longer than one char is a named key.
  if (e.key.length === 1) return { t: 'type', ch: e.key };
  return null;
}

export interface KeyTargets {
  active(): boolean;
  up(): void;
  back(): void;
  forward(): void;
  address(): void;
  /** Selection keys and type-ahead; true when the content used the key. */
  content(cmd: BrowserCmd, now: number): boolean;
  now(): number;
}

const INPUT = /^(input|select|textarea)$/i;

/** Window-level so keys work when nothing is focused. The tree, the address
 * field, the splitter and the toolbar's own controls keep the keys they need. */
export function bindBrowserKeys(win: Window, tg: KeyTargets): () => void {
  const onKey = (e: KeyboardEvent): void => {
    if (!tg.active() || e.isComposing || e.defaultPrevented) return;
    const el = e.target instanceof Element ? e.target : null;
    if (el && (INPUT.test(el.tagName) || el.closest('.br-tree, .br-split'))) return;
    if ((e.key === 'Enter' || e.key === ' ') && el?.closest('button')) return;
    const cmd = browserKey(e);
    if (!cmd) return;
    let handled = true;
    switch (cmd.t) {
      case 'up': tg.up(); break;
      case 'back': tg.back(); break;
      case 'forward': tg.forward(); break;
      case 'address': tg.address(); break;
      default: handled = tg.content(cmd, tg.now());
    }
    if (handled) e.preventDefault();
  };
  win.addEventListener('keydown', onKey);
  return () => win.removeEventListener('keydown', onKey);
}
