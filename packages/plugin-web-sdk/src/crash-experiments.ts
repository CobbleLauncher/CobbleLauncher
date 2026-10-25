// SPDX-License-Identifier: MIT
import type { Transport } from './index';
import type { TrialCopyPreview } from './trial-copy';
export type CrashExperimentPreview = TrialCopyPreview & { copyGameId: string; disabledMods: { id: string; name: string }[]; requestedModIds: string[] };
export type CrashExperimentInput = { gameId: string; modIds: string[]; includeWorlds?: boolean };
function id(value: string) { if (typeof value !== 'string' || !/^[a-zA-Z0-9_-]{1,128}$/u.test(value)) throw Error('EXPERIMENT_REQUEST_INVALID'); return value; }
function mod(value: string) { if (typeof value !== 'string' || !value.endsWith('.jar') || value.length > 200 || /[/\\:\u0000-\u001f\u007f]/u.test(value)) throw Error('EXPERIMENT_REQUEST_INVALID'); return value; }
function target(args: {gameId:string;copyGameId:string}) { const gameId=id(args.gameId),copyGameId=id(args.copyGameId);if(gameId===copyGameId)throw Error('EXPERIMENT_REQUEST_INVALID');return {gameId,copyGameId}; }
export function createCrashExperiments(transport: Transport) {
  return Object.freeze({
    preview: (args: CrashExperimentInput) => { if (!Array.isArray(args.modIds)||!args.modIds.length||args.modIds.length>512||args.includeWorlds!==undefined&&typeof args.includeWorlds!=='boolean') throw Error('EXPERIMENT_REQUEST_INVALID');return transport({method:'recovery.experimentPreview',params:{gameId:id(args.gameId),modIds:[...new Set(args.modIds.map(mod))],includeWorlds:args.includeWorlds??false}}) as Promise<CrashExperimentPreview>; },
    open: (args:{gameId:string;copyGameId:string}) => transport({method:'recovery.experimentOpen',params:target(args)}) as Promise<{requested:true}>,
    cleanup: (args:{gameId:string;copyGameId:string}) => transport({method:'trialCopy.cleanup',params:target(args)}) as Promise<{removed:boolean}>,
  });
}
