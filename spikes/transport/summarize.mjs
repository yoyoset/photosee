// Prints the REPORT.md tables from results/*.json. Pure arithmetic, no I/O
// beyond reading the result files.
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RESULTS = join(dirname(fileURLToPath(import.meta.url)), 'results');
const load = (prefix) => readdirSync(RESULTS).filter((f) => f.startsWith(prefix) && f.endsWith('.json'))
  .sort((a, b) => a.localeCompare(b, 'en', { numeric: true })).map((f) => JSON.parse(readFileSync(join(RESULTS, f), 'utf8')));

export function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

// Nearest-rank p90: with n=10 this is the 9th smallest value.
export function p90(xs) {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.max(0, Math.ceil(0.9 * s.length) - 1)];
}

const f1 = (x) => (x == null || Number.isNaN(x) ? 'n/a' : x.toFixed(1));
const mib = (b) => (b / 2 ** 20).toFixed(1);

function transportTable(benches) {
  console.log('| file | transport | n | decode median | decode p90 | fetch median | fetch p90 | bytes |');
  console.log('|---|---|---|---|---|---|---|---|');
  const keys = Object.keys(benches[0]?.page.transport ?? {});
  for (const k of keys) {
    const rows = benches.map((b) => b.page.transport[k]);
    const dec = rows.flatMap((r) => r.decode);
    const fet = rows.flatMap((r) => r.fetch);
    const [file, tr] = k.split('/');
    console.log(`| ${file} | ${tr} | ${dec.length} | ${f1(median(dec))} | ${f1(p90(dec))} | ${f1(median(fet))} | ${f1(p90(fet))} | ${rows[0].bytes} |`);
  }
}

function prefetchTable(benches) {
  console.log('\n| image | n | rAF median | rAF p90 | rAF max | 2nd rAF median | paint median | paint p90 | waited for prefetch median |');
  console.log('|---|---|---|---|---|---|---|---|---|');
  for (const name of Object.keys(benches[0]?.page.prefetch ?? {})) {
    const rows = benches.flatMap((b) => b.page.prefetch[name]);
    const col = (k) => rows.map((r) => r[k]).filter((x) => x != null);
    const paint = col('paint');
    console.log(`| ${name} | ${rows.length} | ${f1(median(col('raf1')))} | ${f1(p90(col('raf1')))} | ${f1(Math.max(...col('raf1')))} | ${f1(median(col('raf2')))} | ${paint.length ? f1(median(paint)) : 'n/a'} | ${paint.length ? f1(p90(paint)) : 'n/a'} | ${f1(median(col('waited_for_prefetch')))} |`);
  }
}

// Stability check: the same medians, one row per bench launch.
function perLaunch(benches) {
  const tk = Object.keys(benches[0].page.transport);
  const pk = Object.keys(benches[0].page.prefetch);
  console.log(`
| launch | ${tk.map((k) => `${k} decode`).join(' | ')} | ${pk.map((k) => `${k} rAF / paint`).join(' | ')} |`);
  console.log(`|---|${tk.map(() => '---|').join('')}${pk.map(() => '---|').join('')}`);
  benches.forEach((b, i) => {
    const t = tk.map((k) => f1(median(b.page.transport[k].decode)));
    const p = pk.map((k) => {
      const rows = b.page.prefetch[k];
      return `${f1(median(rows.map((r) => r.raf1)))} / ${f1(median(rows.map((r) => r.paint).filter((x) => x != null)))}`;
    });
    console.log(`| ${i + 1} | ${t.join(' | ')} | ${p.join(' | ')} |`);
  });
}

const COLD_COLS = [
  ['main', (d) => d.rust_marks_epoch.main],
  ['axum listening', (d) => d.rust_marks_epoch.axum_listening],
  ['tauri setup', (d) => d.rust_marks_epoch.tauri_setup],
  ['WebView2 created', (d) => d.rust_marks_epoch.webview_created],
  ['nav start', (d) => d.page.time_origin],
  ['image file read', (d) => d.rust_marks_epoch.first_file_read],
  ['image requested', (d) => d.rust_marks_epoch.first_requested],
  ['image received', (d) => d.page.first_response_end_epoch],
  ['decode() resolved', (d) => d.page.decoded_epoch],
  ['painted', (d) => d.page.first_render_epoch],
];

function coldTable(colds) {
  console.log('\nAll values: ms since kernel process creation (GetProcessTimes).\n');
  console.log(`| launch | spawn->created | ${COLD_COLS.map((c) => c[0]).join(' | ')} | idle WS MiB | idle private WS MiB | procs |`);
  console.log(`|---|---|${COLD_COLS.map(() => '---|').join('')}---|---|---|`);
  const cols = COLD_COLS.map(() => []);
  const mem = { ws: [], pws: [] };
  colds.forEach((d, i) => {
    const vals = COLD_COLS.map(([, g], j) => { const v = g(d) - d.created_epoch; cols[j].push(v); return v; });
    const spawn = d.launched_at ? d.created_epoch - d.launched_at : NaN;
    const m = d.idle_memory;
    mem.ws.push(m.ws_bytes); mem.pws.push(m.private_ws_bytes);
    console.log(`| ${i + 1} | ${f1(spawn)} | ${vals.map(f1).join(' | ')} | ${mib(m.ws_bytes)} | ${mib(m.private_ws_bytes)} | ${m.count} |`);
  });
  console.log(`| **median** | | ${cols.map((c) => f1(median(c))).join(' | ')} | ${mib(median(mem.ws))} | ${mib(median(mem.pws))} | |`);
  console.log(`| **max** | | ${cols.map((c) => f1(Math.max(...c))).join(' | ')} | ${mib(Math.max(...mem.ws))} | ${mib(Math.max(...mem.pws))} | |`);
}

function processBreakdown(colds) {
  const last = colds.at(-1)?.idle_memory;
  if (!last) return;
  console.log(`\nProcess tree of the last cold launch, ${f1(last.ms_after_decode)} ms after decode():\n`);
  console.log('| pid | name | WS MiB | private WS MiB |\n|---|---|---|---|');
  for (const p of last.procs) console.log(`| ${p.pid} | ${p.name} | ${mib(p.ws)} | ${mib(p.private_ws ?? 0)} |`);
}

const benches = load('bench-');
if (benches.length) { transportTable(benches); prefetchTable(benches); perLaunch(benches); }
for (const kind of ['photo-24mp', 'photo-screen', 'fresh-photo-24mp']) {
  const colds = load(`cold-${kind}-`);
  if (!colds.length) continue;
  console.log(`
### Cold start: ${kind}`);
  coldTable(colds);
  processBreakdown(colds);
}
