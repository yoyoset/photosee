// What every UI module receives. Contract file: orchestrator only.

import type { Api, ImageSource, Session } from './api';
import type { EscStack, Store } from './state';
import type { Action } from './actions';

export interface Ctx {
  store: Store;
  api: Api;
  src: ImageSource;
  esc: EscStack;
  session: Session;
  /** Route a user intent; implemented by controller.ts. */
  dispatch: (a: Action) => void;
}
