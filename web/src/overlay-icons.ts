// Overlay button icons: constant SVG markup, never built from file data.

const SVG = (body: string): string =>
  `<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" ` +
  `stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export const ICONS = {
  back: SVG('<path d="M19 12H5M11 5l-7 7 7 7"/>'),
  prev: SVG('<path d="M15 5l-7 7 7 7"/>'),
  next: SVG('<path d="M9 5l7 7-7 7"/>'),
  play: SVG('<path d="M8 5v14l11-7z" fill="currentColor"/>'),
  pause: SVG('<path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="currentColor"/>'),
  ccw: SVG('<path d="M3 4v6h6M5.1 15a8 8 0 1 0 1.9-8.3L3 10"/>'),
  cw: SVG('<path d="M21 4v6h-6M18.9 15a8 8 0 1 1-1.9-8.3L21 10"/>'),
  fit: SVG('<path d="M3 6h18v12H3zM8 10h8v4H8z"/>'),
  // Fit width / height: the frame with arrows to the two edges it fills.
  fitW: SVG('<path d="M3 6h18v12H3zM7 12h10M9 10l-2 2 2 2M15 10l2 2-2 2"/>'),
  fitH: SVG('<path d="M6 3h12v18H6zM12 7v10M10 9l2-2 2 2M10 15l2 2 2-2"/>'),
  info: SVG('<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>'),
  actual: SVG('<path d="M3 6h18v12H3z"/><text x="12" y="15.5" font-size="8" text-anchor="middle" fill="currentColor" stroke="none">1:1</text>'),
  fullscreen: SVG('<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>'),
  settings: SVG('<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>'),
};
