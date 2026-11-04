// Page turns and folder switching. A page turn always resets zoom and display
// rotation (docs/ux-spec.md): rotation is a look at one image, not a mode.

import type { Listing } from './api';
import { HINT_TOAST_MS } from './constants';
import { createFolderLoader } from './controller-folder';
import type { Ctx } from './ctx';
import { showLastHint, showToast } from './errors';
import { t } from './i18n';
import { folderName } from './overlay';
import type { SlideshowHandle } from './slideshow-ui';
import { FIT } from './zoom';

export interface NavParts {
  slideshow: SlideshowHandle;
  /** Where toasts, banners and notices go. */
  notices: HTMLElement;
}

export type Turn = 'next' | 'prev' | 'first' | 'last' | number;

export interface Navigator {
  turn(to: Turn): void;
  /** `fromSlideshow`: the slideshow is waiting for folderLoaded/noNextFolder. */
  changeFolder(d: 'next' | 'prev', fromSlideshow?: boolean): Promise<void>;
  openDir(dir: string, file?: string | null, announce?: boolean): Promise<Listing | null>;
}

export function turn(ctx: Ctx, notices: HTMLElement, to: Turn): void {
  const { store, session } = ctx;
  const s = store.get();
  const count = s.listing?.images.length ?? 0;
  if (count === 0) return;
  const target = to === 'next' ? s.index + 1 : to === 'prev' ? s.index - 1
    : to === 'first' ? 0 : to === 'last' ? count - 1 : to;
  const index = Math.min(count - 1, Math.max(0, Math.trunc(target)));
  if (index === s.index) {
    // Only a deliberate step past an end gets a hint; first/last/goto stay quiet.
    if (to === 'next') showLastHint(notices, session.platform);
    else if (to === 'prev') showToast(notices, t('hint.first'), HINT_TOAST_MS);
    return;
  }
  store.set({ index, zoom: FIT, rotation: 0 });
}

export function createNavigator(getCtx: () => Ctx, getParts: () => NavParts): Navigator {
  const loader = createFolderLoader(getCtx, () => getParts().notices);
  // The slideshow's request may be superseded by a manual one; the answer
  // still has to reach it or it would wait forever.
  let slideshowWaiting = false;

  const giveUp = (): void => {
    if (slideshowWaiting) getParts().slideshow.noNextFolder();
    slideshowWaiting = false;
  };

  async function changeFolder(d: 'next' | 'prev', fromSlideshow = false): Promise<void> {
    const ctx = getCtx();
    const s = ctx.store.get();
    // Paused: the state machine drops folder_loaded in this phase, so a
    // switch now would leave it counting the wrong folder.
    if (s.slideshow === 'paused') return;
    if (fromSlideshow) slideshowWaiting = true;
    const dir = s.listing?.dir;
    if (!dir || !s.root) return giveUp();

    const mine = loader.begin();
    let target: string | null;
    try {
      target = await ctx.api.nav(dir, s.root, d);
    } catch (err) {
      if (!loader.isCurrent(mine)) return;
      loader.report(err, dir, () => void changeFolder(d, slideshowWaiting));
      return giveUp();
    }
    if (!loader.isCurrent(mine) || ctx.store.get().slideshow === 'paused') return;
    if (target === null) {
      if (!slideshowWaiting) showToast(getParts().notices, t('hint.no_next_folder'), HINT_TOAST_MS);
      return giveUp();
    }
    await switchTo(ctx, target, mine + 1);
  }

  /** `token`: what openDir's own begin() returns, to tell failure from "superseded". */
  async function switchTo(ctx: Ctx, dir: string, token: number): Promise<void> {
    const playing = ctx.store.get().slideshow === 'playing';
    const listing = await loader.openDir(dir, null, !playing);
    if (!listing) return loader.isCurrent(token) ? giveUp() : undefined;
    // A playing slideshow rebuilds its order for the new folder and toasts the name itself.
    if (ctx.store.get().slideshow === 'playing') {
      getParts().slideshow.folderLoaded(listing.images.length, folderName(listing.dir));
    }
    slideshowWaiting = false;
  }

  return { turn: (to) => turn(getCtx(), getParts().notices, to), changeFolder, openDir: loader.openDir };
}
