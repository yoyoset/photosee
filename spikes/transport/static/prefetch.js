// Prefetched next: keep 3 images ahead decoded with `new Image()` + decode(),
// swap the visible <img> to the prefetched URL, time to the next rAF.
const N = 20;
const AHEAD = 3;
const DWELL_MS = 250;

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const frame = () => new Promise((r) => requestAnimationFrame(() => r(performance.now())));

export async function prefetchRun(name) {
  const view = document.getElementById('v');
  const tag = Math.random().toString(36).slice(2);
  const urls = Array.from({ length: N }, (_, i) => `${location.origin}/file?n=${name}&r=${tag}-${i}`);
  const pending = new Map();
  const pre = (i) => {
    if (i >= N || pending.has(i)) return;
    const im = new Image();
    im.src = urls[i];
    pending.set(i, im.decode().then(() => im));
  };
  // Element Timing gives the presentation time of the frame that painted the
  // image, which rAF cannot: rAF fires before that frame is rastered.
  const paints = new Map();
  const obs = new PerformanceObserver((l) => {
    for (const e of l.getEntries()) if (e.identifier === 'v') paints.set(e.url, e.renderTime);
  });
  obs.observe({ type: 'element' });
  for (let i = 0; i < AHEAD; i++) pre(i);
  const rows = [];
  for (let i = 0; i < N; i++) {
    const w0 = performance.now();
    await pending.get(i);
    const waited = performance.now() - w0;
    // A dwell like a viewer's; it also puts the swap at a random frame phase.
    await sleep(DWELL_MS);
    const t0 = performance.now();
    view.src = urls[i];
    const t1 = await frame();
    const t2 = await frame();
    rows.push({ swap: t0, raf1: t1 - t0, raf2: t2 - t0, waited_for_prefetch: waited });
    pending.delete(i - 1);
    for (let k = 1; k <= AHEAD; k++) pre(i + k);
  }
  await sleep(500);
  obs.disconnect();
  rows.forEach((r, i) => {
    const p = paints.get(urls[i]);
    r.paint = p ? p - r.swap : null;
  });
  return rows;
}
