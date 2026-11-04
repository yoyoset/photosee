// Install / run / uninstall smoke test for the NSIS installer (W3-PKG).
//   node app/nsis/smoke.mjs [path\to\PhotoSee_x_x64-setup.exe]
// Installs silently into a throw-away per-user dir under %TEMP% (never the
// default %LOCALAPPDATA%\PhotoSee, which is also the app's data dir), opens a
// generated JPEG with the INSTALLED exe, checks the page shows it, then
// uninstalls and diffs the file-association registry keys against a snapshot
// taken before the install. Exit code 1 on any failed check.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const nsisDir = join(ROOT, 'target', 'release', 'bundle', 'nsis');
const SETUP = process.argv[2] ?? join(nsisDir, readdirSync(nsisDir).filter((f) => f.endsWith('-setup.exe')).sort().pop());
const WORK = join(tmpdir(), 'photosee-pkg-smoke');
const INST = join(WORK, 'inst');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (name, ok, detail = '') => { if (!ok) failed++; console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? '  ' + detail : ''}`); };

const ps = (script) => execFileSync('powershell', ['-NoProfile', '-Command', script], { encoding: 'utf8' });
const reg = (args) => { try { return execFileSync('reg', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }); } catch { return ''; } };
const exts = (execFileSync('node', ['-e', `
  const s=require('fs').readFileSync(${JSON.stringify(join(ROOT, 'app', 'nsis', 'hooks.nsh'))},'utf8');
  console.log([...s.matchAll(/PS_ASSOC "([a-z0-9]+)"/g)].map(m=>m[1]).join(' '))`], { encoding: 'utf8' })).trim().split(' ');

// Everything the installer may touch, as text, so before/after compare exactly.
const snapshot = () => [
  ...exts.map((e) => reg(['query', `HKCU\\Software\\Classes\\.${e}`, '/s'])),
  reg(['query', 'HKCU\\Software\\PhotoSee', '/s']),
  reg(['query', 'HKCU\\Software\\Classes\\PhotoSee.Image', '/s']),
  reg(['query', 'HKCU\\Software\\Classes\\PhotoSee.Raw', '/s']),
  reg(['query', 'HKCU\\Software\\RegisteredApplications', '/v', 'PhotoSee']),
].join('\n----\n');
const userChoice = () => exts.map((e) => reg(['query', `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.${e}\\UserChoice`])).join('|');
const killTree = (pid) => { try { execFileSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore' }); } catch { /* gone */ } };

async function showsImage(exe, jpg) {
  const port = 9334;
  const env = { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` };
  delete env.PHOTOSEE_WEB; // force the installed layout, not a dev override
  const child = spawn(exe, [jpg], { stdio: 'ignore', env });
  try {
    let target;
    for (let i = 0; i < 100 && !target; i++) {
      await sleep(200);
      try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page' && t.url.startsWith('http://127.0.0.1')); } catch { /* not up */ }
    }
    if (!target) return { ok: false, why: 'no CDP page target (window never came up)' };
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((r) => (ws.onopen = r));
    let id = 0; const pending = new Map();
    ws.onmessage = (m) => { const d = JSON.parse(m.data); pending.get(d.id)?.(d); };
    const evalJs = (expression) => new Promise((r) => { pending.set(++id, r); ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true } })); }).then((d) => d.result.result.value);
    let info = null;
    for (let i = 0; i < 50 && !info; i++) {
      await sleep(200);
      info = await evalJs(`(() => { const i = document.querySelector('#stage img'); return i && i.complete && i.naturalWidth > 0 ? JSON.stringify({ w: i.naturalWidth, h: i.naturalHeight, src: i.src.split('?')[1]?.slice(0, 40) }) : null; })()`);
    }
    return info ? { ok: true, info: JSON.parse(info) } : { ok: false, why: 'page up but no decoded #stage img' };
  } finally { killTree(child.pid); }
}

rmSync(WORK, { recursive: true, force: true });
mkdirSync(WORK, { recursive: true });
const jpg = join(WORK, 'smoke.jpg');
ps(`Add-Type -AssemblyName System.Drawing; $b = New-Object Drawing.Bitmap 640,400; $g = [Drawing.Graphics]::FromImage($b); $g.Clear([Drawing.Color]::SteelBlue); $g.FillEllipse([Drawing.Brushes]::Orange, 200, 100, 240, 200); $b.Save('${jpg}', [Drawing.Imaging.ImageFormat]::Jpeg)`);

const before = snapshot();
const choiceBefore = userChoice();
const jpgDefault = reg(['query', 'HKCU\\Software\\Classes\\.jpg', '/ve']);
if (!jpgDefault) console.log('note: HKCU .jpg has no default value to compare (key absent)');
console.log(`installer: ${SETUP}\ninstall dir: ${INST}\n${exts.length} extensions\n`);
try {
  // /S silent, /D must be last and unquoted (NSIS rule).
  const t0 = Date.now();
  const r = spawn(SETUP, ['/S', `/D=${INST}`], { stdio: 'ignore' });
  const code = await new Promise((res) => r.on('exit', res));
  check('1 silent install exit code 0', code === 0, `code=${code} ${Date.now() - t0} ms`);
  const exe = join(INST, 'photosee.exe');
  check('2 photosee.exe installed', existsSync(exe));
  check('  web/index.html installed beside the exe', existsSync(join(INST, 'web', 'index.html')));
  check('  uninstall.exe present', existsSync(join(INST, 'uninstall.exe')));
  check('  installed under HKCU (no admin)', reg(['query', 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PhotoSee']).includes('PhotoSee'));

  const jpgKey = reg(['query', 'HKCU\\Software\\Classes\\.jpg', '/s']);
  check('3 .jpg lists PhotoSee.Image in OpenWithProgids', /OpenWithProgids[\s\S]*PhotoSee\.Image/.test(jpgKey));
  check('  .jpg default value untouched', reg(['query', 'HKCU\\Software\\Classes\\.jpg', '/ve']) === jpgDefault);
  check('  UserChoice untouched for all types', userChoice() === choiceBefore);
  const cmd = reg(['query', 'HKCU\\Software\\Classes\\PhotoSee.Image\\shell\\open\\command', '/ve']);
  check('  ProgId command points at installed exe', cmd.toLowerCase().includes(exe.toLowerCase()), cmd.trim().split('\n').pop());
  check('  Capabilities registered', reg(['query', 'HKCU\\Software\\RegisteredApplications', '/v', 'PhotoSee']).includes('Software\\PhotoSee\\Capabilities'));
  check('  every extension has its OpenWithProgids entry', exts.every((e) => /PhotoSee\.(Image|Raw)/.test(reg(['query', `HKCU\\Software\\Classes\\.${e}\\OpenWithProgids`]))));

  const shown = await showsImage(exe, jpg);
  check('4 installed exe opens the JPEG and the page decoded it', shown.ok, shown.ok ? JSON.stringify(shown.info) : shown.why);
} finally {
  // _?= keeps the uninstaller in place and synchronous (otherwise it re-launches itself from %TEMP% and returns at once).
  const un = join(INST, 'uninstall.exe');
  if (existsSync(un)) {
    const u = spawn(un, ['/S', `_?=${INST}`], { stdio: 'ignore' });
    const code = await new Promise((res) => u.on('exit', res));
    check('5 silent uninstall exit code 0', code === 0, `code=${code}`);
    await sleep(500);
    rmSync(un, { force: true });
  }
  const left = existsSync(INST) ? readdirSync(INST) : [];
  check('6 install dir empty/removed after uninstall', left.length === 0, left.join(','));
  check('  registry snapshot identical to pre-install', snapshot() === before);
  check('  Uninstall key gone', reg(['query', 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PhotoSee']) === '');
  rmSync(WORK, { recursive: true, force: true });
}
console.log(failed ? `\n${failed} check(s) FAILED` : '\nall checks passed');
process.exit(failed ? 1 : 0);
