// Settings panel: exactly the six options in docs/ux-spec.md. Package L.
// Not a seventh option without a non-goals discussion (docs/non-goals.md).

import { ApiError, type Settings } from './api';
import { SLIDESHOW_INTERVAL_MAX_S as MAX, SLIDESHOW_INTERVAL_MIN_S as MIN } from './constants';
import type { Ctx } from './ctx';
import { el, errorSentence } from './errors';
import { Modal, type PanelHandle } from './help';
import { t, type Key } from './i18n';

/** Debounce for the save call (docs/packages/L.md); not in constants.ts. */
const SAVE_DEBOUNCE_MS = 300;

// Only used if the panel opens before settings were loaded (should not happen
// after boot). The server's defaults win as soon as the first save returns.
const FALLBACK: Settings = {
  interval_s: 5, on_end: 'next_folder', shuffle: false,
  transition: 'none', start_fullscreen: true, wheel: 'navigate',
};

const CHOICES = {
  on_end: ['loop', 'next_folder', 'stop'],
  transition: ['none', 'fade'],
  wheel: ['navigate', 'zoom'],
} as const;
type ChoiceKey = keyof typeof CHOICES;
type CheckKey = 'shuffle' | 'start_fullscreen';

function input(type: string, name: string): HTMLInputElement {
  const i = el('input');
  i.type = type;
  if (name) i.name = name;
  return i;
}

function radioGroup(key: ChoiceKey): HTMLFieldSetElement {
  const set = el('fieldset');
  set.append(el('legend', '', t(`settings.${key}`)));
  for (const v of CHOICES[key]) {
    const radio = input('radio', key);
    radio.value = v;
    // The cross product of key and value includes invalid pairs; the i18n
    // test in settings.test.ts checks every pair we actually render exists.
    const label = el('label', '', t(`settings.${key}.${v}` as Key));
    label.prepend(radio);
    set.append(label);
  }
  return set;
}

function checkbox(key: CheckKey): HTMLLabelElement {
  const label = el('label', '', t(`settings.${key}`));
  label.prepend(input('checkbox', key));
  return label;
}

class SettingsForm {
  readonly el = el('form', 'ps-settings-form');
  private readonly num = input('number', 'interval_s');
  private readonly range = input('range', '');
  private readonly err = el('p', 'ps-error');
  private draft: Settings = { ...FALLBACK };
  private timer: ReturnType<typeof setTimeout> | null = null;
  private seq = 0;
  /** The last save failed: keep the user's values instead of reloading. */
  private failed = false;

  constructor(private readonly ctx: Ctx, private readonly onFail: () => void) {
    this.el.noValidate = true;
    this.el.addEventListener('submit', (e) => e.preventDefault());
    this.el.append(
      this.intervalRow(), radioGroup('on_end'), checkbox('shuffle'),
      radioGroup('transition'), checkbox('start_fullscreen'), radioGroup('wheel'), this.err,
    );
    this.el.addEventListener('input', this.onEvent);
    this.el.addEventListener('change', this.onEvent);
  }

  private intervalRow(): HTMLElement {
    const row = el('div', 'ps-interval');
    const text = t('settings.interval');
    this.num.id = 'ps-interval-num';
    this.range.setAttribute('aria-label', text);
    for (const i of [this.num, this.range]) {
      i.min = String(MIN);
      i.max = String(MAX);
      i.step = '1';
    }
    const label = el('label', 'ps-row-label', text);
    label.htmlFor = this.num.id;
    row.append(label, this.range, this.num);
    return row;
  }

  /** Populate from the store each time the panel opens. */
  load(): void {
    if (this.failed) return;
    this.draft = { ...(this.ctx.store.get().settings ?? FALLBACK) };
    this.fill(this.draft);
    this.err.textContent = '';
  }

  private fill(s: Settings): void {
    this.num.value = this.range.value = String(s.interval_s);
    this.num.removeAttribute('aria-invalid');
    for (const r of this.el.querySelectorAll<HTMLInputElement>('input[type=radio]')) {
      r.checked = r.value === String(s[r.name as ChoiceKey]);
    }
    for (const c of this.el.querySelectorAll<HTMLInputElement>('input[type=checkbox]')) {
      c.checked = Boolean(s[c.name as CheckKey]);
    }
  }

  private readonly onEvent = (e: Event): void => {
    if (e.target === this.range) this.num.value = this.range.value;
    if (e.type === 'change' && e.target === this.num) {
      // Commit (blur/enter): an out-of-range number is clamped, not rejected.
      const n = Math.round(Number(this.num.value));
      const ok = this.num.value !== '' && Number.isFinite(n);
      this.num.value = String(ok ? Math.min(MAX, Math.max(MIN, n)) : this.draft.interval_s);
    }
    const n = this.num.value === '' ? NaN : Number(this.num.value);
    const valid = Number.isInteger(n) && n >= MIN && n <= MAX;
    // While typing, an invalid interval is flagged but left on screen; the
    // other settings still save with the last good interval.
    this.num.setAttribute('aria-invalid', String(!valid));
    if (valid) this.range.value = String(n);
    const next = this.read(valid ? n : this.draft.interval_s);
    // input+change both fire for one click, and an invalid number changes
    // nothing: only a real difference starts a save.
    if ((Object.keys(next) as (keyof Settings)[]).every((k) => next[k] === this.draft[k])) return;
    this.draft = next;
    this.schedule();
  };

  private read(interval: number): Settings {
    const pick = (k: ChoiceKey): string =>
      this.el.querySelector<HTMLInputElement>(`input[name="${k}"]:checked`)?.value ?? '';
    const box = (k: CheckKey): boolean =>
      this.el.querySelector<HTMLInputElement>(`input[name="${k}"]`)?.checked ?? false;
    return {
      interval_s: interval,
      on_end: pick('on_end') as Settings['on_end'],
      transition: pick('transition') as Settings['transition'],
      wheel: pick('wheel') as Settings['wheel'],
      shuffle: box('shuffle'),
      start_fullscreen: box('start_fullscreen'),
    };
  }

  private schedule(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.save(), SAVE_DEBOUNCE_MS);
  }

  /** Closing the panel must not drop a change still inside the debounce. */
  flush(): void {
    if (this.timer === null) return;
    clearTimeout(this.timer);
    void this.save();
  }

  private async save(): Promise<void> {
    this.timer = null;
    const mine = ++this.seq;
    try {
      const norm = await this.ctx.api.saveConfig({ ...this.draft });
      if (mine !== this.seq) return; // a newer save is in flight; it decides
      this.ctx.store.set({ settings: norm });
      this.err.textContent = '';
      this.failed = false;
      // Do not yank a half-typed number out from under the user.
      const typing = this.num.getAttribute('aria-invalid') === 'true';
      if (this.timer === null && !typing) this.fill((this.draft = { ...norm }));
    } catch (e) {
      if (mine !== this.seq) return;
      // Never "saved" in the UI when it was not: show why, keep the user's value.
      const apiErr = e instanceof ApiError ? e : new ApiError('io', String(e));
      this.err.textContent = `${t('settings.save_failed')}: ${errorSentence(apiErr)}`;
      this.err.title = apiErr.detail;
      this.failed = true;
      this.onFail();
    }
  }
}

export function mountSettings(root: HTMLElement, ctx: Ctx): PanelHandle {
  // A save that fails after the panel was closed must not go unnoticed:
  // reopen it so the inline error and the unsaved values are in front of the user.
  const form = new SettingsForm(ctx, () => modal.open());
  const modal = new Modal(root, ctx, 'settings.title', 'ps-settings', {
    onOpen: () => form.load(),
    onClose: () => form.flush(),
  });
  // The only way to the help panel without a keyboard (? on a phone).
  const help = el('button', 'ps-help-link', t('help.open'));
  help.type = 'button';
  help.addEventListener('click', () => ctx.dispatch({ type: 'toggle_help' }));
  modal.body.append(form.el, help);
  return modal;
}
