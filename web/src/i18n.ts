// UI strings, zh and en; language follows the system. Package L.

export type Lang = 'zh' | 'en';

export function systemLang(navLang: string): Lang {
  return navLang.toLowerCase().startsWith('zh') ? 'zh' : 'en';
}

export function t(key: string, lang: Lang): string {
  void lang;
  return key; // package L
}
