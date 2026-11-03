import { describe, expect, it } from 'vitest';
import { systemLang, t } from '../src/i18n';

describe('i18n', () => {
  it('follows the system language', () => {
    expect(systemLang('zh-CN')).toBe('zh');
    expect(systemLang('zh-Hant-TW')).toBe('zh');
    expect(systemLang('en-US')).toBe('en');
    expect(systemLang('fr')).toBe('en');
  });

  it('fills placeholders and keeps unknown ones visible', () => {
    expect(t('live.position', { n: 23, total: 412, name: 'a.jpg' }, 'en')).toBe('23 of 412, a.jpg');
    expect(t('hint.last', {}, 'zh')).toContain('{key}');
  });
});
