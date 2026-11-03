// Prefetch window and decoded-image LRU. Package I.

/** Indices to prefetch around `index`, nearest first, ahead before behind. */
export function prefetchWindow(index: number, count: number, ahead: number, behind: number): number[] {
  void index; void count; void ahead; void behind;
  return []; // package I
}

export class ImageLru {
  constructor(readonly budgetBytes: number) {}
  get(key: string): HTMLImageElement | undefined {
    void key;
    return undefined; // package I
  }
  put(key: string, img: HTMLImageElement, bytes: number): void {
    void key; void img; void bytes; // package I
  }
}
