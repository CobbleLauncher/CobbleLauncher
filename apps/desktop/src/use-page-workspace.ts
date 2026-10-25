// SPDX-License-Identifier: MIT
import { reactive } from 'vue';

// In-memory only: page controls survive navigation for this app session. Keep
// credentials, drafts, pending requests, and operation state out of this store.
const workspaces = new Map<string, object>();
const maxWorkspaces = 48;

export function usePageWorkspace<T extends object>(page: string, gameId: string | undefined, initial: () => T): T {
  const key = JSON.stringify([page, gameId ?? '']);
  let state = workspaces.get(key) as T | undefined;
  if (!state) {
    state = reactive(initial()) as T;
    workspaces.set(key, state);
    if (workspaces.size > maxWorkspaces) workspaces.delete(workspaces.keys().next().value!);
  }
  return state;
}

export function clearPageWorkspacesForTests() {
  workspaces.clear();
}
