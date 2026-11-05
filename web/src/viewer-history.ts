// The viewer as one browser-history entry. On a phone the system Back
// button / edge swipe is the only "close" there is (no Esc key); without an
// entry of our own it would leave PhotoSee for whatever page came before
// (the homepage). Desktop gains mouse-back / Alt+Left for free.

/**
 * Pushes an entry now; Back (popstate) calls `close`. The returned dispose
 * removes the entry again when the viewer was left another way (Esc, the
 * back button), so Back from the folder view still means "previous page".
 */
export function bindViewerHistory(win: Window, close: () => void): () => void {
  win.history.pushState({ photoseeViewer: true }, '');
  let popped = false;
  const onPop = (): void => {
    popped = true;
    close();
  };
  win.addEventListener('popstate', onPop);
  return () => {
    win.removeEventListener('popstate', onPop);
    if (!popped) win.history.back();
  };
}
