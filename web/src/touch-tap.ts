// A finger has no double-click. On a touch screen a tap on a file or folder
// opens it, as in every phone file manager; a mouse click still only selects
// and a double-click opens. Long-press still brings up the item menu
// (browser-menu.ts listens for `contextmenu`).
//
// The pointer type is taken from the pointerdown before the click, because
// a click is not a PointerEvent in every browser (older Safari).

import { DOUBLE_TAP_MS, LONG_PRESS_MS } from './constants';

/** Finger wobble that still counts as holding still. */
const HOLD_SLOP_PX = 10;
/** After a long-press fired: swallow the lift's click and Android's own contextmenu. */
const AFTER_HOLD_MS = 1000;

export function bindTapOpen(
  el: HTMLElement, select: (e: MouseEvent) => void, open: (e: MouseEvent) => void,
): void {
  let touch = false;
  let lastOpen = -Infinity;
  el.addEventListener('pointerdown', (e) => { touch = e.pointerType === 'touch'; }, true);
  el.addEventListener('click', (e) => {
    if (!touch) return select(e);
    // A double tap out of habit must not open the folder and then, with the
    // second tap, whatever lands under the finger in the folder it opened.
    if (e.timeStamp - lastOpen < DOUBLE_TAP_MS) return;
    lastOpen = e.timeStamp;
    open(e);
  });
  el.addEventListener('dblclick', (e) => { if (!touch) open(e); });
}

/** iOS Safari sends no `contextmenu` for a long-press (found in the phone
 * review, 2026-11-06), so a held finger opens the item menu here. The click
 * of the lift is swallowed, and so is Android's own contextmenu right after
 * ours. A finger that moves (a scroll) or lifts early cancels. */
export function bindLongPress(el: HTMLElement, fire: (e: PointerEvent) => void): void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let start: { x: number; y: number } | null = null;
  let firedAt = -Infinity;
  const cancel = (): void => { clearTimeout(timer); start = null; };
  el.addEventListener('pointerdown', (e) => {
    if (e.pointerType !== 'touch') return;
    start = { x: e.clientX, y: e.clientY };
    clearTimeout(timer);
    timer = setTimeout(() => { start = null; firedAt = performance.now(); fire(e); }, LONG_PRESS_MS);
  });
  el.addEventListener('pointermove', (e) => {
    if (start && Math.hypot(e.clientX - start.x, e.clientY - start.y) > HOLD_SLOP_PX) cancel();
  });
  for (const type of ['pointerup', 'pointercancel']) el.addEventListener(type, cancel);
  const swallow = (e: Event): void => {
    if (performance.now() - firedAt > AFTER_HOLD_MS) return;
    e.preventDefault();
    e.stopImmediatePropagation();
  };
  el.addEventListener('click', swallow, true);
  el.addEventListener('contextmenu', swallow, true);
}
