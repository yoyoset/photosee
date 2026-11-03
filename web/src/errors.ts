// Error card, NAS banner, toast and notices (+ two tiny DOM helpers the other
// package-L modules share). Package L. Rules: docs/ux-spec.md §Errors.
//
// Layering for the viewer/overlay packages: `.ps-errcard` is z-index 1 and
// ignores pointer events outside the card itself, so the overlay (package J)
// only needs z-index >= 10 to stay on top of it and clickable.

import type { ApiError, Session } from './api';
import { HINT_TOAST_MS } from './constants';
import { t } from './i18n';
import './panels.css';

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K, cls = '', text?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

// TO VERIFY in Wave 3 (docs/deps.md notes the store IDs were taken from public
// listings, never opened on a real machine): HEIF Image Extensions
// 9PMMSR1CGPWG, HEVC Video Extensions 9NMZLZ57R3T7. Also confirm the webview
// is allowed to launch the `ms-windows-store:` scheme from an <a href>.
const STORE_URL = {
  heif_ext: 'ms-windows-store://pdp/?ProductId=9PMMSR1CGPWG',
  hevc_ext: 'ms-windows-store://pdp/?ProductId=9NMZLZ57R3T7',
} as const;

/** Only transient failures are worth a retry button; a corrupt file stays corrupt. */
const RETRYABLE: ReadonlySet<ApiError['code']> = new Set(['timeout', 'io', 'network']);

/** The localized one-line sentence for an API error (never the server's text). */
export function errorSentence(err: ApiError): string {
  if (err.code === 'missing_codec') {
    return err.codec ? t(`err.missing_codec.${err.codec}`) : t('err.unsupported');
  }
  return t(`err.${err.code}`);
}

function actionFor(err: ApiError, onRetry?: () => void): HTMLElement | null {
  if (err.code === 'missing_codec' && err.codec && err.codec !== 'libheif') {
    const a = el('a', 'ps-btn', t('action.install'));
    a.href = STORE_URL[err.codec];
    a.rel = 'noopener';
    return a;
  }
  if (onRetry && RETRYABLE.has(err.code)) {
    const b = el('button', 'ps-btn', t('action.retry'));
    b.type = 'button';
    b.addEventListener('click', onRetry);
    return b;
  }
  return null;
}

const dismissers = new WeakMap<Element, () => void>();

function removeByClass(host: HTMLElement, cls: string): void {
  for (const c of Array.from(host.children)) {
    if (c.classList.contains(cls)) (dismissers.get(c) ?? (() => c.remove()))();
  }
}

/**
 * Replaces the image with a card: file name, sentence, one action, and the
 * technical detail collapsed. Returns a function that removes the card.
 * `onRetry` is shown only for transient errors (timeout / io / network).
 */
export function renderErrorCard(
  host: HTMLElement, err: ApiError, fileName: string, onRetry?: () => void,
): () => void {
  removeByClass(host, 'ps-errcard');
  const card = el('section', 'ps-errcard');
  card.setAttribute('role', 'alert');
  card.append(el('h2', 'ps-errcard-name', fileName), el('p', 'ps-errcard-msg', errorSentence(err)));
  const action = actionFor(err, onRetry);
  if (action) card.append(action);
  if (err.detail) {
    const d = el('details', 'ps-errcard-detail');
    d.append(el('summary', '', t('action.details')), el('pre', '', err.detail));
    card.append(d);
  }
  host.append(card);
  const remove = (): void => card.remove();
  dismissers.set(card, remove);
  return remove;
}

/** Different folders may be unreachable at once: banners stack in one column. */
function bannerStack(host: HTMLElement): HTMLElement {
  const found = Array.from(host.children).find((c) => c.classList.contains('ps-banners'));
  if (found instanceof HTMLElement) return found;
  const stack = el('div', 'ps-banners');
  host.append(stack);
  return stack;
}

/**
 * Top banner "cannot reach {dir} ({reason})" with a retry button. One banner
 * per `dir`: calling again updates it in place (and rebinds `retry`) instead
 * of stacking. The caller dismisses it when a retry succeeds.
 */
export function showNasBanner(
  host: HTMLElement, dir: string, retry: () => void, reason: 'timeout' | 'io' = 'io',
): () => void {
  const stack = bannerStack(host);
  let banner = Array.from(stack.children).find(
    (c): c is HTMLElement => c instanceof HTMLElement && c.dataset.dir === dir,
  );
  if (!banner) {
    banner = el('div', 'ps-banner');
    banner.dataset.dir = dir;
    banner.setAttribute('role', 'alert');
    stack.append(banner);
  }
  const node = banner;
  const text = t('banner.unreachable', { dir, reason: t(reason === 'timeout' ? 'reason.timeout' : 'reason.io') });
  const retryBtn = el('button', 'ps-btn', t('action.retry'));
  const closeBtn = el('button', 'ps-close', '×');
  retryBtn.type = closeBtn.type = 'button';
  closeBtn.setAttribute('aria-label', t('action.close'));
  retryBtn.addEventListener('click', retry);
  const dismiss = (): void => {
    node.remove();
    if (!stack.hasChildNodes()) stack.remove();
  };
  closeBtn.addEventListener('click', dismiss);
  node.replaceChildren(el('span', 'ps-banner-text', text), retryBtn, closeBtn);
  return dismiss;
}

const timers = new WeakMap<Element, ReturnType<typeof setTimeout>>();

/** One element per (host, class); a newer message replaces the older one. */
function place(host: HTMLElement, cls: string, text: string, ms: number): () => void {
  removeByClass(host, cls);
  const e = el('div', cls, text);
  e.setAttribute('role', 'status'); // implicit aria-live=polite
  host.append(e);
  const remove = (): void => {
    clearTimeout(timers.get(e));
    e.remove();
  };
  dismissers.set(e, remove);
  if (ms > 0) timers.set(e, setTimeout(remove, ms));
  return remove;
}

export function showToast(host: HTMLElement, text: string, ms: number): () => void {
  return place(host, 'ps-toast', text, ms);
}

/** The key that jumps to the next folder, as written for this platform. */
export function nextFolderKey(platform: Session['platform']): string {
  return platform === 'macos' ? '⌘→' : 'Ctrl+→';
}

/** Stays until dismissed (the folder is still empty); replaces, never stacks. */
export function showEmptyFolder(host: HTMLElement, platform: Session['platform']): () => void {
  return place(host, 'ps-notice', t('hint.empty_folder', { key: nextFolderKey(platform) }), 0);
}

export function showLastHint(host: HTMLElement, platform: Session['platform']): () => void {
  return showToast(host, t('hint.last', { key: nextFolderKey(platform) }), HINT_TOAST_MS);
}
