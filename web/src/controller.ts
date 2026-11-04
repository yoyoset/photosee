// Action -> state transitions. The pieces live in controller-nav.ts (page
// turns, folders) and controller-view.ts (zoom, rotation, fullscreen, Esc);
// this file is only the routing table and the handles it needs.

import type { Action } from './actions';
import { createNavigator, type NavParts, type Navigator } from './controller-nav';
import { applyZoom, registerEscLayers, rotate, toggleFullscreen } from './controller-view';
import type { Ctx } from './ctx';
import type { Host } from './host';
import type { PanelHandle } from './help';
import type { Viewer } from './viewer';

/** Everything the controller drives that is mounted after the ctx exists. */
export interface Parts extends NavParts {
  viewer: Pick<Viewer, 'geometry'>;
  info: PanelHandle;
  settings: PanelHandle;
  help: PanelHandle;
  host: Host;
}

export interface Controller {
  dispatch: (a: Action) => void;
  nav: Navigator;
  /** Registers the zoom / fullscreen / window Esc layers. Call once after mount. */
  registerEsc: () => () => void;
}

export function createController(getCtx: () => Ctx, getParts: () => Parts): Controller {
  const nav = createNavigator(getCtx, getParts);

  const dispatch = (a: Action): void => {
    const ctx = getCtx();
    const parts = getParts();
    switch (a.type) {
      case 'next': case 'prev': case 'first': case 'last': return nav.turn(a.type);
      case 'goto': return nav.turn(a.index);
      case 'next_folder': return void nav.changeFolder('next');
      case 'prev_folder': return void nav.changeFolder('prev');
      case 'toggle_fullscreen': return void toggleFullscreen(ctx, parts.host);
      case 'toggle_slideshow': return parts.slideshow.toggle();
      case 'rotate': return rotate(ctx, () => parts.viewer.geometry(), a.quarterTurns);
      case 'zoom_fit': case 'zoom_actual': case 'zoom_toggle': case 'zoom_by': case 'pan_by':
        return applyZoom(ctx, () => parts.viewer.geometry(), a);
      case 'toggle_info': return parts.info.toggle();
      case 'toggle_help': return parts.help.toggle();
      case 'open_settings': return parts.settings.open();
      case 'scrub': return ctx.store.set({ scrubbing: a.active });
      case 'escape': ctx.esc.handle();
    }
  };

  return { dispatch, nav, registerEsc: () => registerEscLayers(getCtx(), getParts().host) };
}

export function createDispatch(getCtx: () => Ctx, getParts: () => Parts): (a: Action) => void {
  return createController(getCtx, getParts).dispatch;
}
