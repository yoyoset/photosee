// Image dimensions for the details list and the status bar. /meta is the only
// source and costs a file open (a NAS round trip), so the rule is: only for
// what is on screen, only after scrolling went quiet, a few at a time, and
// each path at most once.

import type { Api } from './api';
import { META_CONCURRENCY, META_DEBOUNCE_MS } from './browser-consts';
import type { Dims } from './browser-status';

export class MetaCache {
  private readonly known = new Map<string, Dims>();
  /** Paths that were asked for (answered, failed or in flight): never again. */
  private readonly asked = new Set<string>();
  private queue: string[] = [];
  private timer: ReturnType<typeof setTimeout> | undefined;
  private running = 0;
  private dead = false;

  constructor(
    private readonly api: Pick<Api, 'meta'>,
    private readonly onUpdate: () => void,
  ) {}

  get(path: string): Dims | undefined { return this.known.get(path); }

  /** Replace what is wanted (the rows now visible); the fetch starts after a quiet period. */
  want(paths: readonly string[]): void {
    this.queue = paths.filter((p) => !this.asked.has(p));
    clearTimeout(this.timer);
    if (this.queue.length === 0) return;
    this.timer = setTimeout(() => this.pump(), META_DEBOUNCE_MS);
  }

  private pump(): void {
    while (!this.dead && this.running < META_CONCURRENCY) {
      const path = this.queue.shift();
      if (path === undefined) return;
      if (this.asked.has(path)) continue;
      this.asked.add(path);
      this.running++;
      this.api.meta(path).then(
        (m) => { this.known.set(path, { w: m.width, h: m.height }); this.onUpdate(); },
        () => { /* unreadable file: the column just stays empty */ },
      ).finally(() => { this.running--; this.pump(); });
    }
  }

  clear(): void {
    this.known.clear();
    this.asked.clear();
    this.queue = [];
  }

  dispose(): void {
    this.dead = true;
    clearTimeout(this.timer);
  }
}
