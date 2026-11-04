// Folder navigation for the browser: opening a folder, history, "up", and
// the bookkeeping that follows a successful open (recents, last folder,
// network places, the viewer's "next folder" root).

import type { ApiError, Listing, Place } from './api';
import { History, upTarget } from './browser-history';
import { isUnc, isUnder, normPath, samePath } from './browser-path';
import type { Ctx } from './ctx';
import { errorSentence } from './errors';
import { pushRecent, type UiPrefs } from './uistate';

export interface GoOptions {
  /** The user typed the path: report a failure as text for the address bar. */
  typed?: boolean;
  /** Try a folder quietly (start-up fallbacks): a failure is returned, not shown. */
  quiet?: boolean;
  /** Select this path once the folder shows (the child we came up from). */
  select?: string;
  /** Back/forward move through history instead of adding to it. */
  history?: 'push' | 'none';
}

type OpenDir = (dir: string, file?: string | null, announce?: boolean, onError?: (e: ApiError) => void) => Promise<Listing | null>;

/** The place that "next folder" should stay inside: the longest known place
 * containing `dir`, or `dir` itself when it is not under any. */
export function chooseRoot(dir: string, places: readonly string[]): string {
  const hit = places.filter((p) => isUnder(p, dir)).sort((a, b) => normPath(b).length - normPath(a).length)[0];
  return hit ?? dir;
}

export class BrowserNav {
  readonly history = new History();
  /** Read (and cleared) by the browser when the next listing arrives. */
  pendingSelect: string | null = null;
  /** After a folder opened and history / recents were updated. */
  onChanged: () => void = () => {};

  constructor(
    private readonly ctx: Ctx,
    private readonly prefs: UiPrefs,
    /** Failures of clicks and keys (not typed paths) are reported by the
     * loader itself, as a toast or the NAS banner with retry. */
    private readonly openDir: OpenDir,
  ) {}

  get listing(): Listing | null { return this.ctx.store.get().listing; }
  get canUp(): boolean { return upTarget(this.listing) !== null; }

  /** Resolves to a sentence when a typed path failed, else null. */
  async go(dir: string, o: GoOptions = {}): Promise<string | null> {
    const target = dir.trim().replace(/^"(.*)"$/, '$1');
    if (target === '') return null;
    let message: string | null = null;
    this.pendingSelect = o.select ?? null;
    const listing = await (o.typed || o.quiet
      ? this.openDir(target, null, false, (e) => { message = errorSentence(e); })
      : this.openDir(target, null, false));
    if (!listing) {
      this.pendingSelect = null;
      return message;
    }
    if ((o.history ?? 'push') === 'push') this.history.push(listing.dir);
    this.remember(listing.dir, o.typed === true);
    this.onChanged();
    return null;
  }

  async up(): Promise<void> {
    const from = this.listing;
    const parent = upTarget(from);
    if (parent !== null && from) await this.go(parent, { select: from.dir });
  }

  async back(): Promise<void> { await this.step(-1); }
  async forward(): Promise<void> { await this.step(1); }

  private async step(d: -1 | 1): Promise<void> {
    const to = this.history.peek(d);
    if (to === null) return;
    // Commit only once it opened: a folder that vanished must not eat the entry.
    await this.go(to, { history: 'none' });
    if (this.listing && samePath(this.listing.dir, to)) this.history.commit(d);
  }

  private remember(dir: string, typed: boolean): void {
    const p = this.prefs.get();
    const places = this.places();
    const patch: Parameters<UiPrefs['set']>[0] = { recent: pushRecent(p.recent, dir), last: dir };
    if (typed && isUnc(dir) && !places.some((x) => isUnder(x, dir))) patch.network = [...p.network, dir];
    this.prefs.set(patch);
    this.ctx.store.set({ root: chooseRoot(dir, places) });
  }

  private places(): string[] {
    const roots: Place[] = this.ctx.session.roots;
    return [...roots.map((r) => r.path), ...this.prefs.get().network];
  }
}
