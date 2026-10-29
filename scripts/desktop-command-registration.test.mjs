// SPDX-License-Identifier: MIT
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = name => readFileSync(path.join(root, 'apps/desktop/src-tauri', name), 'utf8');
const build = read('build.rs');
const handlers = read('src/main.rs');
const main = JSON.parse(read('capabilities/main.json'));

test('shared changes and device handoff are authorized only in the main webview', () => {
  const commands = ["preview_game_change_export","export_game_change","import_game_change","preview_game_change_apply","apply_game_change","undo_game_change_new_config","game_transfer_context","game_transfer_prepare","game_transfer_send","game_transfer_cancel","game_transfer_offers","game_transfer_list_outgoing","game_transfer_status","game_transfer_accept","game_transfer_reject","game_transfer_discard"];
  for (const command of commands) {
    assert(build.includes(`"${command}"`), `missing generated permission: ${command}`);
    assert.match(handlers, new RegExp(`\\b${command}\\b`), `missing handler: ${command}`);
    assert(main.permissions.includes(`allow-${command.replaceAll('_', '-')}`), `main webview cannot invoke ${command}`);
  }
  assert.deepEqual(main.webviews, ['main']);
  assert.equal(main.remote, undefined);
});

test('daily workflow IPC is registered and authorized for the main webview', () => {
  for (const command of ['start_game_pack_update_copy', 'set_game_mod_pinned', 'get_active_task_count', 'cancel_active_tasks', 'get_onboarding_state', 'save_onboarding_state', 'launcher_update_status', 'set_launcher_update_preferences', 'check_launcher_update', 'install_launcher_update']) {
    assert(build.includes(`"${command}"`), `missing generated permission: ${command}`);
    assert.match(handlers, new RegExp(`\\b${command}\\b`), `missing handler: ${command}`);
    assert(main.permissions.includes(`allow-${command.replaceAll('_', '-')}`), `main webview cannot invoke ${command}`);
  }
});

test('native mutation permissions stay scoped to the main webview', () => {
  assert.deepEqual(main.webviews, ['main']);
  assert.equal(main.remote, undefined);
});

test('reverse configuration commands are registered for the main webview', () => {
  for (const command of ['search_reverse_mods', 'reverse_configure_mods', 'cancel_reverse_configuration', 'create_game_from_mods']) {
    assert(build.includes(`"${command}"`), `missing generated permission: ${command}`);
    assert.match(handlers, new RegExp(`\\b${command}\\b`), `missing handler: ${command}`);
    assert(main.permissions.includes(`allow-${command.replaceAll('_', '-')}`), `main webview cannot invoke ${command}`);
  }
});

test('environment, readiness and background-copy workflows have complete IPC registrations', () => {
  const commands = [
    'search_environment_mods', 'reverse_configure_environment', 'cancel_environment_configuration',
    'preview_environment_installation', 'select_environment_manual_file', 'create_game_from_environment',
    'get_task_environment_requirements', 'select_task_environment_manual_file',
    'inspect_game_readiness', 'preview_game_readiness', 'prepare_game_readiness', 'cancel_game_readiness_check',
    'start_game_copy', 'get_game_mod_update_policy', 'set_game_mod_update_policy', 'list_game_maintenance',
    'preview_game_curseforge_updates', 'mesh_route_policy_save', 'mesh_node_start', 'mesh_node_status', 'mesh_node_stop',
    'pick_game_java_candidate', 'pick_java_candidate', 'pick_default_game_directory_candidate', 'get_standard_game_directory',
    'get_default_game_directory_candidate', 'discover_launcher_import',
    'preview_game_directory_migration', 'start_game_directory_migration',
    'open_game_directory_migration_source', 'inspect_game_directory_migration',
    'cleanup_game_directory_migration_source', 'get_launcher_import_result',
    ...['share_preview', 'share_publish', 'receive', 'prepare', 'manual_file', 'create'].map(suffix => `mesh_environment_${suffix}`),
  ];
  for (const command of commands) {
    assert(build.includes(`"${command}"`), `missing generated permission: ${command}`);
    assert.match(handlers, new RegExp(`\\b${command}\\b`), `missing handler: ${command}`);
    assert(main.permissions.includes(`allow-${command.replaceAll('_', '-')}`), `main webview cannot invoke ${command}`);
  }
});

test('home resources and original screenshot clipboard stay in the trusted webview', () => {
  for (const command of ['get_home_world_summary', 'copy_game_screenshot']) {
    assert(build.includes(`"${command}"`));
    assert(handlers.includes(command));
    assert(main.permissions.includes(`allow-${command.replaceAll('_', '-')}`));
  }
  assert.deepEqual(main.webviews, ['main']);
});
