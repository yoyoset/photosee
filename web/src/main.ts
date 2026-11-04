// Entry point: all wiring is in controller-boot.ts.

import { boot } from './controller-boot';

boot().catch((err: unknown) => {
  // boot() already put a retry banner on screen; this is for the console.
  console.error('PhotoSee failed to start', err);
});
