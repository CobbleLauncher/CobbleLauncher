// SPDX-License-Identifier: MIT
import type { LoaderKind } from '../games';
import type { EnvironmentMod } from './types';
import type { ReverseConfiguration } from './types';

export const reverseModFixtures: EnvironmentMod[] = [
  { source: 'modrinth', project_id: 'sodium', title: 'Sodium', description: 'A modern rendering engine and client optimization Mod.', author: 'CaffeineMC', downloads: 1000000 },
  { source: 'modrinth', project_id: 'iris', title: 'Iris Shaders', description: 'Shader support for a beautiful Minecraft world.', author: 'IrisShaders', downloads: 500000 },
];
const modrinthConfigurationFixture = {
  choices: [
    { plan_id: 'reviewed-fabric-plan', game_version: '1.21.1', loader: 'fabric', roots: [{ project_id: 'sodium', version_id: 'sodium-0.6' }, { project_id: 'iris', version_id: 'iris-1.8' }], plan: { project_id: 'sodium', version_id: 'sodium-0.6', mods: [
      { project_id: 'sodium', version_id: 'sodium-0.6', name: 'Sodium', version_number: '0.6.13+mc1.21.1', required_by: null },
      { project_id: 'iris', version_id: 'iris-1.8', name: 'Iris Shaders', version_number: '1.8.8+1.21.1', required_by: null },
      { project_id: 'fabric-api', version_id: 'fabric-api-0.116', name: 'Fabric API', version_number: '0.116.1+1.21.1', required_by: 'Iris Shaders' },
    ] } },
    { plan_id: 'reviewed-neoforge-plan', game_version: '1.21.1', loader: 'neoforge', roots: [{ project_id: 'sodium', version_id: 'sodium-neo' }, { project_id: 'iris', version_id: 'iris-neo' }], plan: { project_id: 'sodium', version_id: 'sodium-neo', mods: [
      { project_id: 'sodium', version_id: 'sodium-neo', name: 'Sodium', version_number: '0.6.13+mc1.21.1-neoforge', required_by: null },
      { project_id: 'iris', version_id: 'iris-neo', name: 'Iris Shaders', version_number: '1.8.8+1.21.1-neoforge', required_by: null },
    ] } },
  ],
  conflicts: [],
  stats: { metadata_requests: 3, candidate_versions: 30, targets_considered: 2, metadata_elapsed_ms: 500, solve_elapsed_ms: 2 },
  limitations: [],
};

export const reverseConfigurationFixture: ReverseConfiguration = {
  ...modrinthConfigurationFixture,
  choices: modrinthConfigurationFixture.choices.map(choice => {
    const roots = choice.roots.map(root => ({ ...root, source: 'modrinth' as const }));
    return { ...choice, loader: choice.loader as LoaderKind, roots, plan: { game_version: choice.game_version, loader: choice.loader, roots, modrinth: choice.plan, curseforge: null, dependency_edges: [] } };
  }),
};
