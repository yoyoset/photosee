// Single runtime state store and the Esc layer stack. Contract file:
// orchestrator only. Modules read `store.get()`, change state only through
// `store.set()`, and react through `store.subscribe()` — no module keeps its
// own copy of shared state.

import type { ImageItem, Listing, Settings } from './api';

export type ZoomMode = 'fit' | 'actual' | 'custom';
/** What zoom mode 'fit' fills: the whole image, the width or the height.
 * A reading preference: page turns keep it (they reset zoom to fit). */
export type FitMode = 'contain' | 'width' | 'height';
export const FIT_MODES: readonly FitMode[] = ['contain', 'width', 'height'];

export interface ViewState {
  /** Which of the two screens is up. Added by package BR: the browser (folder
   * tree + thumbnails) is the default; the viewer is the black stage. */
  mode: 'browser' | 'viewer';
  /** Folder the user opened; "next folder" never leaves it. */
  root: string | null;
  listing: Listing | null;
  index: number;
  zoom: { mode: ZoomMode; scale: number; x: number; y: number };
  fit: FitMode;
  /** Display-only rotation in quarter turns, 0..3. */
  rotation: number;
  fullscreen: boolean;
  slideshow: 'off' | 'playing' | 'paused';
  overlayVisible: boolean;
  settings: Settings | null;
  /** True while an arrow key is held: show thumbnails, defer full decodes. */
  scrubbing: boolean;
}

export const initialState: ViewState = {
  mode: 'viewer',
  root: null,
  listing: null,
  index: 0,
  zoom: { mode: 'fit', scale: 1, x: 0, y: 0 },
  fit: 'contain',
  rotation: 0,
  fullscreen: false,
  slideshow: 'off',
  overlayVisible: true,
  settings: null,
  scrubbing: false,
};

type Listener = (s: ViewState, prev: ViewState) => void;

export class Store {
  private state: ViewState;
  private readonly listeners = new Set<Listener>();

  constructor(init: ViewState = initialState) {
    this.state = init;
  }

  get(): ViewState {
    return this.state;
  }

  set(patch: Partial<ViewState>): void {
    const prev = this.state;
    this.state = { ...prev, ...patch };
    for (const l of this.listeners) l(this.state, prev);
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  current(): ImageItem | null {
    const { listing, index } = this.state;
    return listing?.images[index] ?? null;
  }
}

/**
 * Esc (and Android Back) unwinds one layer at a time, topmost first:
 * panel/help -> slideshow -> zoom -> fullscreen -> close window.
 * A layer's `close` returns true when it consumed the key.
 */
export type EscLayer = 'panel' | 'slideshow' | 'zoom' | 'fullscreen' | 'window';

const ESC_ORDER: readonly EscLayer[] = ['panel', 'slideshow', 'zoom', 'fullscreen', 'window'];

export class EscStack {
  private readonly handlers = new Map<EscLayer, () => boolean>();

  register(layer: EscLayer, close: () => boolean): () => void {
    this.handlers.set(layer, close);
    return () => this.handlers.delete(layer);
  }

  /** Returns the layer that consumed the key, or null. */
  handle(): EscLayer | null {
    for (const layer of ESC_ORDER) {
      const close = this.handlers.get(layer);
      if (close?.()) return layer;
    }
    return null;
  }
}
