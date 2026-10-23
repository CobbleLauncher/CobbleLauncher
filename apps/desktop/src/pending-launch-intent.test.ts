// SPDX-License-Identifier: MIT
import { expect, it, vi } from 'vitest';
import { createPendingLaunchIntentConsumer, type PendingLaunchIntent } from './pending-launch-intent';

const intent: PendingLaunchIntent = { queue_id: 'queued-1', game_id: 'game-a', play: true };
function setup(overrides: Partial<Parameters<typeof createPendingLaunchIntentConsumer>[0]> = {}) {
  const options = {
    peek: vi.fn(async () => intent as PendingLaunchIntent | null),
    acknowledge: vi.fn(async () => true),
    hasGame: vi.fn(() => true),
    confirmWorkspaceLeave: vi.fn(async () => true),
    openGame: vi.fn(),
    startGame: vi.fn(),
    missingGame: vi.fn(),
    failed: vi.fn(),
    ...overrides,
  };
  return { options, consumer: createPendingLaunchIntentConsumer(options) };
}

it('holds an early notification until game and profile loading is complete', async () => {
  const { options, consumer } = setup();
  consumer.notify();
  await Promise.resolve();
  expect(options.peek).not.toHaveBeenCalled();
  consumer.markReady();
  await vi.waitFor(() => expect(options.startGame).toHaveBeenCalledWith('game-a'));
  expect(options.acknowledge).toHaveBeenCalledWith('queued-1');
});

it('keeps the queued request when the user declines workspace navigation', async () => {
  const { options, consumer } = setup({ confirmWorkspaceLeave: vi.fn().mockResolvedValueOnce(false).mockResolvedValue(true) });
  consumer.markReady();
  await vi.waitFor(() => expect(options.confirmWorkspaceLeave).toHaveBeenCalledTimes(1));
  expect(options.acknowledge).not.toHaveBeenCalled();
  consumer.notify();
  await vi.waitFor(() => expect(options.acknowledge).toHaveBeenCalledWith('queued-1'));
  expect(options.startGame).toHaveBeenCalledTimes(1);
});

it('reports and acknowledges a registered game missing from the loaded renderer list', async () => {
  const { options, consumer } = setup({ hasGame: () => false });
  consumer.markReady();
  await vi.waitFor(() => expect(options.missingGame).toHaveBeenCalledWith('game-a'));
  expect(options.acknowledge).toHaveBeenCalledWith('queued-1');
  expect(options.startGame).not.toHaveBeenCalled();
});

it('opens game details for an intent without the play flag', async () => {
  const { options, consumer } = setup({ peek: vi.fn(async () => ({ ...intent, play: false })) });
  consumer.markReady();
  await vi.waitFor(() => expect(options.openGame).toHaveBeenCalledWith('game-a'));
  expect(options.startGame).not.toHaveBeenCalled();
});

it('does not launch an older request if a new queued generation replaces it during confirmation', async () => {
  let latest: PendingLaunchIntent | null = intent;
  let finish!: (value: boolean) => void;
  const confirm = vi.fn().mockImplementationOnce(() => new Promise<boolean>(resolve => { finish = resolve; })).mockResolvedValue(true);
  const { options, consumer } = setup({
    peek: vi.fn(async () => latest),
    confirmWorkspaceLeave: confirm,
  });
  consumer.markReady();
  await vi.waitFor(() => expect(options.confirmWorkspaceLeave).toHaveBeenCalledTimes(1));
  latest = { ...intent, queue_id: 'queued-2', game_id: 'game-b' };
  finish(true);
  await vi.waitFor(() => expect(options.startGame).toHaveBeenCalledWith('game-b'));
  expect(options.startGame).not.toHaveBeenCalledWith('game-a');
});

it('does not launch or acknowledge while a leave prompt completes after disposal', async () => {
  let finish!: (value: boolean) => void;
  const confirmWorkspaceLeave = vi.fn(() => new Promise<boolean>(resolve => { finish = resolve; }));
  const { options, consumer } = setup({ confirmWorkspaceLeave });
  consumer.markReady();
  await vi.waitFor(() => expect(options.confirmWorkspaceLeave).toHaveBeenCalledTimes(1));
  consumer.dispose();
  finish(true);
  await Promise.resolve();
  await Promise.resolve();
  expect(options.startGame).not.toHaveBeenCalled();
  expect(options.openGame).not.toHaveBeenCalled();
  expect(options.acknowledge).not.toHaveBeenCalled();
});
