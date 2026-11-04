// Defines window.__PHOTOSEE_HOST__ (web/src/host.ts `Host`) for the page the
// embedded server serves. Plain fetch to /api/v1/host/*: the page is plain
// HTTP, so it gets no Tauri IPC (a capability for it would widen the attack
// surface). The session cookie authenticates these calls like any other.
(() => {
  if (window.__PHOTOSEE_HOST__ || location.protocol !== 'http:') return;
  let on = false;
  const subs = new Set();
  const apply = (v) => {
    if (typeof v !== 'boolean' || v === on) return;
    on = v;
    subs.forEach((cb) => cb(on));
  };
  const call = (path, body) =>
    fetch('/api/v1/host/' + path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body ?? {}),
    }).then((r) => (r.ok ? r.json() : Promise.reject(new Error('host ' + path + ': ' + r.status))));
  window.__PHOTOSEE_HOST__ = Object.freeze({
    setFullscreen: (want) => call('fullscreen', { on: !!want }).then((j) => apply(j.on)),
    isFullscreen: () => on,
    // Native folder picker (modal). Resolves to a path, or null on cancel.
    pickFolder: () => call('pick-folder').then((j) => j.path ?? null),
    // Hides the window; the process stays resident (see main.rs).
    close: () => void call('close').catch(() => {}),
    onFullscreenChange(cb) {
      subs.add(cb);
      return () => subs.delete(cb);
    },
  });
  fetch('/api/v1/host/fullscreen').then((r) => r.json()).then((j) => apply(j.on)).catch(() => {});
  // Quit shortcut: the only way out of a resident process without a tray icon.
  const mac = /^Mac/.test(navigator.platform);
  addEventListener('keydown', (e) => {
    const mod = mac ? e.metaKey : e.ctrlKey;
    if (mod && !e.shiftKey && !e.altKey && (e.key === 'q' || e.key === 'Q')) {
      e.preventDefault();
      call('quit').catch(() => {});
    }
  }, true);
})();
