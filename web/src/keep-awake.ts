// Keeps the screen on while a slideshow plays: nobody touches the phone
// then, and it would lock after its idle timeout (phone review, 2026-11-06).
// Reading needs nothing: every page turn is a touch.
//
// The Screen Wake Lock API needs a secure context (HTTPS or localhost). The
// Docker build is served over plain HTTP on the LAN, where the API does not
// exist, so there a tiny muted looping video plays instead (keepawake.mp4,
// 16x16 black, 1.6 KB): phones keep the screen on while a video plays.
// Neither is verified on a real phone yet.

import type { Store } from './state';

interface WakeLockLike { release(): Promise<void> }
type WakeNav = Navigator & { wakeLock?: { request(type: 'screen'): Promise<WakeLockLike> } };

export function keepAwakeDuringSlideshow(store: Store, doc: Document): () => void {
  const win = doc.defaultView as Window;
  const nav = win.navigator as WakeNav;
  let lock: WakeLockLike | null = null;
  let video: HTMLVideoElement | null = null;
  let on = false;

  const acquire = (): void => {
    if (nav.wakeLock && win.isSecureContext) {
      // Released by the browser whenever the page is hidden; reacquired on return.
      nav.wakeLock.request('screen').then((l) => { if (on) lock = l; else void l.release(); }, () => {});
      return;
    }
    video ??= Object.assign(doc.createElement('video'), { src: '/keepawake.mp4', muted: true, loop: true, playsInline: true });
    video.setAttribute('playsinline', '');
    video.style.cssText = 'position:fixed;width:1px;height:1px;opacity:0;pointer-events:none';
    doc.body.append(video);
    void video.play?.()?.catch(() => {}); // refused: the screen may lock, nothing else breaks
  };
  const release = (): void => {
    void lock?.release().catch(() => {});
    lock = null;
    video?.pause?.();
    video?.remove();
  };
  const sync = (): void => {
    const want = store.get().slideshow === 'playing' && doc.visibilityState !== 'hidden';
    if (want === on) return;
    on = want;
    if (on) acquire();
    else release();
  };
  const unsub = store.subscribe((s, p) => { if (s.slideshow !== p.slideshow) sync(); });
  doc.addEventListener('visibilitychange', sync);
  return () => { unsub(); doc.removeEventListener('visibilitychange', sync); on = false; release(); };
}
