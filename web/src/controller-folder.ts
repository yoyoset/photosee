// Loading one folder listing into the store, and what the user sees when that
// goes wrong (docs/ux-spec.md §Errors): NAS banner with retry, empty notice.

import { ApiError, type Listing } from './api';
import { FOLDER_TOAST_MS, HINT_TOAST_MS } from './constants';
import type { Ctx } from './ctx';
import { errorSentence, showEmptyFolder, showNasBanner, showToast } from './errors';
import { folderName } from './overlay';
import { FIT } from './zoom';

export interface FolderLoader {
  /** Start a request; an older request's answer is discarded from now on. */
  begin(): number;
  isCurrent(token: number): boolean;
  /** Banner for transient failures, a toast for the rest. */
  report(err: unknown, dir: string, retry: () => void): void;
  /** List `dir` and show it, starting at `file` (by path) if given.
   * Null: it failed (reported) or a newer request took over. */
  openDir(dir: string, file?: string | null, announce?: boolean): Promise<Listing | null>;
}

const TRANSIENT = new Set<ApiError['code']>(['timeout', 'io', 'network']);

export function createFolderLoader(getCtx: () => Ctx, getNotices: () => HTMLElement): FolderLoader {
  // A slow older answer must not overwrite a newer one.
  let seq = 0;
  let dismissBanner: (() => void) | null = null;
  let dismissEmpty: (() => void) | null = null;

  const begin = (): number => ++seq;

  function report(err: unknown, dir: string, retry: () => void): void {
    const e = err instanceof ApiError ? err : new ApiError('io', String(err));
    if (TRANSIENT.has(e.code)) {
      dismissBanner = showNasBanner(getNotices(), dir, retry, e.code === 'timeout' ? 'timeout' : 'io');
    } else {
      showToast(getNotices(), errorSentence(e), HINT_TOAST_MS);
    }
  }

  async function openDir(dir: string, file?: string | null, announce = false): Promise<Listing | null> {
    const ctx = getCtx();
    const mine = begin();
    let listing: Listing;
    try {
      listing = await ctx.api.list(dir);
    } catch (err) {
      if (mine === seq) report(err, dir, () => void openDir(dir, file, announce));
      return null;
    }
    if (mine !== seq) return null;
    dismissBanner?.();
    dismissEmpty?.();
    dismissBanner = dismissEmpty = null;
    const found = file ? listing.images.findIndex((x) => x.path === file) : -1;
    ctx.store.set({ listing, index: Math.max(0, found), zoom: FIT, rotation: 0 });
    if (listing.images.length === 0) dismissEmpty = showEmptyFolder(getNotices(), ctx.session.platform);
    else if (announce) showToast(getNotices(), folderName(listing.dir), FOLDER_TOAST_MS);
    return listing;
  }

  return { begin, isCurrent: (token) => token === seq, report, openDir };
}
