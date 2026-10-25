// SPDX-License-Identifier: MIT

import { MATERIALS_IMPORT_INTERFACE, MATERIALS_IMPORT_VERSION, validateMaterialsPayload, type MaterialsPayload, type MaterialsHandoffDraft } from './materials.ts';

/** The first typed cross-plugin handoff supported by the launcher. */
export const WAYPOINT_IMPORT_INTERFACE = 'launcher:waypoint-import' as const;
export const WAYPOINT_IMPORT_VERSION = '0.1.0' as const;
export type WaypointDimension = 'overworld' | 'nether' | 'end';
export type WaypointPayload = {
  name: string;
  dimension: WaypointDimension;
  x: number;
  y?: number;
  z: number;
  source?: string;
};
export type HandoffReceiver = { pluginId: string; name: string };
export type WaypointHandoffDraft = {
  handoffId: string;
  sender: { pluginId: string; name: string };
  game: { id: string; name: string };
  worldId?: string | null;
  payload: WaypointPayload;
};
type WaypointInterface = { interfaceId: typeof WAYPOINT_IMPORT_INTERFACE; version: typeof WAYPOINT_IMPORT_VERSION };
type MaterialsInterface = { interfaceId: typeof MATERIALS_IMPORT_INTERFACE; version: typeof MATERIALS_IMPORT_VERSION };
type Interface = WaypointInterface | MaterialsInterface;
type SendInput = (WaypointInterface & { targetPluginId: string; gameId: string; worldId?: string; payload: WaypointPayload }) | (MaterialsInterface & { targetPluginId: string; gameId?: string; worldId?: string; payload: MaterialsPayload });
export type HandoffRequest =
  | { method: 'handoffs.receivers'; params: Interface }
  | { method: 'handoffs.send'; params: SendInput }
  | { method: 'handoffs.take'; params: Interface };
export type HandoffTransport = (request: HandoffRequest) => Promise<unknown>;

function boundedText(value: unknown, max: number, code: string, required = false, maxCharacters = max): string {
  if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > max || [...value].length > maxCharacters || /[\u0000-\u001f\u007f]/u.test(value) || (required && !value.trim())) throw new Error(code);
  return value;
}
function identifier(value: string, code = 'INVALID_HANDOFF_CONTEXT'): string {
  if (!value || value.length > 128 || !/^[a-zA-Z0-9._-]+$/u.test(value)) throw new Error(code);
  return value;
}
function worldIdentifier(value: string): string {
  if (!value || value.length > 128 || /[/\\\u0000-\u001f\u007f]/u.test(value) || value === '.' || value === '..') throw new Error('INVALID_HANDOFF_CONTEXT');
  return value;
}
export function validateWaypointPayload(value: WaypointPayload): WaypointPayload {
  if (!value || typeof value !== 'object' || !['overworld', 'nether', 'end'].includes(value.dimension)) throw new Error('INVALID_WAYPOINT');
  const coord = (number: number) => Number.isSafeInteger(number);
  if (!coord(value.x) || Math.abs(value.x) > 29_999_984 || !coord(value.z) || Math.abs(value.z) > 29_999_984) throw new Error('INVALID_WAYPOINT_COORDINATES');
  if (value.y !== undefined && (!coord(value.y) || value.y < -4096 || value.y > 4096)) throw new Error('INVALID_WAYPOINT_COORDINATES');
  return {
    name: boundedText(value.name, 320, 'INVALID_WAYPOINT', true, 80),
    dimension: value.dimension,
    x: value.x,
    ...(value.y === undefined ? {} : { y: value.y }),
    z: value.z,
    ...(value.source === undefined ? {} : { source: boundedText(value.source, 480, 'INVALID_WAYPOINT', false, 120) }),
  };
}
function interfaceArgs(value: Interface): Interface {
  if (![WAYPOINT_IMPORT_INTERFACE, MATERIALS_IMPORT_INTERFACE].includes(value.interfaceId) || value.version !== WAYPOINT_IMPORT_VERSION) throw new Error('UNSUPPORTED_HANDOFF_INTERFACE');
  return { interfaceId: value.interfaceId, version: value.version };
}

/** Typed payloads; authorization and one-shot delivery stay in the host. */
export function createHandoffs(transport: HandoffTransport) {
  function take<T extends Interface>(input: T): Promise<(T extends MaterialsInterface ? MaterialsHandoffDraft : WaypointHandoffDraft) | null> {
    return transport({method: 'handoffs.take', params: interfaceArgs(input)}) as ReturnType<typeof take<T>>;
  }
  return Object.freeze({
    receivers: (input: Interface) => transport({ method: 'handoffs.receivers', params: interfaceArgs(input) }) as Promise<HandoffReceiver[]>,
    send: (input: SendInput) => {
      interfaceArgs(input);
      if ((input.interfaceId === WAYPOINT_IMPORT_INTERFACE && !input.gameId) || (input.worldId !== undefined && !input.gameId)) throw new Error('INVALID_HANDOFF_CONTEXT');
      const params = {...input, targetPluginId: identifier(input.targetPluginId), ...(input.gameId === undefined ? {} : {gameId: identifier(input.gameId)}), ...(input.worldId === undefined ? {} : {worldId: worldIdentifier(input.worldId)}), payload: input.interfaceId === MATERIALS_IMPORT_INTERFACE ? validateMaterialsPayload(input.payload) : validateWaypointPayload(input.payload)} as SendInput;
      return transport({method: 'handoffs.send', params}) as Promise<{requested: true}>;
    },
    take,
  });
}
