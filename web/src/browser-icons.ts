// 16 px monochrome line icons, drawn with currentColor so CSS themes them.

const PATHS = {
  folder: 'M1.5 4h4.2l1.6 1.6h7.2v7.4H1.5z',
  drive: 'M2 9.5h12V13H2zM2 9.5 3.6 3.5h8.8L14 9.5M11.5 11.3h.01',
  network: 'M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2zM2 8h12M8 2c2.2 2.2 2.2 9.8 0 12M8 2c-2.2 2.2-2.2 9.8 0 12',
  clock: 'M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2zM8 4.7V8l2.4 1.5',
  image: 'M2 3h12v10H2zM2 11.2l3.4-3.4 3 3 2.1-2.1 3.5 3.5M10.6 6h.01',
  back: 'M10 3 5 8l5 5',
  forward: 'M6 3l5 5-5 5',
  up: 'M8 13V3.5M4 7.5l4-4 4 4',
  grid: 'M2 2h5v5H2zM9 2h5v5H9zM2 9h5v5H2zM9 9h5v5H9z',
  list: 'M2 4h12M2 8h12M2 12h12',
  chevron: 'M6 3.5l4.5 4.5L6 12.5',
  panel: 'M2 3h12v10H2zM6 3v10',
  close: 'M4 4l8 8M12 4l-8 8',
  names: 'M3 4h10M8 4v8.5M5.5 12.5h5',
  asc: 'M8 13V3M4 7l4-4 4 4',
  desc: 'M8 3v10M4 9l4 4 4-4',
  broken: 'M2 3h12v10H2zM4.5 10.5 8 7l3.5 3.5M5 5l6 6',
} as const;

export type IconName = keyof typeof PATHS;

const NS = 'http://www.w3.org/2000/svg';

export function icon(name: IconName): SVGSVGElement {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('width', '16');
  svg.setAttribute('height', '16');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '1.25');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.setAttribute('aria-hidden', 'true');
  svg.classList.add('ic');
  const p = document.createElementNS(NS, 'path');
  p.setAttribute('d', PATHS[name]);
  svg.append(p);
  return svg;
}
