// UI strings, zh and en; language follows the system. Contract file: every
// UI package uses these keys, so the table is owned by the orchestrator.
// Add a key here (both languages) before using it; `en` is typed against the
// `zh` keys, so tsc fails on a key that exists in only one language.

export type Lang = 'zh' | 'en';

export function systemLang(navLang: string): Lang {
  return navLang.toLowerCase().startsWith('zh') ? 'zh' : 'en';
}

const zh = {
  'btn.prev': '上一张',
  'btn.next': '下一张',
  'btn.play': '播放幻灯片',
  'btn.pause': '暂停幻灯片',
  'btn.rotate_ccw': '向左旋转',
  'btn.rotate_cw': '向右旋转',
  'btn.fit': '适应窗口',
  'btn.actual': '实际大小',
  'btn.fullscreen': '全屏',
  'btn.settings': '设置',
  'live.position': '{n} / {total}，{name}',
  'hint.first': '已是第一张',
  'hint.last': '已是最后一张 · {key} 下一个文件夹',
  'hint.empty_folder': '此文件夹没有图片 · {key} 下一个文件夹',
  'hint.no_next_folder': '没有下一个文件夹了',
  'hint.browser_fullscreen': '浏览器版：按 F 进入全屏，Esc 和 F11 由浏览器处理',
  'slideshow.ended': '幻灯片已结束',
  'banner.unreachable': '无法访问 {dir}（{reason}）',
  'reason.timeout': '超时',
  'reason.io': '连接失败',
  'err.not_found': '文件不存在或已被移动',
  'err.forbidden': '没有访问权限',
  'err.unsupported': '不支持这种格式',
  'err.missing_codec.heif_ext': '需要安装 Microsoft「HEIF 图像扩展」',
  'err.missing_codec.hevc_ext': '需要安装 Microsoft「HEVC 视频扩展」',
  'err.missing_codec.libheif': '这个版本还不能显示 HEIC',
  'err.corrupt': '文件已损坏或不完整',
  'err.too_large': '图片太大（超过 2 亿像素）',
  'err.timeout': '读取超时：网络存储没有响应',
  'err.io': '读取失败',
  'err.network': '连接不到看图服务',
  'action.install': '去 Microsoft Store 安装',
  'action.retry': '重试',
  'action.details': '技术细节',
  'action.close': '关闭',
  'settings.title': '设置',
  'settings.interval': '幻灯片间隔（秒）',
  'settings.on_end': '播放到文件夹末尾时',
  'settings.on_end.loop': '从头循环',
  'settings.on_end.next_folder': '进入下一个文件夹',
  'settings.on_end.stop': '停止',
  'settings.shuffle': '随机顺序（仅当前文件夹）',
  'settings.transition': '切换效果',
  'settings.transition.none': '无',
  'settings.transition.fade': '淡入淡出',
  'settings.start_fullscreen': '启动时全屏',
  'settings.wheel': '滚轮',
  'settings.wheel.navigate': '翻页',
  'settings.wheel.zoom': '缩放',
  'help.title': '快捷键',
  'info.camera': '相机',
  'info.lens': '镜头',
  'info.exposure': '快门',
  'info.f_number': '光圈',
  'info.iso': 'ISO',
  'info.focal': '焦距',
  'info.taken': '拍摄时间',
  'info.dimensions': '尺寸',
  'info.file_size': '文件大小',
} as const;

export type Key = keyof typeof zh;

const en: Record<Key, string> = {
  'btn.prev': 'Previous',
  'btn.next': 'Next',
  'btn.play': 'Play slideshow',
  'btn.pause': 'Pause slideshow',
  'btn.rotate_ccw': 'Rotate left',
  'btn.rotate_cw': 'Rotate right',
  'btn.fit': 'Fit to window',
  'btn.actual': 'Actual size',
  'btn.fullscreen': 'Fullscreen',
  'btn.settings': 'Settings',
  'live.position': '{n} of {total}, {name}',
  'hint.first': 'First image',
  'hint.last': 'Last image · {key} for the next folder',
  'hint.empty_folder': 'No images in this folder · {key} for the next folder',
  'hint.no_next_folder': 'No more folders',
  'hint.browser_fullscreen': 'Browser version: press F for fullscreen; Esc and F11 belong to the browser',
  'slideshow.ended': 'Slideshow finished',
  'banner.unreachable': 'Cannot reach {dir} ({reason})',
  'reason.timeout': 'timed out',
  'reason.io': 'connection failed',
  'err.not_found': 'The file no longer exists or was moved',
  'err.forbidden': 'Access denied',
  'err.unsupported': 'This format is not supported',
  'err.missing_codec.heif_ext': 'Needs the Microsoft "HEIF Image Extensions"',
  'err.missing_codec.hevc_ext': 'Needs the Microsoft "HEVC Video Extensions"',
  'err.missing_codec.libheif': 'HEIC is not supported in this version yet',
  'err.corrupt': 'The file is damaged or incomplete',
  'err.too_large': 'The image is too large (over 200 megapixels)',
  'err.timeout': 'Timed out: the network storage is not answering',
  'err.io': 'Could not read the file',
  'err.network': 'Cannot connect to the viewer service',
  'action.install': 'Install from the Microsoft Store',
  'action.retry': 'Retry',
  'action.details': 'Technical details',
  'action.close': 'Close',
  'settings.title': 'Settings',
  'settings.interval': 'Slideshow interval (seconds)',
  'settings.on_end': 'At the end of a folder',
  'settings.on_end.loop': 'Start over',
  'settings.on_end.next_folder': 'Go to the next folder',
  'settings.on_end.stop': 'Stop',
  'settings.shuffle': 'Shuffle (current folder only)',
  'settings.transition': 'Transition',
  'settings.transition.none': 'None',
  'settings.transition.fade': 'Fade',
  'settings.start_fullscreen': 'Start in fullscreen',
  'settings.wheel': 'Mouse wheel',
  'settings.wheel.navigate': 'Turns pages',
  'settings.wheel.zoom': 'Zooms',
  'help.title': 'Keyboard shortcuts',
  'info.camera': 'Camera',
  'info.lens': 'Lens',
  'info.exposure': 'Shutter',
  'info.f_number': 'Aperture',
  'info.iso': 'ISO',
  'info.focal': 'Focal length',
  'info.taken': 'Taken',
  'info.dimensions': 'Dimensions',
  'info.file_size': 'File size',
};

const tables: Record<Lang, Record<Key, string>> = { zh, en };

let current: Lang = typeof navigator === 'undefined' ? 'en' : systemLang(navigator.language);

/** For tests and for a future host override; the UI never offers a choice. */
export function setLang(l: Lang): void {
  current = l;
}

/** `{name}` placeholders are replaced from `vars`; unknown ones stay visible
 * so a missing variable shows up in review instead of vanishing. */
export function t(key: Key, vars?: Record<string, string | number>, lang: Lang = current): string {
  const s = tables[lang][key];
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}
