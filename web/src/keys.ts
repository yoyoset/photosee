// Keyboard map (docs/ux-spec.md §keys). Package K.

import type { Action } from './actions';
import type { Session } from './api';

export interface KeyInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

/** Pure mapping; macOS uses Cmd where others use Ctrl. */
export function keyAction(e: KeyInput, platform: Session['platform']): Action | null {
  void e; void platform;
  return null; // package K
}

export function bindKeys(
  target: Window, platform: Session['platform'], dispatch: (a: Action) => void,
): () => void {
  void target; void platform; void dispatch;
  return () => {}; // package K
}
