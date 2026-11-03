// The image stage: progressive display (thumb -> full), fit/zoom/pan via CSS
// transform on a single <img>, DPR-aware requests, error card on failure.
// Package I.

import type { Ctx } from './ctx';

export interface Viewer {
  dispose(): void;
}

export function mountViewer(stage: HTMLElement, ctx: Ctx): Viewer {
  void stage; void ctx;
  return { dispose() {} }; // package I
}
