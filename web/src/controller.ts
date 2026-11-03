// Action -> state transitions and folder loading. Orchestrator (Wave 2).

import type { Action } from './actions';
import type { Ctx } from './ctx';

export function createDispatch(getCtx: () => Ctx): (a: Action) => void {
  return (a) => {
    void getCtx;
    void a; // Wave 2: integration
  };
}
