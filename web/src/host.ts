// What the UI needs from the window it runs in. The browser build uses the
// Fullscreen API; the desktop shell (app/, later) provides its own object as
// `window.__PHOTOSEE_HOST__` before the page script runs.

export interface Host {
  /** May reject (browsers demand a user gesture); callers re-read `isFullscreen()`. */
  setFullscreen(on: boolean): Promise<void> | void;
  isFullscreen(): boolean;
  close(): void;
  /** Optional: report changes the UI did not ask for (F11, the browser's own Esc). */
  onFullscreenChange?(cb: (on: boolean) => void): () => void;
  /** Desktop only: the native folder picker. Resolves to the picked path,
   * or null when the user cancels. Absent in the browser build. */
  pickFolder?(): Promise<string | null>;
}

type HostWindow = Window & { __PHOTOSEE_HOST__?: Host };

export function browserHost(doc: Document = document, win: Window = window): Host {
  return {
    async setFullscreen(on) {
      if (on && !doc.fullscreenElement) await doc.documentElement.requestFullscreen();
      else if (!on && doc.fullscreenElement) await doc.exitFullscreen();
    },
    isFullscreen: () => doc.fullscreenElement !== null,
    // Browsers only honor close() for windows a script opened; nothing else to try.
    close: () => win.close(),
    onFullscreenChange(cb) {
      const fn = (): void => cb(doc.fullscreenElement !== null);
      doc.addEventListener('fullscreenchange', fn);
      return () => doc.removeEventListener('fullscreenchange', fn);
    },
  };
}

export function pickHost(doc: Document = document, win: Window = window): Host {
  return (win as HostWindow).__PHOTOSEE_HOST__ ?? browserHost(doc, win);
}
