// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { visitBatches } from './batches';

it('lets a user action cancel preparation before the rest of a large chunk is visited', async () => {
  const controller = new AbortController();
  let visited = 0, yields = 0;
  const complete = await visitBatches(Array.from({ length: 10000 }, (_, index) => index), () => visited++, controller.signal, async () => {
    yields++; controller.abort();
  });
  expect(complete).toBe(false);
  expect(visited).toBeGreaterThan(0);
  expect(visited).toBeLessThan(10000);
  expect(yields).toBe(1);
});

it('preserves every position in order across cooperative yields', async () => {
  const input = Array.from({ length: 10001 }, (_, index) => index);
  const output: number[] = []; let yields = 0;
  expect(await visitBatches(input, (value, index) => { expect(value).toBe(index); output.push(value); }, new AbortController().signal, async () => { yields++; })).toBe(true);
  expect(output).toEqual(input);
  expect(yields).toBeGreaterThan(1);
});

it('does no work for an already closed viewer', async () => {
  const controller = new AbortController(); controller.abort();
  let calls = 0;
  expect(await visitBatches([1, 2], () => calls++, controller.signal, async () => { calls++; })).toBe(false);
  expect(calls).toBe(0);
});
