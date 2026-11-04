// The browser's persisted layout and history (docs/api.md GET/PUT /ui-state).
// The server only stores the object, so it is ours to validate: anything
// missing or of the wrong type falls back to a default, and fields we do not
// know are carried through untouched so a newer build's data survives.

import type { Api } from './api';
import {
  LIST_MAX, RECENT_MAX, SAVE_DEBOUNCE_MS, THUMB_DEFAULT_PX, THUMB_MAX_PX, THUMB_MIN_PX,
  TREE_DEFAULT_PX, TREE_MIN_PX, TREE_STORED_MAX_PX,
} from './browser-consts';
import { samePath } from './browser-path';
import { SORT_KEYS, type SortDir, type SortKey } from './browser-sort';

export type ViewMode = 'grid' | 'details' | 'single';

export interface UiState {
  version: 1;
  tree: { visible: boolean; width: number };
  view: ViewMode;
  sort: { key: SortKey; dir: SortDir };
  thumb: number;
  names: boolean;
  recent: string[];
  network: string[];
  last: string | null;
  /** Stop adding opened folders to Recent (existing entries stay). */
  recentPaused: boolean;
  [unknown: string]: unknown;
}

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);
const clampInt = (v: unknown, lo: number, hi: number, d: number): number =>
  typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, Math.round(v))) : d;
const strings = (v: unknown, max: number): string[] =>
  Array.isArray(v) ? v.filter((s): s is string => typeof s === 'string' && s !== '').slice(0, max) : [];

export function defaultUiState(): UiState {
  return {
    version: 1, tree: { visible: true, width: TREE_DEFAULT_PX }, view: 'grid',
    sort: { key: 'name', dir: 'asc' }, thumb: THUMB_DEFAULT_PX, names: true,
    recent: [], network: [], last: null, recentPaused: false,
  };
}

export function parseUiState(raw: unknown): UiState {
  if (!isObj(raw) || raw['version'] !== 1) return defaultUiState();
  const tree = isObj(raw['tree']) ? raw['tree'] : {};
  const sort = isObj(raw['sort']) ? raw['sort'] : {};
  const last = raw['last'];
  return {
    ...raw,
    version: 1,
    tree: { ...tree, visible: tree['visible'] !== false, width: clampInt(tree['width'], TREE_MIN_PX, TREE_STORED_MAX_PX, TREE_DEFAULT_PX) },
    view: raw['view'] === 'details' || raw['view'] === 'single' ? raw['view'] : 'grid',
    sort: {
      ...sort,
      key: SORT_KEYS.find((k) => k === sort['key']) ?? 'name',
      dir: sort['dir'] === 'desc' ? 'desc' : 'asc',
    },
    thumb: clampInt(raw['thumb'], THUMB_MIN_PX, THUMB_MAX_PX, THUMB_DEFAULT_PX),
    names: raw['names'] !== false,
    recent: strings(raw['recent'], RECENT_MAX),
    network: strings(raw['network'], LIST_MAX),
    last: typeof last === 'string' && last !== '' ? last : null,
    recentPaused: raw['recentPaused'] === true,
  };
}

/** Newest first, no duplicates (by path), capped. */
export function pushRecent(list: readonly string[], path: string): string[] {
  return [path, ...list.filter((p) => !samePath(p, path))].slice(0, RECENT_MAX);
}

export const removePath = (list: readonly string[], path: string): string[] =>
  list.filter((p) => !samePath(p, path));

/** Loads defensively: a failing or missing endpoint just means defaults. */
export async function loadUiState(api: Pick<Api, 'uiState'>): Promise<UiState> {
  try {
    return parseUiState(await api.uiState());
  } catch (e) {
    console.warn('ui-state unavailable, using defaults', e);
    return defaultUiState();
  }
}

type Listener = (s: UiState) => void;

/** The live copy of the state; every change schedules one debounced save. */
export class UiPrefs {
  private listeners = new Set<Listener>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** What the server already has, so opening the same folder again writes nothing. */
  private saved: string;

  constructor(
    private state: UiState,
    private readonly api: Pick<Api, 'saveUiState'> | null,
  ) {
    this.saved = JSON.stringify(state);
  }

  get(): UiState { return this.state; }

  set(patch: Partial<UiState>): void {
    this.state = { ...this.state, ...patch };
    for (const l of this.listeners) l(this.state);
    this.schedule();
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  private schedule(): void {
    if (!this.api) return;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), SAVE_DEBOUNCE_MS);
  }

  /** Save now (also used on shutdown). Losing layout is not worth a dialog. */
  async flush(): Promise<void> {
    clearTimeout(this.timer);
    this.timer = undefined;
    const json = JSON.stringify(this.state);
    if (json === this.saved) return;
    try {
      this.saved = json;
      await this.api?.saveUiState(this.state);
    } catch (e) {
      this.saved = ''; // retry on the next change
      console.warn('ui-state not saved', e);
    }
  }

  dispose(): void {
    clearTimeout(this.timer);
    this.listeners.clear();
  }
}
