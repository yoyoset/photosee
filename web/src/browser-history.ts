// Visited folders for the back/forward buttons. "Up" is not history: it is
// the listing's own `parent`.

import type { Listing } from './api';
import { samePath } from './browser-path';

export class History {
  private entries: string[] = [];
  private pos = -1;

  get current(): string | null { return this.entries[this.pos] ?? null; }
  get canBack(): boolean { return this.pos > 0; }
  get canForward(): boolean { return this.pos >= 0 && this.pos < this.entries.length - 1; }

  /** A new visit drops the forward branch; revisiting the current folder is a no-op. */
  push(path: string): void {
    if (this.current !== null && samePath(this.current, path)) return;
    this.entries = [...this.entries.slice(0, this.pos + 1), path];
    this.pos = this.entries.length - 1;
  }

  /** The folder one step back/forward, without moving: the caller calls
   * `commit` once that folder actually opened, so a dead entry is not lost. */
  peek(d: -1 | 1): string | null { return this.entries[this.pos + d] ?? null; }
  commit(d: -1 | 1): void { this.pos = Math.min(Math.max(0, this.pos + d), this.entries.length - 1); }
}

export const upTarget = (l: Listing | null): string | null => l?.parent ?? null;
