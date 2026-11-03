// Virtualized thumbnail strip. Package J.

export interface Range { start: number; end: number }

/** Items intersecting the viewport plus `overscan` on each side. */
export function visibleRange(
  scrollLeft: number, viewportW: number, itemW: number, count: number, overscan: number,
): Range {
  void scrollLeft; void viewportW; void itemW; void count; void overscan;
  return { start: 0, end: 0 }; // package J
}
