// Auto-hiding overlay: title, buttons, filmstrip host. Package J.
import './overlay.css';
import type { Action } from './actions';
import type { ImageItem, Session } from './api';
import {
  CURSOR_HIDE_MS, OVERLAY_FADE_MS, OVERLAY_HIDE_MS, OVERLAY_REVEAL_ZONE_PX, TOUCH_TARGET_PX,
} from './constants';
import type { Ctx } from './ctx';
import { mountFilmstrip } from './filmstrip';
import { t, type Key } from './i18n';
import type { ViewState } from './state';

/**
 * Pure idle tracker with injected time, so the hide logic is testable
 * without clocks. "Holds" (hover, keyboard focus, pointer in the reveal zone)
 * keep it shown no matter how long it has been idle.
 */
export class IdleTimer {
  private shown = true;
  private last = 0;
  private readonly holds = new Set<string>();

  constructor(private readonly ms: number, private readonly onChange: (shown: boolean) => void) {}

  get isShown(): boolean { return this.shown; }

  /** Any input: show now and restart the countdown. */
  wake(now: number): void {
    this.last = now;
    this.set(true);
  }

  hold(why: string, on: boolean, now: number): void {
    if (on) this.holds.add(why);
    else this.holds.delete(why);
    this.wake(now);
  }

  /** Take over a visibility someone else wrote to the store, without echoing it back. */
  adopt(shown: boolean, now: number): void {
    this.shown = shown;
    this.last = now;
  }

  tick(now: number): void {
    if (this.shown && this.holds.size === 0 && now - this.last >= this.ms) this.set(false);
  }

  /** Milliseconds until it would hide; null when nothing is pending. */
  remaining(now: number): number | null {
    if (!this.shown || this.holds.size > 0) return null;
    return Math.max(0, this.ms - (now - this.last));
  }

  private set(v: boolean): void {
    if (v === this.shown) return;
    this.shown = v;
    this.onChange(v);
  }
}

export function inRevealZone(y: number, viewportH: number, zone = OVERLAY_REVEAL_ZONE_PX): boolean {
  return y >= viewportH - zone;
}

/** Last path segment; accepts both separators because listings keep the host's own. */
export function folderName(dir: string): string {
  const parts = dir.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? dir;
}

/**
 * Splits at the last dot so CSS can ellipsize only the stem and keep the
 * extension visible. A leading dot (".hidden") or a very long "extension" is
 * part of the stem.
 */
export function splitName(name: string): { stem: string; ext: string } {
  const dot = name.lastIndexOf('.');
  if (dot <= 0 || name.length - dot > 8) return { stem: name, ext: '' };
  return { stem: name.slice(0, dot), ext: name.slice(dot) };
}

export function titleParts(item: ImageItem, index: number, total: number, dir: string) {
  return { ...splitName(item.name), pos: `${index + 1} / ${total}`, folder: folderName(dir) };
}

const SVG = (body: string): string =>
  `<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" ` +
  `stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

const ICONS = {
  prev: SVG('<path d="M15 5l-7 7 7 7"/>'),
  next: SVG('<path d="M9 5l7 7-7 7"/>'),
  play: SVG('<path d="M8 5v14l11-7z" fill="currentColor"/>'),
  pause: SVG('<path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="currentColor"/>'),
  ccw: SVG('<path d="M3 4v6h6M5.1 15a8 8 0 1 0 1.9-8.3L3 10"/>'),
  cw: SVG('<path d="M21 4v6h-6M18.9 15a8 8 0 1 1-1.9-8.3L21 10"/>'),
  fit: SVG('<path d="M3 6h18v12H3zM8 10h8v4H8z"/>'),
  actual: SVG('<path d="M3 6h18v12H3z"/><text x="12" y="15.5" font-size="8" text-anchor="middle" fill="currentColor" stroke="none">1:1</text>'),
  fullscreen: SVG('<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>'),
  settings: SVG('<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>'),
};

interface BtnView { id: string; action: Action; label: Key; icon: string; shortcut: string }

/** The eight buttons, in bar order, for the given state. Pure. */
export function buttonViews(s: ViewState, platform: Session['platform']): BtnView[] {
  const playing = s.slideshow === 'playing';
  const fit = s.zoom.mode === 'fit';
  const b = (id: string, action: Action, label: Key, icon: string, shortcut = ''): BtnView =>
    ({ id, action, label, icon, shortcut });
  return [
    b('prev', { type: 'prev' }, 'btn.prev', ICONS.prev, '←'),
    b('play', { type: 'toggle_slideshow' }, playing ? 'btn.pause' : 'btn.play',
      playing ? ICONS.pause : ICONS.play, 'S'),
    b('next', { type: 'next' }, 'btn.next', ICONS.next, '→'),
    b('ccw', { type: 'rotate', quarterTurns: -1 }, 'btn.rotate_ccw', ICONS.ccw, 'Shift+R'),
    b('cw', { type: 'rotate', quarterTurns: 1 }, 'btn.rotate_cw', ICONS.cw, 'R'),
    // One button, two meanings: it offers whichever view the user is not in.
    b('fit', { type: 'zoom_toggle' }, fit ? 'btn.actual' : 'btn.fit',
      fit ? ICONS.actual : ICONS.fit, fit ? '1' : '0'),
    // F works everywhere (the browser build has no F11); the second key is the
    // native one, which differs by platform.
    b('fullscreen', { type: 'toggle_fullscreen' }, 'btn.fullscreen', ICONS.fullscreen,
      platform === 'macos' ? 'F / ⌃⌘F' : 'F / F11'),
    b('settings', { type: 'open_settings' }, 'btn.settings', ICONS.settings),
  ];
}

/** `label (shortcut)`, or just the label when the action has no key. */
export function tooltip(v: BtnView): string {
  const label = t(v.label);
  return v.shortcut ? `${label} (${v.shortcut})` : label;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  e.className = cls;
  return e;
}

/** Wires input to the two idle timers and schedules the next wake-up. */
class AutoHide {
  private readonly bar: IdleTimer;
  private readonly cursor: IdleTimer;
  private handle: ReturnType<typeof setTimeout> | undefined;

  constructor(ctx: Ctx, private readonly root: HTMLElement,
    private readonly now: () => number = () => performance.now()) {
    const doc = root.ownerDocument;
    this.bar = new IdleTimer(OVERLAY_HIDE_MS, (v) => ctx.store.set({ overlayVisible: v }));
    this.cursor = new IdleTimer(CURSOR_HIDE_MS, (v) => doc.documentElement.classList.toggle('cursor-hidden', !v));
    this.cursor.hold('windowed', !ctx.store.get().fullscreen, this.now());
    this.bar.wake(this.now()); // start the countdown at mount, not at the first input
    this.arm();
  }

  sync(s: ViewState, prev: ViewState): void {
    if (s.overlayVisible !== this.bar.isShown) this.bar.adopt(s.overlayVisible, this.now());
    if (s.fullscreen !== prev.fullscreen) this.cursor.hold('windowed', !s.fullscreen, this.now());
    this.arm();
  }

  listen(bottom: HTMLElement): () => void {
    const doc = this.root.ownerDocument;
    const ac = new AbortController();
    const on = (t: EventTarget, type: string, fn: (e: Event) => void): void =>
      t.addEventListener(type, fn, { signal: ac.signal });
    const hold = (why: string, held: boolean): void => {
      this.bar.hold(why, held, this.now());
      this.arm();
    };
    const input = (e: Event): void => {
      const now = this.now();
      this.cursor.wake(now);
      this.bar.wake(now);
      // Parked in the reveal zone counts like hovering the bar: the bar must
      // not vanish from under a resting cursor.
      if (e.type.startsWith('pointer')) hold('zone', inRevealZone((e as PointerEvent).clientY, window.innerHeight));
      else this.arm();
    };
    for (const type of ['pointermove', 'pointerdown', 'keydown', 'wheel']) on(doc, type, input);
    on(doc.documentElement, 'pointerleave', () => hold('zone', false));
    on(bottom, 'pointerenter', () => hold('hover', true));
    on(bottom, 'pointerleave', () => hold('hover', false));
    // Only keyboard focus holds the bar: a button clicked with the mouse keeps
    // focus, and that must not pin the bar open forever.
    on(this.root, 'focusin', (e) => hold('focus', keyboardFocus(e.target as Element)));
    on(this.root, 'focusout', () => hold('focus', false));
    return () => {
      ac.abort();
      clearTimeout(this.handle);
      doc.documentElement.classList.remove('cursor-hidden');
    };
  }

  private arm(): void {
    clearTimeout(this.handle);
    const now = this.now();
    const waits = [this.bar.remaining(now), this.cursor.remaining(now)].filter((w) => w !== null);
    if (waits.length === 0) return;
    this.handle = setTimeout(() => {
      const at = this.now();
      this.bar.tick(at);
      this.cursor.tick(at);
      this.arm();
    }, Math.min(...waits));
  }
}

function keyboardFocus(target: Element): boolean {
  try {
    return target.matches(':focus-visible');
  } catch {
    return true; // engines without :focus-visible: err on the side of showing
  }
}

/** Builds the overlay DOM once and re-renders only text and attributes. */
class OverlayView {
  readonly bottom = el('div', 'ov-bottom');
  private readonly bar = el('div', 'ov-bar');
  private readonly parts: Record<'stem' | 'ext' | 'pos' | 'folder', HTMLElement>;
  private readonly buttons: HTMLButtonElement[] = [];
  private views: BtnView[] = [];
  private readonly live: HTMLElement | null;
  readonly unFilm: () => void;

  constructor(private readonly root: HTMLElement, private readonly ctx: Ctx) {
    const top = el('div', 'ov-top');
    const title = el('div', 'ov-title');
    this.parts = {
      stem: el('span', 'ov-stem'), ext: el('span', 'ov-ext'),
      pos: el('span', 'ov-pos'), folder: el('span', 'ov-folder'),
    };
    title.append(...Object.values(this.parts));
    top.append(title);
    this.unFilm = mountFilmstrip(this.bottom, ctx);
    for (const v of buttonViews(ctx.store.get(), ctx.session.platform)) {
      const btn = el('button', 'ov-btn');
      btn.type = 'button';
      btn.dataset['id'] = v.id;
      this.buttons.push(btn);
      this.bar.append(btn);
    }
    this.bar.addEventListener('click', (e) => {
      const btn = (e.target as Element).closest<HTMLButtonElement>('.ov-btn');
      const view = this.views[this.buttons.indexOf(btn as HTMLButtonElement)];
      if (view) ctx.dispatch(view.action);
    });
    this.bottom.append(this.bar);
    root.append(top, this.bottom);
    this.live = root.ownerDocument.getElementById('live');
  }

  render(s: ViewState, prev: ViewState | null): void {
    this.root.classList.toggle('ov-hidden', !s.overlayVisible);
    this.views = buttonViews(s, this.ctx.session.platform);
    this.views.forEach((v, i) => {
      const btn = this.buttons[i];
      if (!btn) return;
      btn.setAttribute('aria-label', t(v.label));
      btn.title = tooltip(v);
      // Icons are constants defined above, never data from a file.
      if (btn.dataset['icon'] !== v.icon) { btn.innerHTML = v.icon; btn.dataset['icon'] = v.icon; }
    });
    if (!prev || s.index !== prev.index || s.listing !== prev.listing) this.renderTitle(s);
  }

  private renderTitle(s: ViewState): void {
    const item = this.ctx.store.current();
    const total = s.listing?.images.length ?? 0;
    const p = item ? titleParts(item, s.index, total, s.listing?.dir ?? '') : null;
    const text = { stem: p?.stem, ext: p?.ext, pos: p && ` · ${p.pos}`, folder: p && ` · ${p.folder}` };
    for (const k of Object.keys(text) as Array<keyof typeof text>) this.parts[k].textContent = text[k] ?? '';
    if (this.live && item) {
      this.live.textContent = t('live.position', { n: s.index + 1, total, name: item.name });
    }
  }
}

export function mountOverlay(root: HTMLElement, ctx: Ctx): () => void {
  root.classList.add('ov');
  root.style.setProperty('--ov-fade', `${OVERLAY_FADE_MS}ms`);
  root.style.setProperty('--touch', `${TOUCH_TARGET_PX}px`);
  const view = new OverlayView(root, ctx);
  const auto = new AutoHide(ctx, root);
  const unListen = auto.listen(view.bottom);
  const unsub = ctx.store.subscribe((s, prev) => { view.render(s, prev); auto.sync(s, prev); });
  view.render(ctx.store.get(), null);

  return () => {
    unsub();
    unListen();
    view.unFilm();
    root.replaceChildren();
    root.classList.remove('ov', 'ov-hidden');
  };
}
