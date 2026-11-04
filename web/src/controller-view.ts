// Zoom, rotation, fullscreen and the Esc layers that the controller owns.
// (The panel and slideshow layers register themselves.)

import type { Action } from './actions';
import type { Ctx } from './ctx';
import type { Host } from './host';
import { FIT, clampPan, panBy, toggleZoom, zoomActual, zoomAt, type Geometry, type Zoom } from './zoom';

type ZoomAction = Extract<Action, { type: 'zoom_fit' | 'zoom_actual' | 'zoom_toggle' | 'zoom_by' | 'pan_by' }>;

function nextZoom(z: Zoom, g: Geometry, a: ZoomAction): Zoom {
  switch (a.type) {
    case 'zoom_actual': return zoomActual(z, g);
    case 'zoom_toggle': return toggleZoom(z, g);
    case 'zoom_by': return zoomAt(z, g, a.factor, a.anchor);
    case 'pan_by': return panBy(z, g, a.dx, a.dy);
    case 'zoom_fit': return FIT;
  }
}

/** Zoom actions need the image size; with nothing on screen they are ignored
 * (except fit, which is just a reset). */
export function applyZoom(ctx: Ctx, geometry: () => Geometry | null, a: ZoomAction): void {
  if (a.type === 'zoom_fit') {
    if (ctx.store.get().zoom.mode !== 'fit') ctx.store.set({ zoom: FIT });
    return;
  }
  const g = geometry();
  if (!g) return;
  ctx.store.set({ zoom: nextZoom(ctx.store.get().zoom, g, a) });
}

export function rotate(ctx: Ctx, geometry: () => Geometry | null, quarterTurns: 1 | -1): void {
  ctx.store.set({ rotation: (ctx.store.get().rotation + quarterTurns + 4) % 4 });
  // A quarter turn swaps width and height, so the old pan may now be out of range.
  const g = geometry();
  const z = ctx.store.get().zoom;
  if (g && z.mode !== 'fit') ctx.store.set({ zoom: clampPan(z, g) });
}

export const toggleFullscreen = (ctx: Ctx, host: Host): Promise<void> =>
  setFullscreen(ctx, host, !host.isFullscreen());

export async function setFullscreen(ctx: Ctx, host: Host, on: boolean): Promise<void> {
  try {
    await host.setFullscreen(on);
  } catch {
    // Refused (no user gesture, or the shell said no): the store keeps the truth below.
  }
  syncFullscreen(ctx, host);
}

export function syncFullscreen(ctx: Ctx, host: Host): void {
  const on = host.isFullscreen();
  if (ctx.store.get().fullscreen !== on) ctx.store.set({ fullscreen: on });
}

/** zoom -> fullscreen -> back to the browser; the order is fixed by EscStack. */
export function registerEscLayers(ctx: Ctx, host: Host): () => void {
  const offs = [
    ctx.esc.register('zoom', () => {
      if (ctx.store.get().zoom.mode === 'fit') return false;
      ctx.store.set({ zoom: FIT });
      return true;
    }),
    ctx.esc.register('fullscreen', () => {
      if (!host.isFullscreen()) return false;
      void toggleFullscreen(ctx, host);
      return true;
    }),
    // Esc at the bottom of the stack leaves the viewer for the browser. Closing
    // the window is the title bar's job (and Ctrl+Q), never a stray Esc.
    ctx.esc.register('window', () => {
      if (ctx.store.get().mode !== 'viewer') return false;
      ctx.store.set({ mode: 'browser' });
      return true;
    }),
  ];
  return () => offs.forEach((off) => off());
}
