// SPDX-License-Identifier: MIT
import type {PlayManifestPreview} from '../../../../packages/plugin-web-sdk/src/play-manifest';
export function canExport(preview:PlayManifestPreview|null,busy:boolean,reviewed:boolean,omitAccepted:boolean){return !!preview&&!busy&&reviewed&&(!preview.excludedMods.length||omitAccepted);}
