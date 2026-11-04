import { describe, expect, it } from 'vitest';
import {
  cellPos, clampThumb, gridLayout, listLayout, moveIndex, rowCount, scrollToReveal, sliderAction, thumbEdge,
  totalHeight, visibleRange,
} from '../src/grid-geometry';

describe('layout', () => {
  it('fits columns to the width: cell = size + padding, margin on both sides', () => {
    const l = gridLayout(1000, 160, true);
    expect(l.cellW).toBe(172);
    expect(l.cols).toBe(Math.floor((1000 - 16) / 172));
    expect(l.cellH).toBe(172 + 22);
  });

  it('never has fewer than one column, and drops the label strip when names are off', () => {
    expect(gridLayout(50, 320, false).cols).toBe(1);
    expect(gridLayout(500, 100, false).cellH).toBe(gridLayout(500, 100, false).cellW);
  });

  it('positions cells row by row', () => {
    const l = gridLayout(800, 100, true); // cellW 112, 7 columns
    expect(l.cols).toBe(7);
    expect(cellPos(l, 0)).toEqual({ x: 8, y: 8 });
    expect(cellPos(l, 8)).toEqual({ x: 8 + 112, y: 8 + l.cellH });
    expect(totalHeight(l, 15)).toBe(3 * l.cellH + 16);
    expect(rowCount(14, 7)).toBe(2);
  });

  it('the list is one column of full-width rows', () => {
    const l = listLayout(640, 24);
    expect(l).toMatchObject({ cols: 1, cellW: 640, cellH: 24 });
  });

  it('clamps the thumbnail size to 64..640', () => {
    expect([clampThumb(10), clampThumb(900), clampThumb(500), clampThumb(160.4)]).toEqual([64, 640, 500, 160]);
  });
});

describe('visibleRange', () => {
  const l = gridLayout(800, 100, true); // 7 cols

  it('starts at the top with overscan rows below only', () => {
    const r = visibleRange(l, 10_000, 0, 600, 3);
    expect(r.start).toBe(0);
    const rowsInView = Math.ceil((600 - 8) / l.cellH);
    expect(r.end).toBe((rowsInView + 3) * 7);
  });

  it('whole rows only, so a row is never half built', () => {
    const r = visibleRange(l, 10_000, 5000, 600, 2);
    expect(r.start % 7).toBe(0);
    expect(r.end % 7).toBe(0);
  });

  it('clamps at the end of the list', () => {
    const r = visibleRange(l, 20, 1e9, 600, 3);
    expect(r.end).toBeLessThanOrEqual(20);
    expect(r.start).toBeLessThanOrEqual(r.end);
  });

  it('is empty for no items or no height', () => {
    expect(visibleRange(l, 0, 0, 600, 3)).toEqual({ start: 0, end: 0 });
    expect(visibleRange(l, 10, 0, 0, 3)).toEqual({ start: 0, end: 0 });
  });

  it('keeps 10 000 items to a few hundred cells at most', () => {
    const r = visibleRange(l, 10_000, 100_000, 900, 3);
    expect(r.end - r.start).toBeLessThan(200);
  });
});

describe('moveIndex', () => {
  const cols = 5;
  const count = 23; // last row has 3 items: 20, 21, 22

  it('moves by one and by a row, staying put at the edges', () => {
    expect(moveIndex(7, 'right', cols, count, 3)).toBe(8);
    expect(moveIndex(7, 'left', cols, count, 3)).toBe(6);
    expect(moveIndex(7, 'down', cols, count, 3)).toBe(12);
    expect(moveIndex(7, 'up', cols, count, 3)).toBe(2);
    expect(moveIndex(2, 'up', cols, count, 3)).toBe(2);
    expect(moveIndex(0, 'left', cols, count, 3)).toBe(0);
    expect(moveIndex(22, 'right', cols, count, 3)).toBe(22);
  });

  it('down from a full row above a short last row lands on the last item', () => {
    expect(moveIndex(18, 'down', cols, count, 3)).toBe(22); // 18+5 = 23 is past the end
    expect(moveIndex(22, 'down', cols, count, 3)).toBe(22);
  });

  it('Home and End', () => {
    expect(moveIndex(12, 'home', cols, count, 3)).toBe(0);
    expect(moveIndex(12, 'end', cols, count, 3)).toBe(22);
  });

  it('PgUp / PgDn move whole pages, keep the column, and clamp', () => {
    expect(moveIndex(7, 'pagedown', cols, count, 2)).toBe(17);
    expect(moveIndex(17, 'pageup', cols, count, 2)).toBe(7);
    expect(moveIndex(7, 'pageup', cols, count, 3)).toBe(2);
    expect(moveIndex(17, 'pagedown', cols, count, 3)).toBe(22);
  });

  it('with nothing selected the first key selects the first (or, for End/Up, the last) item', () => {
    expect(moveIndex(-1, 'down', cols, count, 3)).toBe(0);
    expect(moveIndex(-1, 'end', cols, count, 3)).toBe(22);
    expect(moveIndex(-1, 'down', cols, 0, 3)).toBe(-1);
  });

  it('in the list, Left and Right do nothing and Up/Down step by one', () => {
    expect(moveIndex(4, 'right', 1, 10, 3, false)).toBe(4);
    expect(moveIndex(4, 'down', 1, 10, 3, false)).toBe(5);
    expect(moveIndex(4, 'pagedown', 1, 10, 3, false)).toBe(7);
  });
});

describe('scroll and thumbnails', () => {
  it('scrolls the least that shows the row, not at all when visible', () => {
    expect(scrollToReveal(100, 500, 200, 50)).toBe(100);
    expect(scrollToReveal(100, 500, 40, 50)).toBe(40);
    expect(scrollToReveal(100, 500, 580, 50)).toBe(130);
    expect(scrollToReveal(100, 500, 580, 50, 8)).toBe(138);
  });

  it('requests cell size x dpr, rounded up to a step and capped at 512', () => {
    expect(thumbEdge(160, 1)).toBe(192);
    expect(thumbEdge(160, 2)).toBe(384);
    expect(thumbEdge(64, 1)).toBe(64);
    expect(thumbEdge(320, 2)).toBe(512);
    expect(thumbEdge(100, 0.5)).toBe(128); // dpr below 1 never asks for less than CSS px
    // Nearby slider positions share a step, so the server cache is hit.
    expect(thumbEdge(170, 1)).toBe(thumbEdge(190, 1));
  });
});

describe('size slider', () => {
  it('one notch past the maximum is the single-image view', () => {
    expect(sliderAction(648, 'grid')).toEqual({ view: 'single' });
    expect(sliderAction(641, 'details')).toEqual({ view: 'single' });
    expect(sliderAction(640, 'grid')).toEqual({ thumb: 640 });
  });

  it('dragging back below the notch leaves single view for the grid', () => {
    expect(sliderAction(400, 'single')).toEqual({ view: 'grid', thumb: 400 });
    expect(sliderAction(10, 'single')).toEqual({ view: 'grid', thumb: 64 });
    expect(sliderAction(200, 'grid')).toEqual({ thumb: 200 });
  });
});
