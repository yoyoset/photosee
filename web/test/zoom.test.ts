import { describe, expect, it } from 'vitest';
import { ZOOM_MAX, ZOOM_STEP } from '../src/constants';
import type { FitMode } from '../src/state';
import {
  FIT, beyondLoaded, clampPan, cssTransform, effectiveScale, fitScale, grew, isPanorama, isWholeImage,
  overflows, panBy, requestBox, resolutionNeed, rotated, rotatedFit, sameBox, toggleZoom, zoomActual, zoomAt, zoomStep,
  type Box, type Geometry, type Point, type Zoom,
} from '../src/zoom';

const geo = (img: Box, view: Box = { w: 1000, h: 800 }, dpr = 1, fit: FitMode = 'contain'): Geometry =>
  ({ img, view, dpr, fit });

/** Screen position (CSS px, stage coords) of image point `p` (image px from
 * the image center), from the documented units. */
function screenOf(z0: Zoom, g: Geometry, p: Point): Point {
  const z = clampPan(z0, g);
  const s = effectiveScale(z, g) / g.dpr;
  return { x: g.view.w / 2 + z.x + p.x * s, y: g.view.h / 2 + z.y + p.y * s };
}
function imagePointAt(z0: Zoom, g: Geometry, a: Point): Point {
  const z = clampPan(z0, g);
  const s = effectiveScale(z, g) / g.dpr;
  return { x: (a.x - g.view.w / 2 - z.x) / s, y: (a.y - g.view.h / 2 - z.y) / s };
}

describe('fitScale', () => {
  it('fits the limiting axis, and enlarges a small image to fill the view', () => {
    expect(fitScale({ w: 4000, h: 3000 }, { w: 1000, h: 800 }, 1)).toBe(0.25);
    expect(fitScale({ w: 4000, h: 3000 }, { w: 1000, h: 800 }, 2)).toBe(0.5);
    // The user's complaint (2026-11-06): small scans stayed 1:1, "only a little bit" of the screen.
    expect(fitScale({ w: 200, h: 100 }, { w: 1000, h: 800 }, 1)).toBe(5);
    expect(fitScale({ w: 200, h: 100 }, { w: 1000, h: 800 }, 2)).toBe(10);
  });
  it('fit width / height fill that screen axis, whatever the other does', () => {
    // A portrait comic page on a landscape screen: width fills, height overflows.
    expect(fitScale({ w: 1000, h: 1500 }, { w: 1600, h: 900 }, 1, 'width')).toBe(1.6);
    expect(fitScale({ w: 1000, h: 1500 }, { w: 1600, h: 900 }, 1, 'height')).toBe(0.6);
    expect(fitScale({ w: 1000, h: 1500 }, { w: 1600, h: 900 }, 1, 'contain')).toBe(0.6);
    expect(fitScale({ w: 1000, h: 1500 }, { w: 800, h: 900 }, 2, 'width')).toBe(1.6);
  });
  it('fits a panorama on its short axis, either direction', () => {
    expect(fitScale({ w: 6000, h: 1000 }, { w: 1000, h: 800 }, 1)).toBe(0.8);
    expect(fitScale({ w: 500, h: 3000 }, { w: 1000, h: 800 }, 1)).toBe(2);
    // 3.5:1 in a 4:1 window fits whole: short axis = image height, not max(sx, sy).
    expect(fitScale({ w: 3500, h: 1000 }, { w: 4000, h: 1000 }, 1)).toBe(1);
  });
  it('is 1 for degenerate input instead of NaN or Infinity', () => {
    expect(fitScale({ w: 0, h: 10 }, { w: 1000, h: 800 }, 1)).toBe(1);
    expect(fitScale({ w: 10, h: 10 }, { w: 0, h: 0 }, 1)).toBe(1);
    expect(fitScale({ w: 10, h: 10 }, { w: 10, h: 10 }, 0, 'width')).toBe(1);
  });
  it('overflows: true only when the fitted image is bigger than the view', () => {
    const page = { w: 1000, h: 1500 };
    expect(overflows(geo(page, { w: 1600, h: 900 }, 1, 'width'))).toBe(true);
    expect(overflows(geo(page, { w: 1600, h: 900 }, 1, 'contain'))).toBe(false);
    expect(overflows(geo(page, { w: 1600, h: 900 }, 1, 'height'))).toBe(false);
    expect(overflows(geo({ w: 6000, h: 1000 }))).toBe(true); // panorama
  });
});

describe('isPanorama / rotated', () => {
  it('uses a strict PANORAMA_ASPECT threshold both ways', () => {
    expect(isPanorama({ w: 3000, h: 1000 })).toBe(false);
    expect(isPanorama({ w: 3010, h: 1000 })).toBe(true);
    expect(isPanorama({ w: 1000, h: 3010 })).toBe(true);
    expect(isPanorama({ w: 0, h: 0 })).toBe(false);
  });
  it('swaps for odd quarter turns only', () => {
    expect(rotated({ w: 4, h: 3 }, 1)).toEqual({ w: 3, h: 4 });
    expect(rotated({ w: 4, h: 3 }, 2)).toEqual({ w: 4, h: 3 });
    expect(rotated({ w: 4, h: 3 }, 3)).toEqual({ w: 3, h: 4 });
    expect(rotated({ w: 4, h: 3 }, -1)).toEqual({ w: 3, h: 4 });
  });
});

describe('requestBox', () => {
  it('is ceil(view x dpr) before the size is known and for normal images', () => {
    expect(requestBox(null, { w: 1366, h: 768 }, 1.25)).toEqual({ w: 1708, h: 960 });
    expect(requestBox({ w: 4000, h: 3000 }, { w: 1000, h: 800 }, 2)).toEqual({ w: 2000, h: 1600 });
  });
  it('does not ask for an extra pixel because of float error', () => {
    // 1000 * 1.1 = 1100.0000000000002 in floating point.
    expect(requestBox(null, { w: 1000, h: 800 }, 1.1)).toEqual({ w: 1100, h: 880 });
  });
  it('grows a panorama long axis so the short axis fills the view, never past the original', () => {
    expect(requestBox({ w: 6000, h: 1000 }, { w: 1000, h: 800 }, 1)).toEqual({ w: 4800, h: 800 });
    // Fitting needs 1.5x here; the server cannot upscale, so the box stops at the image.
    expect(requestBox({ w: 1000, h: 6000 }, { w: 1000, h: 800 }, 1.5)).toEqual({ w: 1500, h: 6000 });
  });
  it('fit width/height: one box for every page up to PANORAMA_ASPECT, known size or not', () => {
    const view = { w: 1600, h: 900 };
    const box = requestBox(null, view, 1, 'width');
    expect(box).toEqual({ w: 1600, h: 4800 });
    // The page on screen asks for the same URL as its prefetched neighbour.
    expect(requestBox({ w: 1000, h: 1500 }, view, 1, 'width')).toEqual(box);
    expect(requestBox(null, view, 1, 'height')).toEqual({ w: 2700, h: 900 });
    // Longer than that: grown to the fitted size.
    expect(requestBox({ w: 2000, h: 8000 }, view, 1, 'width')).toEqual({ w: 1600, h: 6400 });
  });
  it('rotatedFit swaps width and height for a quarter turn', () => {
    expect([rotatedFit('width', 1), rotatedFit('height', 3), rotatedFit('width', 2), rotatedFit('contain', 1)])
      .toEqual(['height', 'width', 'width', 'contain']);
  });
});

describe('zoomAt', () => {
  const g = geo({ w: 4000, h: 3000 });
  it('keeps the image point under the anchor fixed', () => {
    for (const [z, a, f] of [
      [FIT, { x: 800, y: 600 }, 2],
      [{ mode: 'custom', scale: 0.5, x: -100, y: 50 }, { x: 120, y: 700 }, ZOOM_STEP],
      [{ mode: 'custom', scale: 2, x: 300, y: -200 }, { x: 500, y: 400 }, 1 / ZOOM_STEP],
    ] as const) {
      const p = imagePointAt(z, g, a);
      const next = zoomAt(z, g, f, a);
      expect(next.scale).toBeCloseTo(effectiveScale(z, g) * f, 9);
      const at = screenOf(next, g, p);
      expect(at.x).toBeCloseTo(a.x, 6);
      expect(at.y).toBeCloseTo(a.y, 6);
    }
  });
  it('holds on a 2x screen too', () => {
    const g2 = geo({ w: 4000, h: 3000 }, { w: 1000, h: 800 }, 2);
    const a = { x: 900, y: 100 };
    const p = imagePointAt(FIT, g2, a);
    const at = screenOf(zoomAt(FIT, g2, 3, a), g2, p);
    expect(at.x).toBeCloseTo(a.x, 6);
    expect(at.y).toBeCloseTo(a.y, 6);
  });
  it('zooms around the view center without an anchor', () => {
    const z = zoomAt(FIT, g, 4);
    expect(z).toEqual({ mode: 'custom', scale: 1, x: 0, y: 0 });
  });
  it('clamps to [fit, ZOOM_MAX] and lands on mode fit', () => {
    expect(zoomAt(FIT, g, 0.1)).toEqual({ mode: 'fit', scale: 0.25, x: 0, y: 0 });
    expect(zoomAt({ mode: 'custom', scale: 0.3, x: 40, y: 0 }, g, 0.5).mode).toBe('fit');
    expect(zoomAt(FIT, g, 1e6).scale).toBe(ZOOM_MAX);
    expect(zoomStep(FIT, g, 1).scale).toBeCloseTo(0.25 * ZOOM_STEP, 12);
    expect(zoomStep(zoomStep(FIT, g, 1), g, -1).mode).toBe('fit');
    expect(zoomAt(FIT, g, Number.NaN).scale).toBe(0.25);
  });
  it('never zooms out past fit, which for a small image is above 1:1', () => {
    expect(zoomAt(FIT, geo({ w: 200, h: 100 }), 0.5)).toEqual({ mode: 'fit', scale: 5, x: 0, y: 0 });
  });
});

describe('clampPan / panBy', () => {
  const g = geo({ w: 4000, h: 3000 });
  it('centers a side smaller than the view', () => {
    // At fit the image is 1000x750 CSS px in 1000x800: no room either way.
    expect(clampPan({ ...FIT, x: 300, y: -90 }, g)).toEqual({ mode: 'fit', scale: 0.25, x: 0, y: 0 });
  });
  it('keeps image edges outside the view edges', () => {
    // At 100% the image is 4000x3000: x in [-1500, 1500], y in [-1100, 1100].
    const z: Zoom = { mode: 'actual', scale: 1, x: 0, y: 0 };
    expect(panBy(z, g, 5000, -5000)).toEqual({ mode: 'actual', scale: 1, x: 1500, y: -1100 });
    expect(panBy(z, g, -20, 30)).toEqual({ mode: 'actual', scale: 1, x: -20, y: 30 });
  });
  it('lets a panorama pan along its length in fit mode', () => {
    const p = geo({ w: 6000, h: 1000 });
    // Fit is 0.8: 4800x800 CSS px, so x in [-1900, 1900], y fixed.
    expect(panBy(FIT, p, -5000, 40)).toEqual({ mode: 'fit', scale: 0.8, x: -1900, y: 0 });
  });
  it('a fitted page that overflows opens at its top-left, and the first drag moves from there', () => {
    const page = geo({ w: 1000, h: 1500 }, { w: 1600, h: 900 }, 1, 'width');
    // 1600x2400 CSS px in 1600x900: y in [-750, 750]; +750 shows the top edge.
    expect(clampPan(FIT, page)).toEqual({ mode: 'fit', scale: 1.6, x: 0, y: 750 });
    expect(panBy(FIT, page, 0, -100)).toEqual({ mode: 'fit', scale: 1.6, x: 0, y: 650 });
  });
  it('treats NaN pan as centered', () => {
    expect(clampPan({ mode: 'actual', scale: 1, x: Number.NaN, y: 0 }, g).x).toBe(0);
  });
});

describe('toggleZoom / zoomActual', () => {
  const g = geo({ w: 4000, h: 3000 });
  it('fit -> 100% keeps the image point under the anchor where it is', () => {
    for (const a of [{ x: 600, y: 500 }, { x: 100, y: 700 }, { x: 990, y: 50 }]) {
      const p = imagePointAt(FIT, g, a);
      const z = toggleZoom(FIT, g, a);
      expect([z.mode, z.scale]).toEqual(['actual', 1]);
      const at = screenOf(z, g, p);
      expect(at.x).toBeCloseTo(a.x, 6);
      expect(at.y).toBeCloseTo(a.y, 6);
    }
  });
  it('fit -> 100% at an anchor matches zoomAt by the same factor', () => {
    const a = { x: 250, y: 650 };
    const viaStep = zoomAt(FIT, g, 1 / 0.25, a);
    const viaToggle = toggleZoom(FIT, g, a);
    expect(viaToggle.x).toBeCloseTo(viaStep.x, 9);
    expect(viaToggle.y).toBeCloseTo(viaStep.y, 9);
  });
  it('100% clamps the pan when the anchored point would pull an edge in', () => {
    // Unclamped this would be (-125, -100); at 100% the edges allow only +-100.
    const small = geo({ w: 1200, h: 1000 });
    const z = zoomActual(FIT, small, { x: 1000, y: 800 });
    expect(z).toEqual({ mode: 'actual', scale: 1, x: -100, y: -100 });
  });
  it('an enlarged small image doubles instead of shrinking to 100%', () => {
    const z = toggleZoom(FIT, geo({ w: 200, h: 100 }));
    expect([z.mode, z.scale]).toEqual(['custom', 10]);
  });
  it('anything else -> fit, centered', () => {
    expect(toggleZoom({ mode: 'actual', scale: 1, x: 100, y: 100 }, g)).toEqual({ mode: 'fit', scale: 0.25, x: 0, y: 0 });
    expect(toggleZoom({ mode: 'custom', scale: 3, x: 1, y: 1 }, g).mode).toBe('fit');
  });
  it('zoomActual without anchor keeps the current center', () => {
    const z = zoomActual({ mode: 'custom', scale: 0.5, x: 100, y: -50 }, g);
    expect(z).toEqual({ mode: 'actual', scale: 1, x: 200, y: -100 });
  });
});

describe('resolution checks', () => {
  it('beyondLoaded compares device px on screen with loaded px', () => {
    expect(beyondLoaded(1, { w: 4000, h: 3000 }, { w: 1000, h: 750 })).toBe(true);
    expect(beyondLoaded(0.25, { w: 4000, h: 3000 }, { w: 1000, h: 750 })).toBe(false);
    expect(beyondLoaded(0.2505, { w: 4000, h: 3000 }, { w: 1000, h: 750 })).toBe(false);
  });
  it('isWholeImage: no box, as big as meta, or smaller than the box both ways', () => {
    expect(isWholeImage({ w: 10, h: 10 }, null, null)).toBe(true);
    expect(isWholeImage({ w: 4000, h: 3000 }, { w: 1000, h: 800 }, { w: 4000, h: 3000 })).toBe(true);
    expect(isWholeImage({ w: 200, h: 150 }, { w: 1000, h: 800 }, null)).toBe(true);
    expect(isWholeImage({ w: 1000, h: 750 }, { w: 1000, h: 800 }, null)).toBe(false);
    expect(isWholeImage({ w: 1000, h: 750 }, { w: 1000, h: 800 }, { w: 4000, h: 3000 })).toBe(false);
  });
  it('resolutionNeed: bigger box in fit mode, the original when zoomed past the pixels', () => {
    const img = { w: 4000, h: 3000 };
    const loaded = { w: 1000, h: 750 };
    const asked = { w: 1000, h: 800 };
    expect(resolutionNeed(true, 0.5, img, loaded, { w: 2000, h: 1600 }, asked)).toEqual({ w: 2000, h: 1600 });
    expect(resolutionNeed(true, 0.125, img, loaded, { w: 500, h: 400 }, asked)).toBeNull();
    expect(resolutionNeed(false, 0.5, img, loaded, asked, asked)).toBe('original');
    expect(resolutionNeed(false, 0.25, img, loaded, { w: 2000, h: 1600 }, asked)).toBeNull();
  });
  it('sameBox treats null as "the original"', () => {
    expect([sameBox(null, null), sameBox(null, { w: 1, h: 1 }), sameBox({ w: 1, h: 2 }, { w: 1, h: 2 })]).toEqual([true, false, true]);
  });
  it('grew ignores growth under 2%', () => {
    expect(grew({ w: 2000, h: 1600 }, { w: 1000, h: 800 })).toBe(true);
    expect(grew({ w: 1010, h: 800 }, { w: 1000, h: 800 })).toBe(false);
    expect(grew({ w: 500, h: 400 }, { w: 1000, h: 800 })).toBe(false);
  });
});

describe('cssTransform', () => {
  it('centers, scales by scale/dpr, rotates, then pans', () => {
    const g = geo({ w: 4000, h: 3000 }, { w: 1000, h: 800 }, 2);
    expect(cssTransform(FIT, g, 0)).toBe('translate(500px, 400px) rotate(0deg) scale(0.25) translate(-2000px, -1500px)');
    // g.img is already rotated; the element keeps its unrotated CSS size.
    const r = geo({ w: 3000, h: 4000 }, { w: 1000, h: 800 }, 1);
    expect(cssTransform(FIT, r, 1)).toBe('translate(500px, 400px) rotate(90deg) scale(0.2) translate(-2000px, -1500px)');
  });
});
