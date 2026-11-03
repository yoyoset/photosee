// Settings panel: exactly the six options in docs/ux-spec.md. Package L.

import type { Ctx } from './ctx';

export function mountSettings(root: HTMLElement, ctx: Ctx): () => void {
  void root; void ctx;
  return () => {}; // package L
}
