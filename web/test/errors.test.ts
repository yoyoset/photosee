import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError, type ErrorCode } from '../src/api';
import {
  errorSentence, renderErrorCard, showEmptyFolder, showLastHint, showNasBanner, showToast,
} from '../src/errors';
import { setLang, t } from '../src/i18n';

const CODES: ErrorCode[] = [
  'not_found', 'forbidden', 'unsupported', 'missing_codec', 'corrupt', 'too_large', 'timeout', 'io', 'network',
];
const NAS = '\\\\nas\\photos';

let host: HTMLElement;
beforeEach(() => {
  setLang('en');
  document.body.replaceChildren();
  host = document.body.appendChild(document.createElement('div'));
});

describe('error card', () => {
  it('renders a non-empty sentence for every ErrorCode', () => {
    for (const code of CODES) {
      const codec = code === 'missing_codec' ? 'libheif' : null;
      const s = errorSentence(new ApiError(code, 'd', codec));
      expect(s, code).toBeTruthy();
      expect(s, code).not.toContain('undefined');
    }
    for (const codec of ['heif_ext', 'hevc_ext', 'libheif'] as const) {
      expect(errorSentence(new ApiError('missing_codec', 'd', codec))).toBe(t(`err.missing_codec.${codec}`));
    }
  });

  it('shows file name, sentence and collapsed technical detail', () => {
    renderErrorCard(host, new ApiError('corrupt', 'bad SOS marker'), 'a<b>.jpg');
    expect(host.querySelector('.ps-errcard-name')?.textContent).toBe('a<b>.jpg');
    expect(host.querySelector('.ps-errcard-name')?.children.length).toBe(0); // text, not HTML
    expect(host.querySelector('.ps-errcard-msg')?.textContent).toBe(t('err.corrupt'));
    const d = host.querySelector('details');
    expect(d?.open).toBe(false);
    expect(d?.textContent).toContain('bad SOS marker');
    expect(host.querySelector('a, button')).toBeNull(); // corrupt: nothing to click
  });

  it('links each missing codec to its Store page; libheif gets no link', () => {
    renderErrorCard(host, new ApiError('missing_codec', 'x', 'heif_ext'), 'a.heic');
    expect(host.querySelector('a')?.getAttribute('href')).toBe('ms-windows-store://pdp/?ProductId=9PMMSR1CGPWG');
    renderErrorCard(host, new ApiError('missing_codec', 'x', 'hevc_ext'), 'a.heic');
    expect(host.querySelectorAll('.ps-errcard').length).toBe(1); // replaced
    expect(host.querySelector('a')?.getAttribute('href')).toBe('ms-windows-store://pdp/?ProductId=9NMZLZ57R3T7');
    renderErrorCard(host, new ApiError('missing_codec', 'x', 'libheif'), 'a.heic');
    expect(host.querySelector('a')).toBeNull();
  });

  it('offers retry only for transient errors', () => {
    const retry = vi.fn();
    renderErrorCard(host, new ApiError('timeout', ''), 'a.jpg', retry);
    host.querySelector('button')?.click();
    expect(retry).toHaveBeenCalledOnce();
    renderErrorCard(host, new ApiError('corrupt', ''), 'a.jpg', retry);
    expect(host.querySelector('button')).toBeNull();
  });

  it('returns a remover', () => {
    const remove = renderErrorCard(host, new ApiError('io', ''), 'a.jpg');
    remove();
    expect(host.querySelector('.ps-errcard')).toBeNull();
  });
});

describe('NAS banner', () => {
  it('does not stack for the same dir, updates the reason and rebinds retry', () => {
    const a = vi.fn();
    const b = vi.fn();
    showNasBanner(host, NAS, a, 'timeout');
    showNasBanner(host, NAS, b, 'io');
    expect(host.querySelectorAll('.ps-banner').length).toBe(1);
    expect(host.textContent).toContain(t('reason.io'));
    host.querySelector<HTMLButtonElement>('.ps-banner .ps-btn')?.click();
    expect(a).not.toHaveBeenCalled();
    expect(b).toHaveBeenCalledOnce();
  });

  it('shows dir and reason, stacks different dirs, and dismisses', () => {
    const dismiss = showNasBanner(host, NAS, () => {}, 'timeout');
    showNasBanner(host, '\\\\nas\\b', () => {});
    expect(host.querySelectorAll('.ps-banner').length).toBe(2);
    expect(host.textContent).toContain(t('banner.unreachable', { dir: NAS, reason: t('reason.timeout') }));
    dismiss();
    expect(host.querySelectorAll('.ps-banner').length).toBe(1);
  });

  it('the close button dismisses it', () => {
    showNasBanner(host, 'd', () => {});
    host.querySelector<HTMLButtonElement>('.ps-close')?.click();
    expect(host.querySelector('.ps-banner')).toBeNull();
  });
});

describe('toast and hints', () => {
  it('a toast expires and a newer one replaces it', () => {
    vi.useFakeTimers();
    showToast(host, 'one', 1000);
    showToast(host, 'two', 1000);
    expect(host.querySelectorAll('.ps-toast').length).toBe(1);
    expect(host.textContent).toBe('two');
    vi.advanceTimersByTime(1000);
    expect(host.querySelector('.ps-toast')).toBeNull();
    vi.useRealTimers();
  });

  it('empty-folder notice and last hint name no key: phones have none', () => {
    showEmptyFolder(host);
    expect(host.textContent).toBe(t('hint.empty_folder'));
    showEmptyFolder(host);
    expect(host.querySelectorAll('.ps-notice').length).toBe(1);
    showLastHint(host);
    expect(host.querySelector('.ps-toast')?.textContent).toBe(t('hint.last'));
    expect(host.textContent).not.toMatch(/Ctrl|⌘/);
  });
});
