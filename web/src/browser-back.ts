// Back (the phone's back gesture, a mouse's back button) in the folder view.
// Without this the page had no history of its own outside the viewer, so
// Back from a folder left PhotoSee for whatever came before it (found in the
// phone review, 2026-11-06).
//
// One "guard" entry sits above the page's own entry. Back pops it; we then do
// one step inside the app (close the drawer, previous folder, parent folder)
// and push the guard again. Only when there is nothing left to step back to
// is the guard not restored, so the next Back leaves as a browser would.
//
// The viewer pushes its own entry above the guard (viewer-history.ts). Popping
// that one lands on the guard and is the viewer's business, not ours.

import { viewerOwnsPop } from './viewer-history';

const GUARD = { photoseeGuard: true } as const;
const isGuard = (s: unknown): boolean => (s as { photoseeGuard?: unknown } | null)?.photoseeGuard === true;

export interface BrowserBack {
  /** Restore the guard if Back used it up (call after any navigation). */
  arm(): void;
  dispose(): void;
}

/** `step`: one step back inside the app; false when there is none. Not
 * called while `inViewer()` (the viewer closes itself on that Back). */
export function bindBrowserBack(win: Window, inViewer: () => boolean, step: () => boolean): BrowserBack {
  const arm = (): void => { if (!isGuard(win.history.state)) win.history.pushState(GUARD, ''); };
  const onPop = (e: PopStateEvent): void => {
    if (isGuard(e.state) || viewerOwnsPop() || inViewer()) return;
    if (step()) arm();
  };
  win.addEventListener('popstate', onPop);
  arm();
  return { arm, dispose: () => win.removeEventListener('popstate', onPop) };
}
