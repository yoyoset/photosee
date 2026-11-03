// Slideshow state machine, pure (time and randomness are injected).
// Package B. Rules: docs/ux-spec.md §slideshow.

import type { Settings } from './api';

export type SlideEvent =
  | { type: 'start'; index: number; count: number }
  | { type: 'tick' }
  | { type: 'pause' } | { type: 'resume' } | { type: 'stop' }
  | { type: 'image_failed' }
  | { type: 'folder_loaded'; count: number };

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

export function step(
  s: SlideState,
  ev: SlideEvent,
  cfg: Pick<Settings, 'on_end' | 'shuffle'>,
  rand: () => number,
): [SlideState, SlideEffect] {
  void cfg; void rand; void ev;
  return [s, { type: 'none' }]; // package B
}
