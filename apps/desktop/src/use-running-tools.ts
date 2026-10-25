// SPDX-License-Identifier: MIT
import { onScopeDispose, readonly, shallowRef, toRaw, toValue, watch, type MaybeRefOrGetter } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import { pluginLocale, type ExtensionContribution, type ResolvedCommand } from './extensions/contributions';
import { rankUsedTools } from './tool-usage';

export type RunningToolsContext = {
  gameId: MaybeRefOrGetter<string | null | undefined>;
  launchState: MaybeRefOrGetter<string | null | undefined>;
  active: MaybeRefOrGetter<boolean>;
};
/** Native metadata only; command resolution still rechecks the exact game and menu. */
export function useRunningTools(options: RunningToolsContext) {
  const commands = shallowRef<ResolvedCommand[]>([]), loading = shallowRef(false), resolving = shallowRef(false), error = shallowRef('');
  const issued = new WeakMap<object, { revision: number; position: 'tools' | 'instance' }>();
  let revision = 0, resolveRevision = 0, disposed = false;
  const context = () => ({ gameId: toValue(options.gameId) ?? '', launchState: toValue(options.launchState) ?? '', active: toValue(options.active), locale: pluginLocale() });
  const signature = () => JSON.stringify(context());
  const current = (version: number, key: string) => !disposed && version === revision && key === signature();

  async function refresh(): Promise<void> {
    const version = ++revision, key = signature(), selected = context();
    ++resolveRevision; commands.value = []; loading.value = false; resolving.value = false; error.value = '';
    if (disposed || !selected.active || !selected.gameId) return;
    loading.value = true;
    try {
      const contributions = await invoke<ExtensionContribution[]>('list_extension_contributions', { locale: selected.locale, context: { instanceId: selected.gameId } });
      if (!current(version, key)) return;
      const eligible = contributions.flatMap(plugin => plugin.commands.flatMap(command => {
        // Native already evaluates enabled plugins, `when`, and current game state.
        const menu = command.menus.find(menu => menu.position === 'instance') ?? command.menus.find(menu => menu.position === 'tools');
        if (!menu || menu.position === 'blueprint') return [];
        const value: ResolvedCommand = { pluginId: plugin.pluginId, commandId: command.id, label: command.label, workflow: command.workflow, instanceId: selected.gameId };
        issued.set(value, { revision: version, position: menu.position });
        return [value];
      }));
      commands.value = rankUsedTools(eligible);
    } catch {
      if (current(version, key)) error.value = t('extensionCommands.loadError');
    } finally {
      if (current(version, key)) loading.value = false;
    }
  }
  /** Pass an item from commands. Stale listings and copied/invented bindings are rejected. */
  async function resolve(command: ResolvedCommand): Promise<ResolvedCommand | null> {
    const selected = context(), metadata = issued.get(toRaw(command)), version = revision, key = signature();
    if (disposed || !selected.active || !selected.gameId || loading.value || resolving.value || command.instanceId !== selected.gameId || metadata?.revision !== version) return null;
    const operation = ++resolveRevision;
    resolving.value = true; error.value = '';
    try {
      const result = await invoke<ResolvedCommand>('resolve_extension_command', { pluginId: command.pluginId, commandId: command.commandId, position: metadata.position, locale: selected.locale, context: { instanceId: selected.gameId } });
      if (!current(version, key) || operation !== resolveRevision) return null;
      if (result.pluginId !== command.pluginId || result.commandId !== command.commandId || result.instanceId !== selected.gameId || result.workflow !== command.workflow) {
        error.value = t('extensionCommands.commandError'); return null;
      }
      return result;
    } catch {
      if (current(version, key) && operation === resolveRevision) error.value = t('extensionCommands.commandError');
      return null;
    } finally {
      if (current(version, key) && operation === resolveRevision) resolving.value = false;
    }
  }
  watch(() => [toValue(options.gameId), toValue(options.launchState), toValue(options.active), locale.value], () => { void refresh(); }, { immediate: true, flush: 'sync' });
  onScopeDispose(() => { disposed = true; ++revision; ++resolveRevision; commands.value = []; loading.value = false; resolving.value = false; });
  return { commands: readonly(commands), loading: readonly(loading), resolving: readonly(resolving), error: readonly(error), refresh, resolve };
}
