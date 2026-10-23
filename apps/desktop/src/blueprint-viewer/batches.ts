// SPDX-License-Identifier: MIT
/** Yield between bounded groups, including within a large IPC chunk. */
export async function visitBatches<T>(items: readonly T[], visit: (item: T, index: number) => void, signal: AbortSignal, yieldControl: () => Promise<void>, batchSize = 2048): Promise<boolean> {
  for (let start = 0; start < items.length; start += batchSize) {
    if (signal.aborted) return false;
    const end = Math.min(start + batchSize, items.length);
    for (let index = start; index < end; index++) visit(items[index]!, index);
    if (end < items.length) await yieldControl();
  }
  return !signal.aborted;
}
