// Measurement only: injected when PHOTOSEE_PERF_OUT is set at launch. Reports
// when the viewer's own <img> has decoded and been through two frames, which
// is the closest page-side proxy for "the picture is on screen".
(() => {
  if (location.protocol !== 'http:') return;
  const send = (kind, extra) =>
    fetch('/api/v1/host/perf', {
      method: 'POST',
      keepalive: true,
      body: JSON.stringify({ kind, t: performance.timeOrigin + performance.now(), ...extra }),
    }).catch(() => {});
  document.addEventListener('load', async (e) => {
    const img = e.target;
    if (!(img instanceof HTMLImageElement) || !img.closest('#stage')) return;
    const src = img.currentSrc;
    const kind = src.includes('/thumb') ? 'paint_thumb' : 'paint_img';
    try { await img.decode(); } catch { /* still paint what we have */ }
    requestAnimationFrame(() => requestAnimationFrame(() => send(kind, { w: img.naturalWidth })));
  }, true);
})();
