// Address bar: clickable breadcrumbs that turn into a text field for typing
// any path (UNC included). A path that fails shows its reason right under
// the field; the user keeps what they typed and nothing is modal.

import { breadcrumbs } from './browser-path';
import { icon } from './browser-icons';
import { t } from './i18n';

export interface AddressHooks {
  /** Open `path`. `typed`: the user typed it, so a failure is returned as
  * text for the inline message (null = opened, or not worth reporting). */
  go(path: string, typed: boolean): Promise<string | null>;
}

export class AddressBar {
  readonly el = document.createElement('div');
  private readonly crumbs = document.createElement('div');
  private readonly err = document.createElement('div');
  private input: HTMLInputElement | null = null;
  private path = '';

  constructor(private readonly hooks: AddressHooks) {
    this.el.className = 'addr';
    this.crumbs.className = 'addr-crumbs';
    this.err.className = 'addr-err';
    this.err.setAttribute('role', 'alert');
    this.err.hidden = true;
    this.el.append(this.crumbs, this.err);
    // Anything that is not a crumb button is "empty space" and starts editing.
    this.crumbs.addEventListener('click', (e) => {
      const b = (e.target as Element).closest<HTMLElement>('.crumb');
      if (b) void this.hooks.go(b.dataset['path'] ?? '', false);
      else this.edit();
    });
  }

  get editing(): boolean { return this.input !== null; }

  /** Show `path` as breadcrumbs (unless the user is typing). */
  set(path: string | null): void {
    this.path = path ?? '';
    if (this.input) return;
    this.crumbs.replaceChildren();
    const parts = breadcrumbs(this.path);
    parts.forEach((c, i) => {
      if (i > 0) this.crumbs.append(icon('chevron'));
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'crumb';
      b.textContent = c.name;
      b.dataset['path'] = c.path;
      b.tabIndex = -1; // Ctrl+L is the keyboard way in; tabbing through them is noise
      this.crumbs.append(b);
    });
    this.crumbs.scrollLeft = this.crumbs.scrollWidth;
  }

  edit(): void {
    if (this.input) return this.input.select();
    const input = document.createElement('input');
    input.className = 'addr-in';
    input.type = 'text';
    input.spellcheck = false;
    input.autocomplete = 'off';
    input.value = this.path;
    input.setAttribute('aria-label', t('br.address'));
    input.addEventListener('keydown', (e) => this.onKey(e, input));
    input.addEventListener('input', () => this.showError(null));
    input.addEventListener('blur', () => this.stop());
    this.input = input;
    this.crumbs.replaceChildren(input);
    input.focus();
    input.select();
  }

  private onKey(e: KeyboardEvent, input: HTMLInputElement): void {
    // Typing must not reach the grid's type-ahead or the viewer's keys.
    e.stopPropagation();
    if (e.key === 'Escape') { e.preventDefault(); this.stop(); }
    if (e.key !== 'Enter' || e.isComposing) return;
    e.preventDefault();
    const value = input.value.trim();
    if (value === '' || value === this.path) return this.stop();
    void this.hooks.go(value, true).then((msg) => {
      if (this.input !== input) return;
      if (msg) this.showError(msg);
      else this.stop();
    });
  }

  private showError(msg: string | null): void {
    this.err.hidden = msg === null;
    this.err.textContent = msg ?? '';
    this.input?.classList.toggle('bad', msg !== null);
  }

  private stop(): void {
    if (!this.input) return;
    const input = this.input;
    this.input = null; // before remove(): blur fires during it
    input.remove();
    this.showError(null);
    this.set(this.path);
  }
}
