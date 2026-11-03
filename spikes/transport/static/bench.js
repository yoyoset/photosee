// Transport benchmark (a: axum on 127.0.0.1, b: custom protocol), then the
// prefetched-next runs. Results go to POST /result; the app then exits.
import { prefetchRun, sleep } from './prefetch.js';

const RUNS = 10;
const SIZES = ['noise-3mb', 'noise-30mb'];
const BASE = { axum: `${location.origin}/file?n=`, proto: 'http://ps.localhost/file?n=' };
const bust = () => `&r=${Date.now()}-${Math.random().toString(36).slice(2)}`;

// The brief's metric: from `img.src = ...` to `img.decode()` resolving.
async function decodeOnce(url) {
  const img = new Image();
  const t0 = performance.now();
  img.src = url;
  await img.decode();
  return performance.now() - t0;
}

// Transfer only, no decode, to separate transport cost from decode cost.
async function fetchOnce(url) {
  const t0 = performance.now();
  const buf = await (await fetch(url)).arrayBuffer();
  return { ms: performance.now() - t0, bytes: buf.byteLength };
}

async function transports() {
  const out = {};
  for (const s of SIZES) for (const tr of Object.keys(BASE)) out[`${s}/${tr}`] = { decode: [], fetch: [], bytes: 0 };
  for (let i = 0; i < RUNS; i++) {
    for (const s of SIZES) {
      // Alternate the order so neither transport always runs first.
      for (const tr of i % 2 ? ['proto', 'axum'] : ['axum', 'proto']) {
        const row = out[`${s}/${tr}`];
        row.decode.push(await decodeOnce(BASE[tr] + s + bust()));
        await sleep(100);
        const f = await fetchOnce(BASE[tr] + s + bust());
        row.fetch.push(f.ms);
        row.bytes = f.bytes;
        await sleep(100);
      }
    }
  }
  return out;
}

async function main() {
  const res = { ua: navigator.userAgent, dpr: devicePixelRatio, viewport: [innerWidth, innerHeight] };
  try {
    res.transport = await transports();
    res.prefetch = {};
    for (const name of ['photo-screen', 'photo-24mp']) res.prefetch[name] = await prefetchRun(name);
  } catch (e) {
    res.error = String(e && e.stack || e);
  }
  await fetch('/result', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(res) });
}

main();
