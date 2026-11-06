// Structural gates that compilers and linters don't cover.
//   node scripts/gates.mjs                      size limits + deps.md + bundle
//   node scripts/gates.mjs --ownership WP BASE  files changed since BASE ⊆ WP
// Exit code 1 on any violation; every violation is printed.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { gzipSync } from 'node:zlib';

const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const MAX_LINES = 300;
const MAX_BUNDLE_GZ = 60 * 1024;
const SOURCE_DIRS = ['core', 'server', 'app', 'web/src', 'web/test', 'tools', 'spikes'];
const SKIP = new Set(['node_modules', 'target', 'dist', 'gen', '.git']);

const problems = [];
const rel = (p) => relative(ROOT, p).replaceAll('\\', '/');

function* walk(dir) {
  if (!existsSync(dir)) return;
  for (const name of readdirSync(dir)) {
    if (SKIP.has(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else yield p;
  }
}

function checkFileLengths() {
  for (const d of SOURCE_DIRS) {
    for (const f of walk(join(ROOT, d))) {
      if (!/\.(rs|ts|js|mjs|css)$/.test(f)) continue;
      // A final newline ends the last line rather than starting a new one;
      // eslint's max-lines counts the same way.
      const n = readFileSync(f, 'utf8').replace(/\n$/, '').split('\n').length;
      if (n > MAX_LINES) problems.push(`${rel(f)}: ${n} lines > ${MAX_LINES}`);
    }
  }
}

function checkDepsDocumented() {
  const doc = readFileSync(join(ROOT, 'docs/deps.md'), 'utf8');
  const documented = new Set([...doc.matchAll(/^\|\s*([@\w./-]+(?:,\s*[@\w./-]+)*)\s*\|/gm)]
    .flatMap((m) => m[1].split(/,\s*/)));
  const cargo = readFileSync(join(ROOT, 'Cargo.toml'), 'utf8');
  const section = cargo.split('[workspace.dependencies]')[1]?.split(/^\[/m)[0] ?? '';
  const crates = [...section.matchAll(/^([\w-]+)\s*=/gm)].map((m) => m[1]);
  const pkg = JSON.parse(readFileSync(join(ROOT, 'web/package.json'), 'utf8'));
  const npm = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies });
  for (const d of [...crates, ...npm]) {
    if (!documented.has(d)) problems.push(`dependency "${d}" has no row in docs/deps.md`);
  }
}

function checkBundle() {
  const assets = join(ROOT, 'web/dist/assets');
  if (!existsSync(assets)) return;
  let total = 0;
  for (const f of walk(assets)) if (/\.(js|css)$/.test(f)) total += gzipSync(readFileSync(f)).length;
  if (total > MAX_BUNDLE_GZ) problems.push(`web bundle ${total} B gzip > ${MAX_BUNDLE_GZ}`);
  console.log(`web bundle: ${total} B gzip`);
}

function parseOwnership() {
  const text = readFileSync(join(ROOT, 'docs/ownership.toml'), 'utf8');
  const map = {};
  for (const block of text.split(/^\[/m).slice(1)) {
    const name = block.slice(0, block.indexOf(']'));
    map[name] = [...block.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  }
  return map;
}

function globToRegex(g) {
  const esc = g.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`^${esc.replace(/\*\*/g, '\u0000').replace(/\*/g, '[^/]*').replace(/\u0000/g, '.*')}$`);
}

function checkOwnership(wp, base) {
  const own = parseOwnership();
  if (!own[wp]) return problems.push(`unknown work package "${wp}"`);
  const allowed = own[wp].map(globToRegex);
  const out = execFileSync('git', ['diff', '--name-only', `${base}...HEAD`], { cwd: ROOT, encoding: 'utf8' });
  for (const f of out.split('\n').filter(Boolean)) {
    if (!allowed.some((r) => r.test(f))) problems.push(`${wp} may not change ${f}`);
  }
}

/** One product version everywhere (user, 2026-11-06): the installer and zip
 * take it from tauri.conf.json, the binaries and /session from Cargo.toml,
 * npm from package.json and its lockfile. tools/ and spikes/ are not shipped
 * and keep their own. */
function checkVersions() {
  const json = (p) => JSON.parse(readFileSync(join(ROOT, p), 'utf8'));
  const cargo = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(readFileSync(join(ROOT, 'Cargo.toml'), 'utf8'))?.[1];
  const lock = json('web/package-lock.json');
  const found = {
    'Cargo.toml': cargo,
    'app/tauri.conf.json': json('app/tauri.conf.json').version,
    'web/package.json': json('web/package.json').version,
    'web/package-lock.json': lock.version,
    'web/package-lock.json packages[""]': lock.packages?.['']?.version,
  };
  if (new Set(Object.values(found)).size !== 1) {
    problems.push(`versions differ: ${Object.entries(found).map(([k, v]) => `${k}=${v}`).join(', ')}`);
  }
}

const args = process.argv.slice(2);
if (args[0] === '--ownership') {
  checkOwnership(args[1], args[2] ?? 'main');
} else {
  checkFileLengths();
  checkDepsDocumented();
  checkBundle();
  checkVersions();
}
if (problems.length) {
  for (const p of problems) console.error(`GATE: ${p}`);
  process.exit(1);
}
console.log('gates: ok');
