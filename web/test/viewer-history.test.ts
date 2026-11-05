// The viewer's history entry: system Back closes it, other exits remove it.
import { describe, expect, it, vi } from 'vitest';
import { bindViewerHistory } from '../src/viewer-history';

function fakeWin() {
  const target = new EventTarget();
  const history = { pushState: vi.fn(), back: vi.fn() };
  const win = Object.assign(target, { history }) as unknown as Window;
  const pop = (): boolean => target.dispatchEvent(new Event('popstate'));
  return { win, history, pop };
}

describe('bindViewerHistory', () => {
  it('pushes one entry and Back (popstate) closes the viewer', () => {
    const { win, history, pop } = fakeWin();
    const close = vi.fn();
    const off = bindViewerHistory(win, close);
    expect(history.pushState).toHaveBeenCalledTimes(1);
    pop();
    expect(close).toHaveBeenCalledTimes(1);
    off(); // the mode switch disposes it in response to close()
    expect(history.back).not.toHaveBeenCalled(); // the browser already popped it
  });

  it('leaving another way (Esc, button) pops our entry and stops listening', () => {
    const { win, history, pop } = fakeWin();
    const close = vi.fn();
    const off = bindViewerHistory(win, close);
    off();
    expect(history.back).toHaveBeenCalledTimes(1);
    pop(); // that back() arrives later as popstate: must not close anything
    expect(close).not.toHaveBeenCalled();
  });
});
