// Slideshow timer and controls around the pure state machine in
// slideshow.ts (package B). Package L.
//
// Who calls what (the controller and viewer wire this in Wave 2):
//   controller -> toggle()/start()/pause()/resume()/stop()   (toggle_slideshow, key S, overlay button)
//   viewer     -> imageShown() once the image for store.index is on screen,
//                 imageFailed() when it failed (the error card is already up)
//   controller -> folderLoaded(count, name) after it put the next folder's
//                 listing into the store; noNextFolder() when /nav says none
// The state machine never owns time: the timer starts only when the current
// image is on screen, so a slow NAS image still gets its whole interval.
//
// Fade: while a slideshow is running and settings.transition === 'fade' (and
// the OS does not ask for reduced motion), `#stage` carries class FADE_CLASS.
// Viewer contract: see the end of panels.css.

import { FOLDER_TOAST_MS, HINT_TOAST_MS, SLIDESHOW_FADE_MS } from './constants';
import type { Ctx } from './ctx';
import { showToast } from './errors';
import { t } from './i18n';
import { idle, step, type SlideEffect, type SlideEvent, type SlideState } from './slideshow';
import './panels.css';

export const FADE_CLASS = 'ps-fade';
const FALLBACK_INTERVAL_S = 5; // only if settings never loaded
const FALLBACK_CFG = { on_end: 'next_folder', shuffle: false } as const;

export interface SlideshowHooks {
  /** Controller: do the /nav call, then folderLoaded() or noNextFolder(). */
  loadNextFolder: () => void;
  /** Toast host; default document.body. */
  host?: HTMLElement;
  /** Element that gets FADE_CLASS; default #stage. */
  stage?: HTMLElement | null;
  /** Injection points for tests. */
  step?: typeof step;
  rand?: () => number;
}

export interface SlideshowHandle {
  toggle(): void;
  start(): void;
  pause(): void;
  resume(): void;
  stop(): void;
  imageShown(): void;
  imageFailed(): void;
  folderLoaded(count: number, folderName: string): void;
  noNextFolder(): void;
  dispose(): void;
}

function reducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

class SlideshowUi implements SlideshowHandle {
  private s: SlideState = idle;
  private timer: ReturnType<typeof setTimeout> | null = null;
  /** A `show` was requested and the viewer has not reported it yet. */
  private awaiting = false;
  /** load_next_folder is out; no timer runs until folderLoaded(). */
  private waitingFolder = false;
  /** imageShown() arrived while waiting for the folder (controller showed #0 itself). */
  private shownWhileWaiting = false;
  /** True only while folderLoaded() feeds its event to the machine. */
  private fromFolder = false;
  /** Consecutive failed images; stops a folder of broken files from spinning. */
  private failures = 0;
  private readonly stepFn: typeof step;
  private readonly rand: () => number;
  private readonly cleanups: (() => void)[] = [];

  constructor(private readonly ctx: Ctx, private readonly hooks: SlideshowHooks) {
    this.stepFn = hooks.step ?? step;
    this.rand = hooks.rand ?? Math.random;
    this.cleanups.push(
      // Esc stops a running or paused slideshow, one layer per press.
      ctx.esc.register('slideshow', () => {
        if (this.s.phase === 'off') return false;
        this.stop();
        return true;
      }),
      ctx.store.subscribe(() => this.applyFade()),
    );
  }

  toggle(): void {
    if (this.s.phase === 'off') this.start();
    else if (this.s.phase === 'playing') this.pause();
    else this.resume();
  }

  start(): void {
    if (this.s.phase !== 'off') return;
    const { listing, index } = this.ctx.store.get();
    this.reset();
    // The current image is already on screen, so the timer starts right away.
    this.send({ type: 'start', index, count: listing?.images.length ?? 0 });
    if (this.phase() === 'playing') this.schedule({ type: 'tick' }, this.intervalMs());
  }

  pause(): void {
    this.clearTimer();
    this.send({ type: 'pause' });
  }

  resume(): void {
    if (this.s.phase !== 'paused') return;
    this.send({ type: 'resume' });
    if (!this.awaiting && !this.waitingFolder) this.schedule({ type: 'tick' }, this.intervalMs());
  }

  stop(): void {
    this.send({ type: 'stop' });
  }

  imageShown(): void {
    if (this.s.phase === 'off') return;
    if (this.waitingFolder) {
      this.shownWhileWaiting = true;
      return;
    }
    this.awaiting = false;
    this.failures = 0;
    // Manual navigation while playing: continue from where the user is.
    const pos = this.s.order.indexOf(this.ctx.store.get().index);
    if (pos >= 0) this.s = { ...this.s, pos };
    if (this.s.phase === 'playing') this.schedule({ type: 'tick' }, this.intervalMs());
    else this.clearTimer();
  }

  imageFailed(): void {
    this.awaiting = false;
    if (this.s.phase !== 'playing' || this.waitingFolder) return;
    // A failed image counts as shown: skip it now (deferred one task so the
    // viewer's failure handler has returned before we navigate again).
    if (++this.failures >= Math.max(1, this.s.order.length)) {
      this.stop(); // everything failed: leave the last error card on screen
      return;
    }
    this.schedule({ type: 'image_failed' }, 0);
  }

  folderLoaded(count: number, folderName: string): void {
    if (this.s.phase === 'off') return;
    this.waitingFolder = false;
    this.failures = 0;
    if (count > 0) showToast(this.toastHost(), folderName, FOLDER_TOAST_MS);
    this.fromFolder = true;
    this.send({ type: 'folder_loaded', count });
    this.fromFolder = false;
  }

  noNextFolder(): void {
    this.waitingFolder = false;
    this.send({ type: 'no_next_folder' });
  }

  dispose(): void {
    this.clearTimer();
    for (const c of this.cleanups.splice(0)) c();
    this.stageEl()?.classList.remove(FADE_CLASS);
  }

  /** A method, not `this.s.phase`: TS cannot see that send() changes it. */
  private phase(): SlideState['phase'] {
    return this.s.phase;
  }

  private reset(): void {
    this.awaiting = this.waitingFolder = this.shownWhileWaiting = false;
    this.failures = 0;
  }

  private send(ev: SlideEvent): void {
    const [next, effect] = this.stepFn(this.s, ev, this.cfg(), this.rand);
    this.s = next;
    if (this.ctx.store.get().slideshow !== next.phase) this.ctx.store.set({ slideshow: next.phase });
    this.applyFade();
    this.apply(effect);
  }

  private apply(effect: SlideEffect): void {
    switch (effect.type) {
      case 'show':
        this.show(effect.index);
        break;
      case 'load_next_folder':
        this.clearTimer();
        this.waitingFolder = true;
        this.shownWhileWaiting = false;
        this.hooks.loadNextFolder();
        break;
      case 'stopped':
        this.clearTimer();
        this.reset();
        if (effect.reason === 'end') showToast(this.toastHost(), t('slideshow.ended'), HINT_TOAST_MS);
        break;
      case 'none':
        break;
    }
  }

  private show(index: number): void {
    const onScreen = index === this.ctx.store.get().index;
    // After a folder switch the controller may already have shown #0; trust
    // that only if the viewer said so. Otherwise wait for imageShown().
    if (onScreen && (!this.fromFolder || this.shownWhileWaiting)) {
      this.schedule({ type: 'tick' }, this.intervalMs());
      return;
    }
    this.awaiting = true;
    if (!onScreen) this.ctx.dispatch({ type: 'goto', index });
  }

  private schedule(ev: SlideEvent, ms: number): void {
    this.clearTimer();
    this.timer = setTimeout(() => {
      this.timer = null;
      this.send(ev);
    }, ms);
  }

  private clearTimer(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  private intervalMs(): number {
    return (this.ctx.store.get().settings?.interval_s ?? FALLBACK_INTERVAL_S) * 1000;
  }

  private cfg(): { on_end: 'loop' | 'next_folder' | 'stop'; shuffle: boolean } {
    const s = this.ctx.store.get().settings;
    return s ? { on_end: s.on_end, shuffle: s.shuffle } : FALLBACK_CFG;
  }

  private toastHost(): HTMLElement {
    return this.hooks.host ?? document.body;
  }

  private stageEl(): HTMLElement | null {
    return this.hooks.stage !== undefined ? this.hooks.stage : document.getElementById('stage');
  }

  private applyFade(): void {
    const st = this.ctx.store.get();
    const on = st.slideshow !== 'off' && st.settings?.transition === 'fade' && !reducedMotion();
    const stage = this.stageEl();
    stage?.classList.toggle(FADE_CLASS, on);
    if (on) stage?.style.setProperty('--ps-fade-ms', `${SLIDESHOW_FADE_MS}ms`);
  }
}

export function mountSlideshow(ctx: Ctx, hooks: SlideshowHooks): SlideshowHandle {
  return new SlideshowUi(ctx, hooks);
}
