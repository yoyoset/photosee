// Slideshow state machine, pure (time and randomness are injected).
// Package B. Rules: docs/ux-spec.md §slideshow.
//
// The timer, DOM and /nav calls live in slideshow-ui.ts; it feeds events in
// and performs the returned effect. Anything that does not apply to the
// current phase is a no-op, so a late timer callback or a duplicate
// `image_failed` can never corrupt the state.

import type { Settings } from './api';

export type SlideEvent =
  | { type: 'start'; index: number; count: number }
  | { type: 'tick' }
  | { type: 'pause' } | { type: 'resume' } | { type: 'stop' }
  | { type: 'image_failed' }
  | { type: 'folder_loaded'; count: number }
  /** The server found no further folder with images (answer to load_next_folder). */
  | { type: 'no_next_folder' };

export type SlideEffect =
  | { type: 'show'; index: number }
  | { type: 'load_next_folder' }
  | { type: 'stopped'; reason: 'user' | 'end' }
  | { type: 'none' };

export interface SlideState {
  phase: 'off' | 'playing' | 'paused';
  /** Position in `order`, not an image index. */
  pos: number;
  /** Image indices in play order (identity unless shuffled). */
  order: number[];
}

export const idle: SlideState = { phase: 'off', pos: 0, order: [] };

type Cfg = Pick<Settings, 'on_end' | 'shuffle'>;
type Result = [SlideState, SlideEffect];

const NONE: SlideEffect = { type: 'none' };

/** Counts come from the UI/server; treat anything non-finite or < 1 as empty. */
function safeCount(n: number): number {
  return Number.isFinite(n) && n >= 1 ? Math.floor(n) : 0;
}

/** Fisher–Yates in place. `rand()` is clamped so a buggy 1.0 cannot overrun. */
function shuffleInPlace(a: number[], rand: () => number): void {
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.min(i, Math.max(0, Math.floor(rand() * (i + 1))));
    const tmp = a[i] as number; // i, j are in range by construction
    a[i] = a[j] as number;
    a[j] = tmp;
  }
}

/** Play order for a folder of `count` images; `first` is pinned to the front. */
function makeOrder(count: number, first: number | null, cfg: Cfg, rand: () => number): number[] {
  const all = Array.from({ length: count }, (_, i) => i);
  if (!cfg.shuffle) return all;
  if (first === null) {
    shuffleInPlace(all, rand);
    return all;
  }
  const rest = all.filter((i) => i !== first);
  shuffleInPlace(rest, rand);
  return [first, ...rest];
}

function begin(ev: { index: number; count: number }, cfg: Cfg, rand: () => number): Result {
  const count = safeCount(ev.count);
  if (count === 0) return [idle, { type: 'stopped', reason: 'end' }];
  const idx = Number.isFinite(ev.index) ? Math.min(count - 1, Math.max(0, Math.floor(ev.index))) : 0;
  // The current image is already on screen, so no `show`: the UI just starts its timer.
  return [{ phase: 'playing', pos: 0, order: makeOrder(count, idx, cfg, rand) }, NONE];
}

/** order[pos]; callers guarantee a non-empty order and an in-range pos. */
function at(s: SlideState, pos: number): number {
  return s.order[pos] ?? 0;
}

/** Advance after a tick or a failed image (a failed image counts as shown). */
function advance(s: SlideState, cfg: Cfg): Result {
  if (s.pos + 1 < s.order.length) {
    const pos = s.pos + 1;
    return [{ ...s, pos }, { type: 'show', index: at(s, pos) }];
  }
  switch (cfg.on_end) {
    case 'loop': // keep the same order so a shuffle is drawn once per start
      return [{ ...s, pos: 0 }, { type: 'show', index: at(s, 0) }];
    case 'next_folder':
      // Stay playing; the UI answers with folder_loaded or no_next_folder.
      return [s, { type: 'load_next_folder' }];
    case 'stop':
      return [idle, { type: 'stopped', reason: 'end' }];
  }
}

function folderLoaded(s: SlideState, count: number, cfg: Cfg, rand: () => number): Result {
  const n = safeCount(count);
  // The server skips empty folders; if one slips through, ask again.
  if (n === 0) return [s, { type: 'load_next_folder' }];
  const order = makeOrder(n, null, cfg, rand);
  return [{ phase: 'playing', pos: 0, order }, { type: 'show', index: order[0] ?? 0 }];
}

export function step(s: SlideState, ev: SlideEvent, cfg: Cfg, rand: () => number): Result {
  if (s.phase === 'off') return ev.type === 'start' ? begin(ev, cfg, rand) : [s, NONE];
  if (ev.type === 'stop') return [idle, { type: 'stopped', reason: 'user' }];
  if (s.phase === 'paused') {
    // Only resume applies. A folder_loaded / no_next_folder arriving while paused is
    // dropped on purpose: resume then re-runs the end rule and asks again, which
    // self-heals without needing a "waiting for folder" flag in the state.
    return ev.type === 'resume' ? [{ ...s, phase: 'playing' }, NONE] : [s, NONE];
  }
  switch (ev.type) {
    case 'tick':
    case 'image_failed':
      return s.order.length === 0 ? [s, NONE] : advance(s, cfg);
    case 'folder_loaded':
      return folderLoaded(s, ev.count, cfg, rand);
    case 'no_next_folder':
      return [idle, { type: 'stopped', reason: 'end' }];
    case 'pause':
      return [{ ...s, phase: 'paused' }, NONE];
    default: // start / resume while playing do not apply
      return [s, NONE];
  }
}
