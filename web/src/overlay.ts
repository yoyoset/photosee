// Auto-hiding overlay: title, buttons, filmstrip host. Package J.
import './overlay.css';
import type { Action } from './actions';
import type { ImageItem, Session } from './api';
import { OVERLAY_FADE_MS, TOUCH_TARGET_PX } from './constants';
import type { Ctx } from './ctx';
import { mountFilmstrip } from './filmstrip';
import { AutoHide } from './overlay-autohide';
import { ICONS } from './overlay-icons';
import { t, type Key } from './i18n';
import { topButtons } from './overlay-top';
import type { FitMode, ViewState } from './state';
export { IdleTimer, inRevealZone } from './overlay-autohide';

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

const FIT_LABEL: Record<FitMode, Key> = { contain: 'fit.contain', width: 'fit.width', height: 'fit.height' };
const FIT_ICON: Record<FitMode, string> = { contain: ICONS.fit, width: ICONS.fitW, height: ICONS.fitH };

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
    // Shows the fit in use; a press steps whole -> width -> height (zoomed:
    // back to fit). 1:1 is a double-click/tap or the 1 key.
    b('fit', { type: 'fit_cycle' }, fit ? FIT_LABEL[s.fit] : 'btn.fit', FIT_ICON[s.fit], 'W'),
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
    const [back, info] = topButtons(ctx);
    top.append(back, title, info);
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
