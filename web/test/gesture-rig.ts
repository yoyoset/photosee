// Shared DOM rig for the gesture tests: a 1000x600 stage bound to a
// recording dispatch. `el`, `store` and `seen` are live bindings: mount()
// replaces them, and the test files read them through the import.
import type { Action } from '../src/actions';
import type { Api, ImageSource, Session } from '../src/api';
import type { Ctx } from '../src/ctx';
import { bindGestures } from '../src/gestures';
import { EscStack, Store, type ViewState } from '../src/state';

export const W = 1000;
export const H = 600;
export let el: HTMLElement;
export let store: Store;
export let seen: Action[];
export let off: () => void;

/** `overflows`: the fitted image is bigger than the stage (fit width).
 * `apply`: also run each action against the store, as the controller would. */
export function mount(patch: Partial<ViewState> = {}, overflows = false, apply?: (a: Action) => void): void {
  seen = [];
  store = new Store();
  store.set(patch);
  const ctx: Ctx = {
    store, esc: new EscStack(), dispatch: (a) => { seen.push(a); apply?.(a); },
    api: {} as Api, src: {} as ImageSource, session: {} as Session,
  };
  el = document.createElement('div');
  el.getBoundingClientRect = () => ({ left: 0, top: 0, width: W, height: H, right: W, bottom: H, x: 0, y: 0, toJSON: () => ({}) });
  document.body.append(el);
  off = bindGestures(el, ctx, () => overflows);
}

export const zoomedState: Partial<ViewState> = { zoom: { mode: 'actual', scale: 1, x: 0, y: 0 } };

export function ptr(type: string, x: number, y: number, t: number, o: { id?: number; kind?: string; button?: number } = {}): PointerEvent {
  const e = new PointerEvent(type, {
    clientX: x, clientY: y, pointerId: o.id ?? 1, pointerType: o.kind ?? 'touch',
    button: o.button ?? 0, bubbles: true, cancelable: true,
  });
  Object.defineProperty(e, 'timeStamp', { value: t });
  el.dispatchEvent(e);
  return e;
}

export function drag(pts: [number, number, number][], kind = 'touch', id = 1): void {
  pts.forEach(([x, y, t], i) => ptr(i === 0 ? 'pointerdown' : 'pointermove', x, y, t, { kind, id }));
  const [x, y, t] = pts[pts.length - 1] as [number, number, number];
  ptr('pointerup', x, y, t, { kind, id });
}

export function wheel(o: WheelEventInit, t: number): WheelEvent {
  const e = new WheelEvent('wheel', { bubbles: true, cancelable: true, clientX: 300, clientY: 200, ...o });
  Object.defineProperty(e, 'timeStamp', { value: t });
  el.dispatchEvent(e);
  return e;
}
