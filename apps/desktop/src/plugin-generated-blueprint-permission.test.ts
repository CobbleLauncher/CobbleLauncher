// SPDX-License-Identifier: MIT
import { expect,it } from 'vitest';
import manifest from '../../../examples/geometry-builder/plugin.json';
import { isSupportedCatalogPlugin,type CatalogManifest } from './plugin-catalogs';
import { permissionTone,permissionCategory } from './extensions/permission-review';
it('accepts generated-only local frontend with a visible write grant',()=>{
 const value=manifest as CatalogManifest;
 expect(isSupportedCatalogPlugin({manifest:value,description:''})).toBe(true);
 expect(isSupportedCatalogPlugin({manifest:{...value,frontend:undefined},description:''})).toBe(false);
 const node={permission:'launcher:blueprints/generated-save'} as const;
 expect(permissionTone(node)).toBe('write');expect(permissionCategory(node)).toBe('exports');
});
