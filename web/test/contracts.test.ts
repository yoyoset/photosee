// Golden files must stay assignable to the TS API types (drift guard).
import { describe, expect, it } from 'vitest';
import type { Listing, Meta, Session, Settings } from '../src/api';
import list from '../../contracts/list.json';
import meta from '../../contracts/meta.json';
import session from '../../contracts/session.json';
import config from '../../contracts/config.json';

describe('contracts', () => {
  it('golden files type-check against api.ts', () => {
    const l: Listing = list as Listing;
    const m: Meta = meta as Meta;
    const s: Session = session as Session;
    const c: Settings = config as Settings;
    expect(l.images.map((i) => i.name)).toEqual(['a2.jpg', 'a10.jpg']);
    expect(m.width).toBe(4000);
    expect(s.api_version).toBe(1);
    expect(c.interval_s).toBe(5);
  });
});
