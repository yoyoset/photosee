// Measurement driver for the release build (docs/packages/W2-APP.md).
//   node app/measure.mjs startup [n]   cold start, resident re-open, idle memory
//   node app/measure.mjs fullscreen    does requestFullscreen() fullscreen the window?
//   node app/measure.mjs smoke         the manual checklist, driven over CDP key events
// Times come from PHOTOSEE_PERF_OUT lines (Unix epoch ms, see perf.rs) minus the
// driver's own clock stamped just before spawn(). The spike measured that gap to
// the kernel's process-creation time at 1.4 to 4.1 ms, so it is not corrected.
import { execFileSync, spawn } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const EXE = join(ROOT, 'target', 'release', 'photosee.exe');
const GEN = join(ROOT, 'fixtures', 'generated');
const WORK = join(tmpdir(), 'photosee-measure');
const now = () => performance.timeOrigin + performance.now();
const sleep = (ms) => new Promise((r) => setTimeout(r, Math.max(0, ms)));

function ps(script) {
  const utf8 = '[Console]::OutputEncoding = [Text.Encoding]::UTF8; ';
  return execFileSync('powershell', ['-NoProfile', '-Command', utf8 + script], { encoding: 'utf8' });
}

// Same method as the M0 spike: sum of "Working Set - Private" over the whole
// process tree under the exe (app + WebView2 browser/GPU/renderer/...).
function treeMemory(root) {
  const d = JSON.parse(ps('@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name) | ConvertTo-Json -Compress'));
  const kids = new Map();
  for (const p of d) kids.set(p.ParentProcessId, [...(kids.get(p.ParentProcessId) ?? []), p]);
  const pids = [root];
  const visit = (pid) => { for (const c of kids.get(pid) ?? []) { pids.push(c.ProcessId); visit(c.ProcessId); } };
  visit(root);
  let map = new Map();
  for (let attempt = 0; attempt < 5; attempt++) {
    const f = JSON.parse(ps('@(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Select-Object IDProcess,WorkingSetPrivate) | ConvertTo-Json -Compress'));
    map = new Map(f.map((x) => [x.IDProcess, Number(x.WorkingSetPrivate)]));
    if (pids.every((p) => map.has(p))) break;
    ps('Start-Sleep -Milliseconds 700');
  }
  const bytes = pids.reduce((a, p) => a + (map.get(p) ?? 0), 0);
  return { procs: pids.length, private_mib: +(bytes / 2 ** 20).toFixed(1) };
}

const lines = (file) => (existsSync(file) ? readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

async function waitPaint(file, fromIndex, timeoutMs = 20000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    const all = lines(file);
    const hit = all.slice(fromIndex).find((l) => l.kind === 'paint_img');
    if (hit) return { hit, all };
    await sleep(10);
  }
  throw new Error('no paint_img within timeout');
}

function launch(args, env = {}) {
  const t0 = now();
  const child = spawn(EXE, args, { stdio: 'ignore', env: { ...process.env, ...env } });
  return { child, t0 };
}

const killTree = (pid) => { try { execFileSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore' }); } catch { /* already gone */ } };
const closeWindow = (pid) => ps(`(Get-Process -Id ${pid}).CloseMainWindow() | Out-Null`);

async function startupOnce(i, files) {
  const out = join(WORK, `perf-${i}.jsonl`);
  rmSync(out, { force: true });
  const env = { PHOTOSEE_PERF_OUT: out };
  const { child, t0 } = launch([files.a], env);
  const cold = await waitPaint(out, 0);
  const marks = Object.fromEntries(cold.all.filter((l) => !l.kind.startsWith('paint')).map((l) => [l.kind, +(l.t - t0).toFixed(0)]));
  const thumb = cold.all.find((l) => l.kind === 'paint_thumb');
  const result = { cold_ms: +(cold.hit.t - t0).toFixed(0), cold_thumb_ms: thumb ? +(thumb.t - t0).toFixed(0) : null, marks };
  await sleep(3000);
  // Second launch while the window is visible, then while it is hidden.
  for (const [name, file, hide] of [['reopen_visible_ms', files.b, false], ['reopen_hidden_ms', files.c, true]]) {
    if (hide) { closeWindow(child.pid); await sleep(3000); }
    const from = lines(out).length;
    const second = launch([file], env);
    const r = await waitPaint(out, from);
    result[name] = +(r.hit.t - second.t0).toFixed(0);
    const th = r.all.slice(from).find((l) => l.kind === 'paint_thumb');
    result[name.replace('_ms', '_thumb_ms')] = th ? +(th.t - second.t0).toFixed(0) : null;
    await sleep(2000);
  }
  closeWindow(child.pid);
  await sleep(10000);
  result.idle_hidden_memory = treeMemory(child.pid);
  killTree(child.pid);
  console.log(JSON.stringify({ run: i, ...result }));
  return result;
}

async function startup(n) {
  mkdirSync(WORK, { recursive: true });
  const src = join(GEN, 'big24mp.jpg');
  const files = { a: src, b: join(WORK, 'b.jpg'), c: join(WORK, 'c.jpg') };
  copyFileSync(src, files.b);
  copyFileSync(src, files.c);
  for (let i = 0; i < n; i++) await startupOnce(i, files);
}

// Ground truth for the window's size: GetWindowRect on the process's main window.
const RECT_PS = (pid, hwnd) => `
Add-Type 'using System;using System.Runtime.InteropServices;public struct R{public int l,t,r,b;}public class W2{[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out R r);}';
$h=${hwnd ? `[IntPtr]${hwnd}` : `(Get-Process -Id ${pid}).MainWindowHandle`}; $r=New-Object R; [W2]::GetWindowRect($h,[ref]$r)|Out-Null;
"$($r.r-$r.l)x$($r.b-$r.t)"`;

// WebView2 reads this env var; CDP lets us call requestFullscreen with a user
// gesture, which a plain script cannot, and send real key events.
async function attach(args, env = {}) {
  mkdirSync(WORK, { recursive: true });
  const port = 9333;
  const { child } = launch(args, { ...env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` });
  let target;
  for (let i = 0; i < 100 && !target; i++) {
    await sleep(200);
    try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page' && t.url.startsWith('http://127.0.0.1')); } catch { /* not up yet */ }
  }
  if (!target) throw new Error('no CDP page target');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0;
  const pending = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); pending.get(d.id)?.(d); };
  const cdp = (method, params) => new Promise((r) => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })); });
  const evalJs = async (expression, userGesture = false) =>
    (await cdp('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true, userGesture })).result.result;
  const key = async (k, code = k.length === 1 ? 'Key' + k.toUpperCase() : k, vk = 0, modifiers = 0) => {
    for (const type of ['keyDown', 'keyUp']) {
      await cdp('Input.dispatchKeyEvent', { type, key: k, code, windowsVirtualKeyCode: vk, modifiers, text: type === 'keyDown' && k.length === 1 ? k : undefined });
    }
    await sleep(900);
  };
  return { child, evalJs, key };
}

const probe = `JSON.stringify({fsEl: !!document.fullscreenElement, inner: innerWidth+'x'+innerHeight, outer: outerWidth+'x'+outerHeight, screen: screen.width+'x'+screen.height, dpr: devicePixelRatio})`;

async function fullscreen() {
  const { child, evalJs } = await attach([join(GEN, 'big24mp.jpg')]);
  try {
    const state = async () => ({ page: JSON.parse((await evalJs(probe)).value), window_px: ps(RECT_PS(child.pid)).trim() });
    await sleep(2500);
    const out = { before: await state() };
    out.requestFullscreen = (await evalJs('document.documentElement.requestFullscreen().then(()=>"ok",e=>String(e))', !process.env.NO_GESTURE)).value;
    await sleep(1500);
    out.after_request = await state();
    await evalJs('document.exitFullscreen().then(()=>"ok",e=>String(e))', true);
    console.log(JSON.stringify(out, null, 2));
  } finally { killTree(child.pid); }
}

// Pitfall: MainWindowHandle changes once the main window is hidden (it falls
// back to the single-instance plugin's own visible helper window), so the
// handle is captured once and asked about directly.
const mainHandle = (pid) => ps(`(Get-Process -Id ${pid}).MainWindowHandle`).trim();
const visible = (hwnd) => ps(`Add-Type 'using System;using System.Runtime.InteropServices;public class V{[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);}'; [V]::IsWindowVisible([IntPtr]${hwnd})`).trim() === 'True';
const shown = async (evalJs) => (await evalJs(`(document.querySelector('#stage img')?.src ?? '').split('p=')[1]?.split('&')[0] ?? ''`)).value;

async function smoke() {
  const tree = join(GEN, 'tree');
  const { child, evalJs, key } = await attach([tree]);
  const log = (step, v) => console.log(`${step.padEnd(34)} ${JSON.stringify(v)}`);
  const fs = async () => JSON.parse((await evalJs(probe)).value).fsEl;
  let hwnd = null;
  const rect = () => ps(RECT_PS(child.pid, hwnd)).trim();
  try {
    await sleep(3500);
    hwnd = mainHandle(child.pid);
    log('1 folder opened, image', decodeURIComponent((await shown(evalJs))));
    log('  window (start_fullscreen default)', rect());
    const first = await shown(evalJs);
    await key('ArrowRight', 'ArrowRight', 39);
    log('2 ArrowRight, image changed', (await shown(evalJs)) !== first);
    await key('ArrowLeft', 'ArrowLeft', 37);
    log('  ArrowLeft, back to first', (await shown(evalJs)) === first);
    const screenRect = rect();
    await key('?', 'Slash');
    log('3 ? opens help (Esc 1 closes it)', await evalJs(`document.querySelector('#overlay')?.innerText.length > 0`).then((r) => r.value));
    await key('Escape', 'Escape', 27);
    log('  after Esc 1: window still full', rect() === screenRect);
    await key('Escape', 'Escape', 27);
    log('  after Esc 2: left fullscreen', rect() !== screenRect ? rect() : 'STILL FULL');
    log('  window visible before Esc 3', visible(hwnd));
    await key('Escape', 'Escape', 27);
    log('4 after Esc 3: hidden, process alive', { visible: visible(hwnd), alive: ps(`(Get-Process -Id ${child.pid} -ErrorAction SilentlyContinue) -ne $null`).trim() });
    const t0 = now();
    const second = spawn(EXE, [join(tree, 'a10.jpg')], { stdio: 'ignore' });
    const secondExit = await new Promise((r) => second.on('exit', r));
    log('5 2nd launch exited with code', secondExit);
    await sleep(2500);
    log('  window shown again', visible(hwnd));
    log('  image is the 2nd launch\'s', decodeURIComponent(await shown(evalJs)));
    log('  one app process (same pid)', child.pid);
    log('  ms from 2nd launch to now', Math.round(now() - t0));
    log('6 before F (UI re-entered fullscreen)', rect());
    await key('f', 'KeyF', 70);
    log('  after F', rect());
    await key('q', 'KeyQ', 81, 2);
    await sleep(1500);
    log('7 Ctrl+Q: process gone', ps(`(Get-Process -Id ${child.pid} -ErrorAction SilentlyContinue) -eq $null`).trim());
  } finally { killTree(child.pid); }
}

const [cmd, arg] = process.argv.slice(2);
if (cmd === 'startup') await startup(Number(arg ?? 3));
else if (cmd === 'fullscreen') await fullscreen();
else if (cmd === 'smoke') await smoke();
else console.log('usage: node app/measure.mjs startup [n] | fullscreen | smoke');
