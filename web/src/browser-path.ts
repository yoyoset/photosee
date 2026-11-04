// Path text handling for the address bar and the tree. The server hands out
// logical paths in the host's own style ("C:\Pics", "\\nas\share\x",
// "/photos/a"), and the user types them the same way, so all comparisons go
// through `normPath` and no code splits on one separator by assumption.

export interface Crumb { name: string; path: string }

const DRIVE = /^[A-Za-z]:/;

export const isUnc = (p: string): boolean => p.startsWith('\\\\') || p.startsWith('//');
/** Windows-style paths compare case-insensitively. */
const isWindows = (p: string): boolean => DRIVE.test(p) || isUnc(p);

export function sepOf(p: string): '\\' | '/' {
  return isWindows(p) || p.includes('\\') ? '\\' : '/';
}

/** One canonical spelling for equality and prefix tests: '/' separators, no
 * trailing separator (except a bare root), lower-cased for Windows paths. */
export function normPath(p: string): string {
  let n = p.trim().replaceAll('\\', '/');
  const unc = n.startsWith('//');
  n = n.replace(/\/{2,}/g, '/');
  if (unc) n = `/${n}`;
  if (n.length > 1 && n.endsWith('/') && !/^[A-Za-z]:\/$/.test(n)) n = n.slice(0, -1);
  return isWindows(p) ? n.toLowerCase() : n;
}

export const samePath = (a: string, b: string): boolean => normPath(a) === normPath(b);

/** True when `dir` is `root` or lies below it. */
export function isUnder(root: string, dir: string): boolean {
  const r = normPath(root);
  const d = normPath(dir);
  if (d === r) return true;
  return d.startsWith(r.endsWith('/') ? r : `${r}/`);
}

/** Segments from a path's root to the path itself, each clickable. A UNC
 * path starts at "\\host\share" (the host alone is not a folder); a POSIX
 * "/" is not offered because the server normally forbids it. */
export function breadcrumbs(path: string): Crumb[] {
  const sep = sepOf(path);
  const unc = isUnc(path);
  const parts = path.split(/[\\/]+/).filter((s) => s !== '');
  const out: Crumb[] = [];
  let acc = '';
  parts.forEach((name, i) => {
    if (unc && i === 0) { acc = `${sep}${sep}${name}`; return; }
    if (unc && i === 1) {
      acc = `${acc}${sep}${name}`;
      out.push({ name: acc, path: acc });
      return;
    }
    acc = i === 0 && DRIVE.test(name) ? `${name}${sep}` : `${acc}${acc.endsWith(sep) ? '' : sep}${name}`;
    out.push({ name, path: acc });
  });
  return out;
}

/** Paths from `root` down to `dir`, inclusive; empty when `dir` is outside. */
export function ancestorChain(root: string, dir: string): string[] {
  if (!isUnder(root, dir)) return [];
  const crumbs = breadcrumbs(dir);
  const at = crumbs.findIndex((c) => samePath(c.path, root));
  if (at < 0) return samePath(root, dir) ? [root] : [root, dir];
  return crumbs.slice(at).map((c) => c.path);
}
