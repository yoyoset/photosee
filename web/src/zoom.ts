// Pure geometry for fitting, zooming and panning. Package I.

export interface Box { w: number; h: number }

/** Scale that fits `img` (display px, after rotation) in `view`; capped at
 * 1 device pixel per image pixel unless `upscale`. */
export function fitScale(img: Box, view: Box, dpr: number, upscale: boolean): number {
  void img; void view; void dpr; void upscale;
  return 1; // package I
}

/** Box to request from /img: fitted size x devicePixelRatio, rounded up. */
export function requestBox(img: Box, view: Box, dpr: number): Box {
  void img; void dpr;
  return view; // package I
}

export function isPanorama(img: Box): boolean {
  void img;
  return false; // package I
}
