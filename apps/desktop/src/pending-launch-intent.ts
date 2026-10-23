// SPDX-License-Identifier: MIT
export type PendingLaunchIntent = { queue_id: string; game_id: string; play: boolean };

type LaunchIntentOptions = {
  peek: () => Promise<PendingLaunchIntent | null>;
  acknowledge: (queueId: string) => Promise<boolean>;
  hasGame: (gameId: string) => boolean;
  confirmWorkspaceLeave: () => Promise<boolean>;
  openGame: (gameId: string) => void | Promise<void>;
  startGame: (gameId: string) => void | Promise<void>;
  missingGame: (gameId: string) => void;
  failed: (cause: unknown) => void;
};

/** Queues native launch notifications until the renderer has loaded its game and profile state. */
export function createPendingLaunchIntentConsumer(options: LaunchIntentOptions) {
  let ready = false;
  let queued = false;
  let processing = false;
  let disposed = false;

  async function processOne() {
    const intent = await options.peek();
    if (disposed || !intent) return;
    if (!options.hasGame(intent.game_id)) {
      options.missingGame(intent.game_id);
      await options.acknowledge(intent.queue_id);
      return;
    }
    if (!await options.confirmWorkspaceLeave() || disposed) return;

    // A second-instance request may replace the queue while the leave prompt is open.
    const current = await options.peek();
    if (disposed) return;
    if (current?.queue_id !== intent.queue_id) {
      queued = current !== null;
      return;
    }
    if (intent.play) await options.startGame(intent.game_id);
    else await options.openGame(intent.game_id);
    await options.acknowledge(intent.queue_id);
  }

  async function drain() {
    if (disposed || !ready || processing || !queued) return;
    processing = true;
    try {
      while (queued && ready && !disposed) {
        queued = false;
        try { await processOne(); }
        catch (cause) { options.failed(cause); }
      }
    } finally {
      processing = false;
      if (queued && ready && !disposed) void drain();
    }
  }

  return {
    notify() {
      if (disposed) return;
      queued = true;
      void drain();
    },
    markReady() {
      if (disposed) return;
      ready = true;
      queued = true;
      void drain();
    },
    dispose() {
      disposed = true;
      queued = false;
    },
  };
}
