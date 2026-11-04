// Type-ahead: typing letters jumps to the next name with that prefix.

import { TYPEAHEAD_RESET_MS } from './browser-consts';

export class TypeAhead {
  private buf = '';
  private last = -Infinity;

  /** Append a character (time injected for tests); a pause starts a new word. */
  push(ch: string, now: number): string {
    if (now - this.last > TYPEAHEAD_RESET_MS) this.buf = '';
    this.last = now;
    this.buf += ch.toLowerCase();
    return this.buf;
  }

  reset(): void { this.buf = ''; }
}

/** Index of the next name starting with `prefix`, searching from `from` and
 * wrapping; -1 if none. A single letter starts after `from` (pressing "a"
 * again moves to the next a-name); a longer prefix may stay on `from`. */
export function findPrefix(names: readonly string[], prefix: string, from: number): number {
  if (prefix === '' || names.length === 0) return -1;
  const n = names.length;
  const first = prefix.length === 1 ? from + 1 : Math.max(from, 0);
  for (let k = 0; k < n; k++) {
    const i = (((first + k) % n) + n) % n;
    if ((names[i] as string).toLowerCase().startsWith(prefix)) return i;
  }
  return -1;
}
