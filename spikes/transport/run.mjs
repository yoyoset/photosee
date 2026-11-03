// Driver: launches the release binary repeatedly, no clicking needed.
//   node run.mjs gen          write the test JPEGs into samples/
//   node run.mjs machine      CPU / GPU / Windows build / WebView2 -> results/machine.json
//   node run.mjs bench [n]    n bench launches -> results/bench-<i>.json
//   node run.mjs cold [n] [img] n cold launches with samples/<img>.jpg (default
//                             photo-24mp) as CLI arg + idle memory -> results/cold-<img>-<i>.json
//   node run.mjs cold [n] [img] --fresh-profile   same, deleting the WebView2 profile first
//   node summarize.mjs        markdown tables from results/
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const EXE = join(HERE, 'target', 'release', 'transport-spike.exe');
const SAMPLES = join(HERE, 'samples');
const RESULTS = join(HERE, 'results');
const nowEpoch = () => performance.timeOrigin + performance.now();
const sleep = (ms) => new Promise((r) => setTimeout(r, Math.max(0, ms)));

function ps(script) {
  const utf8 = '[Console]::OutputEncoding = [Text.Encoding]::UTF8; ';
  const out = execFileSync('powershell', ['-NoProfile', '-Command', utf8 + script], { encoding: 'utf8' });
  return JSON.parse(out);
}

function readJson(path) {
  try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return null; }
}

async function waitForJson(path, timeoutMs) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    const doc = existsSync(path) && readJson(path);
    if (doc) return doc;
    await sleep(20);
  }
  throw new Error(`timed out waiting for ${path}`);
}

// Working set of `root` and all descendants (app + WebView2 browser, GPU,
// renderer, utility processes). Summed WS double-counts shared DLL pages, so
// the private working set is reported next to it.
function treeMemory(root) {
  const d = ps(`@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,WorkingSetSize) | ConvertTo-Json -Compress`);
  const kids = new Map();
  for (const p of d) kids.set(p.ParentProcessId, [...(kids.get(p.ParentProcessId) ?? []), p]);
  const tree = [d.find((p) => p.ProcessId === root)];
  const visit = (pid) => {
    for (const c of kids.get(pid) ?? []) { tree.push(c); visit(c.ProcessId); }
  };
  visit(root);
  const procs = tree.filter(Boolean).map((p) => ({ pid: p.ProcessId, name: p.Name, ws: Number(p.WorkingSetSize) }));
  const priv = privateWs(procs.map((p) => p.pid));
  for (const p of procs) p.private_ws = priv.get(p.pid) ?? null;
  const sum = (k) => procs.reduce((a, p) => a + (p[k] ?? 0), 0);
  return { count: procs.length, ws_bytes: sum('ws'), private_ws_bytes: sum('private_ws'), procs };
}

// Pitfall (seen on the first trial run): the WMI perf provider can answer
// from a process list that predates our just-launched tree, so every lookup
// missed. Re-query until every pid is present.
function privateWs(pids) {
  let map = new Map();
  for (let attempt = 0; attempt < 5; attempt++) {
    const f = ps(`@(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Select-Object IDProcess,WorkingSetPrivate) | ConvertTo-Json -Compress`);
    map = new Map(f.map((x) => [x.IDProcess, Number(x.WorkingSetPrivate)]));
    if (pids.every((p) => map.has(p))) break;
    execFileSync('powershell', ['-NoProfile', '-Command', 'Start-Sleep -Milliseconds 700']);
  }
  return map;
}

function launch(args) {
  const child = spawn(EXE, args, { stdio: 'ignore' });
  const exited = new Promise((r) => child.on('exit', (code) => r(code)));
  return { child, exited };
}

// The spike's own WebView2 profile (Tauri: %LOCALAPPDATA%\<identifier>\EBWebView).
// Deleting it simulates the first launch after install.
const PROFILE = join(process.env.LOCALAPPDATA ?? '', 'dev.photosee.transport-spike');

async function coldOnce(i, image, fresh) {
  const out = join(RESULTS, `cold-${fresh ? 'fresh-' : ''}${image}-${i}.json`);
  rmSync(out, { force: true });
  if (fresh) rmSync(PROFILE, { recursive: true, force: true });
  const launched = nowEpoch();
  const img = join(SAMPLES, `${image}.jpg`);
  const { child, exited } = launch([img, '--out', out, '--hold', '14', '--launched-at', String(launched)]);
  const doc = await waitForJson(out, 30000);
  await sleep(doc.page.decoded_epoch + 10000 - nowEpoch());
  const t = nowEpoch();
  doc.idle_memory = treeMemory(child.pid);
  doc.idle_memory.ms_after_decode = t - doc.page.decoded_epoch;
  writeFileSync(out, JSON.stringify(doc, null, 2));
  const code = await exited;
  console.log(`cold ${image} ${i}: exit ${code}, ${doc.idle_memory.count} procs, ${(doc.idle_memory.ws_bytes / 2 ** 20).toFixed(1)} MiB WS`);
}

async function benchOnce(i) {
  const out = join(RESULTS, `bench-${i}.json`);
  rmSync(out, { force: true });
  const { exited } = launch(['--bench', SAMPLES, '--out', out]);
  await waitForJson(out, 600000);
  console.log(`bench ${i}: exit ${await exited}`);
}

function machine() {
  const m = ps(String.raw`@{ cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1 Name,NumberOfCores,NumberOfLogicalProcessors);
    gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate);
    os = (Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,TotalVisibleMemorySize);
    ubr = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').UBR;
    webview2 = (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}').pv;
    lastboot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToString('o') } | ConvertTo-Json -Depth 3`);
  writeFileSync(join(RESULTS, 'machine.json'), JSON.stringify(m, null, 2));
  console.log(m);
}

async function main() {
  mkdirSync(RESULTS, { recursive: true });
  const [cmd, nArg, image = 'photo-24mp', flag] = process.argv.slice(2);
  const n = Number(nArg ?? (cmd === 'cold' ? 5 : 1));
  if (cmd === 'gen') execFileSync(join(HERE, 'target', 'release', 'gen-jpegs.exe'), [SAMPLES], { stdio: 'inherit' });
  else if (cmd === 'machine') machine();
  else if (cmd === 'bench') for (let i = 1; i <= n; i++) await benchOnce(i);
  else if (cmd === 'cold') for (let i = 1; i <= n; i++) { await coldOnce(i, image, flag === '--fresh-profile'); await sleep(3000); }
  else throw new Error('usage: node run.mjs gen|machine|bench [n]|cold [n]');
}

main().catch((e) => { console.error(e); process.exit(1); });
