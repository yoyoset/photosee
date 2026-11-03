// Keyboard map (docs/ux-spec.md §Keys). Package K.

import type { Action } from './actions';
import type { Session } from './api';
import { KEY_REPEAT_SETTLE_MS, ZOOM_STEP } from './constants';

export interface KeyInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

type Platform = Session['platform'];

const PAGE: Record<string, Action> = {
  ArrowRight: { type: 'next' }, ArrowLeft: { type: 'prev' },
  ' ': { type: 'next' }, Backspace: { type: 'prev' },
  PageDown: { type: 'next' }, PageUp: { type: 'prev' },
  Home: { type: 'first' }, End: { type: 'last' },
};

const PLAIN: Record<string, Action> = {
  ']': { type: 'next_folder' }, '[': { type: 'prev_folder' },
  Enter: { type: 'toggle_fullscreen' }, f: { type: 'toggle_fullscreen' },
  s: { type: 'toggle_slideshow' }, i: { type: 'toggle_info' },
  '0': { type: 'zoom_fit' }, '1': { type: 'zoom_actual' },
  '-': { type: 'zoom_by', factor: 1 / ZOOM_STEP },
  Escape: { type: 'escape' },
};

// Shift is part of how these characters are typed ('+' is Shift+'=' on most
// layouts, '?' is Shift+'/'), so it must not disqualify them. We look at
// e.key, never e.code, so non-US layouts keep working.
const SHIFT_TYPED: Record<string, Action> = {
  '+': { type: 'zoom_by', factor: ZOOM_STEP },
  '=': { type: 'zoom_by', factor: ZOOM_STEP },
  '?': { type: 'toggle_help' },
};

function folderKey(key: string): Action | null {
  if (key === 'ArrowRight') return { type: 'next_folder' };
  if (key === 'ArrowLeft') return { type: 'prev_folder' };
  return null;
}

/**
 * Pure mapping; macOS uses Cmd where others use Ctrl. Anything not in the
 * table (including every unlisted modifier combination) returns null so the
 * browser/webview keeps its own shortcut: Ctrl+R reload, Alt+Left back, ...
 */
export function keyAction(e: KeyInput, platform: Platform): Action | null {
  const mac = platform === 'macos';
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (e.altKey) return null;

  // macOS fullscreen is Ctrl+Cmd+F; Ctrl alone or Cmd alone must not fire.
  if (mac && e.ctrlKey && e.metaKey) {
    return key === 'f' && !e.shiftKey ? { type: 'toggle_fullscreen' } : null;
  }
  const mod = mac ? e.metaKey : e.ctrlKey;
  const stray = mac ? e.ctrlKey : e.metaKey; // Win/Super key, or Ctrl on mac
  if (stray) return null;
  if (mod) return e.shiftKey ? null : folderKey(key);

  if (e.shiftKey) {
    if (key === 'r') return { type: 'rotate', quarterTurns: -1 };
    return SHIFT_TYPED[key] ?? null;
  }
  // F11 is reserved by macOS (show desktop), so it only maps elsewhere.
  if (key === 'F11') return mac ? null : { type: 'toggle_fullscreen' };
  if (key === 'r') return { type: 'rotate', quarterTurns: 1 };
  return PAGE[key] ?? PLAIN[key] ?? SHIFT_TYPED[key] ?? null;
}

/** Elements where typing must reach the page's own handling. */
function isTyping(t: EventTarget | null): boolean {
  if (!(t instanceof Element)) return false;
  if (/^(input|select|textarea)$/i.test(t.tagName)) return true;
  // jsdom has no isContentEditable, and "false" inside an editable ancestor
  // opts a subtree out again, so resolve it from the attribute.
  const host = t.closest('[contenteditable]');
  return host !== null && host.getAttribute('contenteditable') !== 'false';
}

/** Space/Enter on a focused control activate that control; do not also act. */
function ownsActivation(t: EventTarget | null, key: string): boolean {
  if (key !== ' ' && key !== 'Enter') return false;
  return t instanceof Element && t.closest('button, a[href], summary') !== null;
}

const isArrowTurn = (e: KeyboardEvent): boolean => e.key === 'ArrowLeft' || e.key === 'ArrowRight';

export function bindKeys(
  target: Window, platform: Platform, dispatch: (a: Action) => void,
): () => void {
  let scrubbing = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const setScrub = (active: boolean): void => {
    clearTimeout(timer);
    timer = undefined;
    if (scrubbing === active) return;
    scrubbing = active;
    dispatch({ type: 'scrub', active });
  };

  const onDown = (e: KeyboardEvent): void => {
    if (e.isComposing || isTyping(e.target) || ownsActivation(e.target, e.key)) return;
    const action = keyAction(e, platform);
    if (!action) return;
    e.preventDefault();
    // Only plain arrows scrub; Ctrl+arrow (folder) must never look like a hold.
    if (e.repeat && isArrowTurn(e) && (action.type === 'next' || action.type === 'prev')) {
      setScrub(true);
      // Re-arm on every repeat: "keys went quiet" is what ends the scrub.
      timer = setTimeout(() => setScrub(false), KEY_REPEAT_SETTLE_MS);
    }
    dispatch(action);
  };

  const onUp = (e: KeyboardEvent): void => {
    if (isArrowTurn(e)) setScrub(false);
  };
  const onBlur = (): void => setScrub(false);

  target.addEventListener('keydown', onDown);
  target.addEventListener('keyup', onUp);
  target.addEventListener('blur', onBlur);
  return () => {
    setScrub(false);
    target.removeEventListener('keydown', onDown);
    target.removeEventListener('keyup', onUp);
    target.removeEventListener('blur', onBlur);
  };
}
