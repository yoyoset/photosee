// The two buttons of the overlay's top bar. Phones have no keyboard, so
// everything a key does in the viewer needs something to tap (2026-11-06):
// back (Esc) top-left, where phones put it, and photo info (I) top-right.

import type { Ctx } from './ctx';
import { t } from './i18n';
import { ICONS } from './overlay-icons';

function topButton(cls: string, icon: string, label: string, key: string, run: () => void): HTMLButtonElement {
  const b = document.createElement('button');
  b.className = `ov-btn ${cls}`;
  b.type = 'button';
  b.innerHTML = icon; // constant markup, never data from a file
  b.setAttribute('aria-label', label);
  b.title = `${label} (${key})`;
  b.addEventListener('click', run);
  return b;
}

export function topButtons(ctx: Ctx): [HTMLButtonElement, HTMLButtonElement] {
  return [
    topButton('ov-back', ICONS.back, t('btn.back'), 'Esc', () => ctx.dispatch({ type: 'close_viewer' })),
    topButton('ov-info', ICONS.info, t('help.info'), 'I', () => ctx.dispatch({ type: 'toggle_info' })),
  ];
}
