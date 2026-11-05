// Login screen of the multi-user server: tap your name, type your PIN.
// Shown by boot when /session answers 401; resolves once the server has set
// the session cookie, and boot then starts over as that user.
//
// Names come from the server (GET /users) as buttons, so nobody types a
// name on a phone. The PIN field is `inputmode=numeric` (digit pad) and
// `type=password` (not shown over a shoulder on the shared PC).

import { ApiError, type Api } from './api';
import { t } from './i18n';
import './login.css';

const el = <K extends keyof HTMLElementTagNameMap>(doc: Document, tag: K, cls: string, text = ''): HTMLElementTagNameMap[K] =>
  Object.assign(doc.createElement(tag), { className: cls, textContent: text });

export function showLogin(doc: Document, api: Api, names: readonly string[]): Promise<void> {
  const root = el(doc, 'div', 'lg');
  root.setAttribute('role', 'dialog');
  root.setAttribute('aria-label', t('login.who'));
  doc.body.append(root);
  return new Promise((done) => {
    const pick = (): void => {
      const list = el(doc, 'div', 'lg-names');
      for (const n of names) {
        const b = el(doc, 'button', 'lg-name', n);
        b.type = 'button';
        b.addEventListener('click', () => pinFor(n));
        list.append(b);
      }
      root.replaceChildren(el(doc, 'h1', 'lg-title', t('login.who')), list);
      (list.firstElementChild as HTMLElement | null)?.focus();
    };
    const pinFor = (name: string): void => {
      const form = el(doc, 'form', 'lg-form');
      const pin = el(doc, 'input', 'lg-pin');
      Object.assign(pin, { type: 'password', inputMode: 'numeric', autocomplete: 'off', required: true, maxLength: 12 });
      pin.setAttribute('aria-label', t('login.pin'));
      pin.placeholder = t('login.pin');
      const msg = el(doc, 'p', 'lg-msg');
      msg.setAttribute('role', 'alert');
      const go = el(doc, 'button', 'lg-go', t('login.go'));
      go.type = 'submit';
      const other = el(doc, 'button', 'lg-other', t('login.other'));
      other.type = 'button';
      other.addEventListener('click', pick);
      form.addEventListener('submit', (e) => {
        e.preventDefault();
        go.disabled = true;
        api.login(name, pin.value).then(() => { root.remove(); done(); }, (err: unknown) => {
          go.disabled = false;
          pin.value = '';
          pin.focus();
          msg.textContent = loginError(err);
        });
      });
      form.append(pin, go, msg, other);
      root.replaceChildren(el(doc, 'h1', 'lg-title', name), form);
      pin.focus();
    };
    pick();
  });
}

/** The server's lockout message starts with "too many"; anything that is not
 * a 401 (network, server down) shows the plain error text instead. */
export function loginError(err: unknown): string {
  if (!(err instanceof ApiError)) return String(err);
  if (err.code !== 'unauthorized') return t(`err.${err.code === 'network' ? 'network' : 'io'}`);
  return err.detail.startsWith('too many') ? t('login.locked') : t('login.wrong');
}
