// User intents. Input modules (keys, gestures, overlay buttons) only emit
// these; controller.ts turns them into state changes. Contract file.

export type Action =
  | { type: 'next' } | { type: 'prev' } | { type: 'first' } | { type: 'last' }
  | { type: 'goto'; index: number }
  | { type: 'next_folder' } | { type: 'prev_folder' }
  | { type: 'toggle_fullscreen' }
  | { type: 'toggle_slideshow' }
  | { type: 'rotate'; quarterTurns: 1 | -1 }
  | { type: 'zoom_fit' } | { type: 'zoom_actual' } | { type: 'zoom_toggle' }
  | { type: 'zoom_by'; factor: number; anchor?: { x: number; y: number } }
  | { type: 'pan_by'; dx: number; dy: number }
  | { type: 'toggle_info' } | { type: 'toggle_help' } | { type: 'open_settings' }
  | { type: 'escape' }
  /** Arrow key held (true) or released (false); see KEY_REPEAT_SETTLE_MS. */
  | { type: 'scrub'; active: boolean };
