// The draggable divider between tree and content. Pointer capture keeps the
// drag alive when the cursor leaves the thin handle; arrow keys resize too,
// so the separator is operable without a mouse.

import { TREE_MAX_FRACTION, TREE_MIN_PX } from './browser-consts';

export const treeMax = (winW: number): number => Math.max(TREE_MIN_PX, Math.floor(winW * TREE_MAX_FRACTION));

/** Width limited to 160 px .. half of the window. */
export const clampTree = (px: number, winW: number): number =>
  Math.min(treeMax(winW), Math.max(TREE_MIN_PX, Math.round(px)));

const KEY_STEP_PX = 16;

/** `current` reads the live width; `set` receives each new clamped width;
 * `winW` is read at every move, so a resized window moves the limit. */
export function mountSplitter(
  el: HTMLElement, current: () => number, set: (px: number) => void, winW: () => number,
): () => void {
  el.setAttribute('role', 'separator');
  el.setAttribute('aria-orientation', 'vertical');
  el.tabIndex = 0;
  let start: { x: number; w: number } | null = null;

  const down = (e: PointerEvent): void => {
    start = { x: e.clientX, w: current() };
    el.setPointerCapture?.(e.pointerId);
    el.classList.add('drag');
    e.preventDefault();
  };
  const move = (e: PointerEvent): void => {
    if (start) set(clampTree(start.w + e.clientX - start.x, winW()));
  };
  const up = (): void => {
    start = null;
    el.classList.remove('drag');
  };
  const key = (e: KeyboardEvent): void => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    e.stopPropagation();
    set(clampTree(current() + (e.key === 'ArrowRight' ? KEY_STEP_PX : -KEY_STEP_PX), winW()));
  };

  el.addEventListener('pointerdown', down);
  el.addEventListener('pointermove', move);
  el.addEventListener('pointerup', up);
  el.addEventListener('pointercancel', up);
  el.addEventListener('keydown', key);
  return () => {
    el.removeEventListener('pointerdown', down);
    el.removeEventListener('pointermove', move);
    el.removeEventListener('pointerup', up);
    el.removeEventListener('pointercancel', up);
    el.removeEventListener('keydown', key);
  };
}
