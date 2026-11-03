import { describe, expect, it } from 'vitest';
import { EscStack, Store } from '../src/state';

describe('EscStack', () => {
  it('unwinds topmost open layer first and skips closed ones', () => {
    const esc = new EscStack();
    const closed: string[] = [];
    let panelOpen = false;
    let zoomed = true;
    esc.register('panel', () => panelOpen);
    esc.register('zoom', () => (zoomed ? ((zoomed = false), closed.push('zoom'), true) : false));
    esc.register('window', () => (closed.push('window'), true));
    expect(esc.handle()).toBe('zoom');
    expect(esc.handle()).toBe('window');
    panelOpen = true;
    expect(esc.handle()).toBe('panel');
    expect(closed).toEqual(['zoom', 'window']);
  });
});

describe('Store', () => {
  it('notifies with previous state', () => {
    const s = new Store();
    let seen = -1;
    s.subscribe((now, prev) => (seen = now.index - prev.index));
    s.set({ index: 3 });
    expect(seen).toBe(3);
  });
});
