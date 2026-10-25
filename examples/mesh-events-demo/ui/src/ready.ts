// SPDX-License-Identifier: MIT
import type { JsonValue } from '../../../../packages/plugin-web-sdk/src/index.ts';
export function readiness(payload: JsonValue): boolean | null {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload) || payload.schemaVersion !== 1 || typeof payload.ready !== 'boolean') return null;
  return payload.ready;
}
