// Strings for the fit modes and the touch half of the help panel (phones
// have no keyboard, 2026-11-06). Split from i18n.ts for its size limit;
// spread into both tables there, so tsc still checks both languages.

export const ZH_TOUCH = {
  'fit.contain': '适应：整张',
  'fit.width': '适应：宽度',
  'fit.height': '适应：高度',
  'help.fit_cycle': '切换适应：整张 / 宽度 / 高度',
  'help.keys': '键盘',
  'help.touch': '触屏',
  'help.open': '操作说明',
  'touch.tap_left': '点左侧',
  'touch.tap_right': '点右侧',
  'touch.tap_mid': '点中间',
  'touch.bars': '显示 / 隐藏工具栏',
  'touch.again_last': '最后一张再点右侧',
  'touch.again_first': '第一张再点左侧',
  'touch.pinch': '双指捏合',
  'touch.double': '双击',
  'touch.pan': '移动图片（放大后，或超出屏幕时）',
  'touch.drag': '拖动',
  'touch.pull': '下拉',
  'touch.info_btn': '右上角 ⓘ',
} as const;

export const EN_TOUCH: Record<keyof typeof ZH_TOUCH, string> = {
  'fit.contain': 'Fit: whole image',
  'fit.width': 'Fit: width',
  'fit.height': 'Fit: height',
  'help.fit_cycle': 'Switch fit: whole / width / height',
  'help.keys': 'Keyboard',
  'help.touch': 'Touch',
  'help.open': 'Controls',
  'touch.tap_left': 'Tap the left side',
  'touch.tap_right': 'Tap the right side',
  'touch.tap_mid': 'Tap the middle',
  'touch.bars': 'Show / hide the bars',
  'touch.again_last': 'On the last image, tap right again',
  'touch.again_first': 'On the first image, tap left again',
  'touch.pinch': 'Pinch',
  'touch.double': 'Double-tap',
  'touch.pan': 'Move the image (zoomed, or bigger than the screen)',
  'touch.drag': 'Drag',
  'touch.pull': 'Pull down',
  'touch.info_btn': 'ⓘ top right',
};
