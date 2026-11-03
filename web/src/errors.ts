// Error card, NAS banner, empty-folder notice. Package L.

import type { ApiError } from './api';

export function renderErrorCard(host: HTMLElement, err: ApiError, fileName: string): void {
  void host; void err; void fileName; // package L
}

export function showNasBanner(host: HTMLElement, dir: string, retry: () => void): () => void {
  void host; void dir; void retry;
  return () => {}; // package L
}
