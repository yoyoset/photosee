// Typed client for docs/api.md. Contract file: orchestrator only.
// The UI builds image URLs only through `ImageSource`, so mobile can later swap
// the HTTP transport for a Tauri custom protocol without touching the viewer.

export type Format =
  | 'jpeg' | 'png' | 'gif' | 'webp' | 'avif' | 'bmp' | 'ico' | 'svg'
  | 'tiff' | 'jxl' | 'psd' | 'heic'
  | { raw: 'cr2' | 'cr3' | 'nef' | 'arw' | 'dng' | 'orf' | 'rw2' | 'raf' };

export interface ImageItem {
  name: string;
  path: string;
  fmt: Format;
  size: number;
  mtime_ms: number;
}

export interface DirItem {
  name: string;
  path: string;
}

export interface Listing {
  dir: string;
  parent: string | null;
  images: ImageItem[];
  dirs: DirItem[];
}

export interface Session {
  api_version: number;
  mode: 'desktop' | 'server';
  platform: 'windows' | 'macos' | 'linux' | 'android' | 'ios';
  start: { dir: string; file: string | null } | null;
  roots: DirItem[];
}

export interface ExifSummary {
  camera: string | null;
  lens: string | null;
  iso: number | null;
  exposure: string | null;
  f_number: number | null;
  focal_mm: number | null;
  taken: string | null;
}

export interface Meta {
  fmt: Format;
  /** Display size, after EXIF orientation. */
  width: number;
  height: number;
  orientation: string;
  animated: boolean;
  size: number;
  mtime_ms: number;
  exif: ExifSummary;
}

export interface Settings {
  interval_s: number;
  on_end: 'loop' | 'next_folder' | 'stop';
  shuffle: boolean;
  transition: 'none' | 'fade';
  start_fullscreen: boolean;
  wheel: 'navigate' | 'zoom';
}

export type ErrorCode =
  | 'not_found' | 'forbidden' | 'unsupported' | 'missing_codec'
  | 'corrupt' | 'too_large' | 'timeout' | 'io' | 'network';

export class ApiError extends Error {
  constructor(
    readonly code: ErrorCode,
    readonly detail: string,
    readonly codec: 'heif_ext' | 'hevc_ext' | 'libheif' | null = null,
  ) {
    super(`${code}: ${detail}`);
  }
}

export type Priority = 'current' | 'prefetch';

/** How image bytes are addressed. `w`/`h` are device pixels; omit = original. */
export interface ImageSource {
  imageUrl(path: string, box?: { w: number; h: number }, prio?: Priority): string;
  thumbUrl(path: string, edge: number): string;
}

const q = encodeURIComponent;

export class HttpImageSource implements ImageSource {
  constructor(private readonly base = '/api/v1') {}

  imageUrl(path: string, box?: { w: number; h: number }, prio: Priority = 'current'): string {
    const size = box ? `&w=${Math.round(box.w)}&h=${Math.round(box.h)}` : '';
    return `${this.base}/img?p=${q(path)}${size}&prio=${prio}`;
  }

  thumbUrl(path: string, edge: number): string {
    return `${this.base}/thumb?p=${q(path)}&s=${edge}`;
  }
}

async function call<T>(url: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(url, { credentials: 'same-origin', ...init });
  } catch (e) {
    throw new ApiError('network', String(e));
  }
  if (!res.ok) {
    const body = (await res.json().catch(() => ({}))) as {
      code?: ErrorCode; message?: string; codec?: ApiError['codec'];
    };
    throw new ApiError(body.code ?? 'io', body.message ?? `HTTP ${res.status}`, body.codec ?? null);
  }
  return (await res.json()) as T;
}

export class Api {
  constructor(private readonly base = '/api/v1') {}

  session(): Promise<Session> {
    return call(`${this.base}/session`);
  }
  list(dir: string): Promise<Listing> {
    return call(`${this.base}/list?p=${q(dir)}`);
  }
  meta(path: string): Promise<Meta> {
    return call(`${this.base}/meta?p=${q(path)}`);
  }
  async nav(dir: string, root: string, d: 'next' | 'prev'): Promise<string | null> {
    const r = await call<{ dir: string | null }>(
      `${this.base}/nav?p=${q(dir)}&root=${q(root)}&d=${d}`,
    );
    return r.dir;
  }
  config(): Promise<Settings> {
    return call(`${this.base}/config`);
  }
  saveConfig(s: Settings): Promise<Settings> {
    return call(`${this.base}/config`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(s),
    });
  }
}
