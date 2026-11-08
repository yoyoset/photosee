// Auto-hide of the overlay: the idle clock, and the input that feeds it. Package J.
// Split out of overlay.ts to stay under the 300-line file limit.
import { CURSOR_HIDE_MS, OVERLAY_HIDE_MS, OVERLAY_REVEAL_ZONE_PX } from './constants';
import type { Ctx } from './ctx';
import type { ViewState } from './state';

/**
 * Pure idle tracker with injected time, so the hide logic is testable
 * without clocks. "Holds" (hover, keyboard focus, pointer in the reveal zone)
 * keep it shown no matter how long it has been idle.
 */
export class IdleTimer {
  private shown = true;
  private last = 0;
  private readonly holds = new Set<string>();

  constructor(private readonly ms: number, private readonly onChange: (shown: boolean) => void) {}

  get isShown(): boolean { return this.shown; }

  /** Any input: show now and restart the countdown. */
  wake(now: number): void {
    this.last = now;
    this.set(true);
  }

  hold(why: string, on: boolean, now: number): void {
    if (on) this.holds.add(why);
    else this.holds.delete(why);
    this.wake(now);
  }

  /** Drop a hold without counting it as input (the bar keeps its own clock). */
  release(why: string): void { this.holds.delete(why); }

  /** Take over a visibility someone else wrote to the store, without echoing it back. */
  adopt(shown: boolean, now: number): void {
    this.shown = shown;
    this.last = now;
  }

  tick(now: number): void {
    if (this.shown && this.holds.size === 0 && now - this.last >= this.ms) this.set(false);
  }

  /** Milliseconds until it would hide; null when nothing is pending. */
  remaining(now: number): number | null {
    if (!this.shown || this.holds.size > 0) return null;
    return Math.max(0, this.ms - (now - this.last));
  }

  private set(v: boolean): void {
    if (v === this.shown) return;
    this.shown = v;
    this.onChange(v);
  }
}

export function inRevealZone(y: number, viewportH: number, zone = OVERLAY_REVEAL_ZONE_PX): boolean {
  return y >= viewportH - zone;
}

/** Wires input to the two idle timers and schedules the next wake-up. */
export class AutoHide {
  private readonly bar: IdleTimer;
  private readonly cursor: IdleTimer;
  private handle: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly ctx: Ctx, private readonly root: HTMLElement,
    private readonly now: () => number = () => performance.now()) {
    const doc = root.ownerDocument;
    this.bar = new IdleTimer(OVERLAY_HIDE_MS, (v) => ctx.store.set({ overlayVisible: v }));
    this.cursor = new IdleTimer(CURSOR_HIDE_MS, (v) => doc.documentElement.classList.toggle('cursor-hidden', !v));
    this.cursor.hold('windowed', !ctx.store.get().fullscreen, this.now());
    this.bar.wake(this.now()); // start the countdown at mount, not at the first input
    this.arm();
  }

  sync(s: ViewState, prev: ViewState): void {
    if (s.overlayVisible !== this.bar.isShown) this.bar.adopt(s.overlayVisible, this.now());
    if (s.fullscreen !== prev.fullscreen) this.cursor.hold('windowed', !s.fullscreen, this.now());
    this.arm();
  }

  listen(bottom: HTMLElement): () => void {
    const doc = this.root.ownerDocument;
    const ac = new AbortController();
    const on = (t: EventTarget, type: string, fn: (e: Event) => void): void =>
      t.addEventListener(type, fn, { signal: ac.signal });
    const hold = (why: string, held: boolean): void => {
      this.bar.hold(why, held, this.now());
      this.arm();
    };
    const input = (e: Event): void => {
      const now = this.now();
      this.cursor.wake(now);
      if (this.quiet(e)) {
        this.bar.release('zone');
        this.arm();
        return;
      }
      this.bar.wake(now);
      // Parked in the reveal zone counts like hovering the bar: the bar must
      // not vanish from under a resting cursor.
      if (e.type.startsWith('pointer')) hold('zone', inRevealZone((e as PointerEvent).clientY, window.innerHeight));
      else this.arm();
    };
    for (const type of ['pointermove', 'pointerdown', 'keydown', 'wheel']) on(doc, type, input);
    // A mouse leaving the window drops its hold. Not input: a lifted finger
    // fires this too, right after every tap (iPad mini, 2026-11-08), and
    // waking here brought every bar up on each page turn.
    on(doc.documentElement, 'pointerleave', () => { this.bar.release('zone'); this.arm(); });
    on(bottom, 'pointerenter', () => hold('hover', true));
    on(bottom, 'pointerleave', () => hold('hover', false));
    // Only keyboard focus holds the bar: a button clicked with the mouse keeps
    // focus, and that must not pin the bar open forever.
    on(this.root, 'focusin', (e) => hold('focus', keyboardFocus(e.target as Element)));
    on(this.root, 'focusout', () => hold('focus', false));
    return () => {
      ac.abort();
      clearTimeout(this.handle);
      doc.documentElement.classList.remove('cursor-hidden');
    };
  }

  /**
   * Input that must not bring the bars up over the picture: a finger or pen
   * on the picture (a page-turn tap; the middle tap summons the bars on
   * purpose, gestures.ts), and in fullscreen also keys, the wheel and a mouse
   * outside the bottom reveal zone. Touching the bars themselves still counts.
   */
  private quiet(e: Event): boolean {
    const ptr = e.type.startsWith('pointer') ? (e as PointerEvent) : null;
    if (ptr && this.root.contains(e.target as Node)) return false;
    if (ptr && (ptr.pointerType === 'touch' || ptr.pointerType === 'pen')) return true;
    if (!this.ctx.store.get().fullscreen) return false;
    return ptr ? !inRevealZone(ptr.clientY, window.innerHeight) : true;
  }

  private arm(): void {
    clearTimeout(this.handle);
    const now = this.now();
    const waits = [this.bar.remaining(now), this.cursor.remaining(now)].filter((w) => w !== null);
    if (waits.length === 0) return;
    this.handle = setTimeout(() => {
      const at = this.now();
      this.bar.tick(at);
      this.cursor.tick(at);
      this.arm();
    }, Math.min(...waits));
  }
}

function keyboardFocus(target: Element): boolean {
  try {
    return target.matches(':focus-visible');
  } catch {
    return true; // engines without :focus-visible: err on the side of showing
  }
}
