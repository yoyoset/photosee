// Multi-user server: each reader comes back to the comic and page they left
// (the user's choice, 2026-11-05). The page is kept in the reader's own
// ui-state as `reading: {dir, file}`. Both are paths the server gave us: the
// UI never splits a path (docs/api.md), so the folder is stored, not derived.
//
// Desktop does not record or resume: it starts from what was double-clicked.

import type { Session } from './api';
import type { BrowserNav } from './browser-nav';
import type { Store } from './state';
import type { UiPrefs } from './uistate';
import { FIT } from './zoom';

export const resumes = (s: Session): boolean => s.mode === 'server';

/** Remembers the image on screen whenever the viewer shows a new one. */
export function recordReading(store: Store, prefs: UiPrefs): () => void {
  return store.subscribe((s) => {
    const f = s.mode === 'viewer' ? s.listing?.images[s.index] : undefined;
    if (!f || !s.listing || prefs.get().reading?.file === f.path) return;
    prefs.set({ reading: { dir: s.listing.dir, file: f.path } });
  });
}

/** Opens the last page read. False when there is none or its folder is gone
 * (the caller falls back to the folder list); a folder that still exists
 * without that page just stays open in the browser. */
export async function resumeReading(store: Store, prefs: UiPrefs, nav: BrowserNav): Promise<boolean> {
  const r = prefs.get().reading;
  if (!r || (await nav.go(r.dir, { quiet: true })) !== null) return false;
  const i = store.get().listing?.images.findIndex((x) => x.path === r.file) ?? -1;
  // One set(): the viewer mounts on the mode change and must already see the index.
  if (i >= 0) store.set({ index: i, zoom: FIT, rotation: 0, mode: 'viewer' });
  return store.get().listing !== null;
}
