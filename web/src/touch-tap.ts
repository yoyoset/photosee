// A finger has no double-click. On a touch screen a tap on a file or folder
// opens it, as in every phone file manager; a mouse click still only selects
// and a double-click opens. Long-press still brings up the item menu
// (browser-menu.ts listens for `contextmenu`).
//
// The pointer type is taken from the pointerdown before the click, because
// a click is not a PointerEvent in every browser (older Safari).

import { DOUBLE_TAP_MS } from './constants';

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
