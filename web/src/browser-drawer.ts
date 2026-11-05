// On a narrow window (a phone) the folder tree is a drawer over the files,
// not a column beside them: at 375 px the 220 px column left a single
// thumbnail column (measured on the Docker build, 2026-11-05).
//
// The drawer's open/closed state lives here, for this page only. It is NOT
// the persisted `tree.visible`: ui-state is stored on the server and shared
// by every device, so a phone closing its drawer must not hide the tree on
// the desktop, and a desktop with the tree shown must not open the drawer
// over the phone's files on every visit.

import { NARROW_MAX_PX } from './browser-consts';

export const isNarrow = (winW: number): boolean => winW <= NARROW_MAX_PX;

export interface TreeLayout { tree: boolean; split: boolean; scrim: boolean }

/** What shows, given the window width, the drawer and the persisted pref. */
export function treeLayout(narrow: boolean, drawerOpen: boolean, visiblePref: boolean): TreeLayout {
  if (narrow) return { tree: drawerOpen, split: false, scrim: drawerOpen };
  return { tree: visiblePref, split: visiblePref, scrim: false };
}

export class TreeDrawer {
  readonly scrim = document.createElement('div');
  private open = false;
  private narrow = false;
  /** Set by the owner: re-applies the layout and the toolbar state. */
  onChange: () => void = () => {};

  constructor(readonly tree$: HTMLElement, readonly split: HTMLElement) {
    this.scrim.className = 'br-scrim';
    this.scrim.hidden = true;
    this.scrim.addEventListener('click', () => this.close());
  }

  get isNarrow(): boolean { return this.narrow; }

  /** Applies the layout; returns whether the tree is showing. */
  apply(root: HTMLElement, winW: number, visiblePref: boolean): boolean {
    this.narrow = isNarrow(winW);
    if (!this.narrow) this.open = false; // a phone turned to landscape starts closed when it narrows again
    root.classList.toggle('narrow', this.narrow);
    const l = treeLayout(this.narrow, this.open, visiblePref);
    this.tree$.hidden = !l.tree;
    this.split.hidden = !l.split;
    this.scrim.hidden = !l.scrim;
    return l.tree;
  }

  toggle(): void {
    this.open = !this.open;
    this.onChange();
  }

  /** After a folder was picked in the tree: get out of the way of its files. */
  close(): void {
    if (!this.open) return;
    this.open = false;
    this.onChange();
  }
}

export function onWindowResize(f: () => void): () => void {
  window.addEventListener('resize', f);
  return () => window.removeEventListener('resize', f);
}
