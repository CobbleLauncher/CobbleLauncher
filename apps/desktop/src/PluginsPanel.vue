<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import PluginCatalogsPanel from './PluginCatalogsPanel.vue';
import PluginUpdatesPanel from './PluginUpdatesPanel.vue';
import BundledPluginsPanel from './BundledPluginsPanel.vue';
import { TabsRoot, TabsList, TabsTrigger, TabsContent, DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem } from 'reka-ui';
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { matchesPluginMetadata, type CatalogManifest } from './plugin-catalogs';
import PluginPermissions from './PluginPermissions.vue';
import {addedPermissionNodes, effectivePermissionNodes} from './plugin-permissions';
import PluginSensitivePermissions from './PluginSensitivePermissions.vue';
import PluginSignatureStatus from './PluginSignatureStatus.vue';
import PluginPermissionBadges from './PluginPermissionBadges.vue';
import PluginListPagination from './PluginListPagination.vue';
import { usePluginListPagination } from './plugin-list-pagination';
import { permissionConsentKey, requiresSensitiveConsent } from './extensions/permission-review';
import PluginIdentity from './PluginIdentity.vue';
import { pluginFallbackIcon } from './plugin-presentation';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import SearchField from './ui/SearchField.vue';
import SwitchField from './ui/SwitchField.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import PermissionConsentCard from './ui/PermissionConsentCard.vue';
import ConfirmationDialog from './ui/ConfirmationDialog.vue';
import type { ExtensionCandidate } from './navigation';
const props=defineProps<{candidate?:ExtensionCandidate}>();
const emit = defineEmits<{ openDetail: [id: string] }>();
const section = ref('installed');
const discoverVisited = ref(false);
watch(section, value => { if (value === 'discover') discoverVisited.value = true; }, {immediate: true});
const search = ref('');
const openPluginMenu = ref<string | null>(null);
type Plugin = { manifest: CatalogManifest; enabled: boolean; problem?: string };
const plugins = ref<Plugin[]>([]);
watch(()=>props.candidate, candidate=>{if(candidate){section.value=candidate.sourceId==='bundled'?'discover':'sources';search.value=candidate.pluginId;}},{immediate:true});
const visiblePlugins = computed(() => plugins.value.filter(plugin => matchesPluginMetadata(plugin.manifest, search.value)));
const installedPages = usePluginListPagination(visiblePlugins, [search, plugins]);
const pending = ref<CatalogManifest | null>(null); const pendingUpdateFrom = ref<CatalogManifest | null>(null); const pendingSource = ref(''); const busy = ref(false); const error = ref('');
const busyPluginId = ref('');
const removing = ref<CatalogManifest | null>(null);
function uninstall() { if (!removing.value || busy.value) return; void run('uninstall_plugin', {id:removing.value.id}); }
const permissionApproved=ref(false);
const addedPermissions=computed(()=>pending.value&&pendingUpdateFrom.value?addedPermissionNodes(pending.value,pendingUpdateFrom.value):[]);
const sensitiveAccepted = ref<string[]>([]);
const signatureReady = ref(false);
const sensitivePermissions = computed(() => (pendingUpdateFrom.value ? addedPermissions.value
  : pending.value ? effectivePermissionNodes(pending.value) : []).filter(requiresSensitiveConsent));
const ordinaryAddedPermissions = computed(() => addedPermissions.value.filter(node => !requiresSensitiveConsent(node)));
const ordinaryReadOnlyPermissions = computed(() => pending.value ? effectivePermissionNodes(pending.value).filter(node => !requiresSensitiveConsent(node)
  && !ordinaryAddedPermissions.value.some(added => added.permission === node.permission)) : []);
const consentComplete = computed(() => (!ordinaryAddedPermissions.value.length || permissionApproved.value)
  && sensitivePermissions.value.every(node => sensitiveAccepted.value.includes(permissionConsentKey(node))));
watch([pending,pendingUpdateFrom],()=>{permissionApproved.value=false;sensitiveAccepted.value=[];signatureReady.value=false;},{flush:'sync'});
const loadFailed = ref(false);
const approval = ref<HTMLElement | null>(null);
async function run(command: string, args?: Record<string, unknown>) {
  busy.value = true; busyPluginId.value = typeof args?.id === 'string' ? args.id : ''; error.value = '';
  try { plugins.value = await invoke<Plugin[]>(command, args); pending.value = null; pendingUpdateFrom.value = null; pendingSource.value = ''; loadFailed.value = false; }
  catch (cause) { const message=String(cause); error.value=message.includes('PLUGIN_REVIEW_MISMATCH')?t('pluginPermissions.reviewChanged'):message.includes('PLUGIN_PERMISSION_APPROVAL_REQUIRED')?t('pluginPermissions.reviewRequired'):message; loadFailed.value = command === 'list_plugins'; }
  finally { busy.value = false; busyPluginId.value = ''; }
}
async function pick() { busy.value = true; error.value = ''; pendingSource.value = ''; pendingUpdateFrom.value = null; try { pending.value = await invoke<CatalogManifest | null>('preview_plugin_package'); pendingUpdateFrom.value=plugins.value.find(plugin=>plugin.manifest.id===pending.value?.id)?.manifest??null; } catch(cause) { error.value=String(cause); } finally { busy.value=false; } }
function prepareCatalog(value: { manifest: CatalogManifest; sourceName: string; updateFrom?: CatalogManifest }) {
  pending.value = value.manifest; pendingSource.value = value.sourceName; pendingUpdateFrom.value = value.updateFrom ?? null; error.value = '';
  void nextTick(() => { if (approval.value && typeof approval.value.scrollIntoView === 'function') approval.value.scrollIntoView(); });
}
function setCatalogPreparing(value: boolean) {
  if (value) { pending.value = null; pendingSource.value = ''; pendingUpdateFrom.value = null; }
  busy.value = value;
}
async function cancelPending() {
  busy.value = true; error.value = '';
  try { await invoke('cancel_plugin_preview'); pending.value = null; pendingSource.value = ''; pendingUpdateFrom.value = null; }
  catch (cause) { error.value = String(cause); }
  finally { busy.value = false; }
}
onMounted(() => { void run('list_plugins'); });
function publisherChange() {
  const previous = pendingUpdateFrom.value;
  return previous && pending.value && previous.publisher !== pending.value.publisher
    ? t('plugins.publisherChanged', { old: previous.publisher, current: pending.value.publisher })
    : '';
}
async function approvePending() {
  if (busy.value || !pending.value || !consentComplete.value || !signatureReady.value) return;
  await run(pendingUpdateFrom.value ? 'update_plugin' : 'install_plugin', {
    id: pending.value.id, reviewedManifest: pending.value,
  });
}
</script>
<template>
  <div class="plugins-panel page-content">
    <TabsRoot
      v-model="section"
      :unmount-on-hide="false"
    >
      <TabsList
        v-show="!pending"
        class="extensions-tabs"
        :aria-label="t('navigation.extensions')"
      >
        <TabsTrigger value="installed">
          {{ t('navigation.installed') }}
        </TabsTrigger>
        <TabsTrigger value="discover">
          {{ t('navigation.discover') }}
        </TabsTrigger>
        <TabsTrigger value="sources">
          {{ t('navigation.sources') }}
        </TabsTrigger>
      </TabsList>

      <section
        v-if="pending"
        ref="approval"
        class="plugin-approval"
      >
        <header class="approval-header">
          <PluginIdentity
            :manifest="pending"
            :show-description="false"
            :show-tags="false"
          />
          <span class="approval-version">{{ pendingUpdateFrom ? `${pendingUpdateFrom.version} → ${pending.version}` : pending.version }}</span>
        </header>
        <p class="approval-source">
          <span>{{ pending.publisher }}</span>
          <span v-if="pendingSource">{{ t('plugins.catalogSourceAttribution', { source: pendingSource }) }}</span>
        </p>
        <p
          v-if="publisherChange()"
          class="settings-error"
          role="status"
        >
          {{ publisherChange() }}
        </p>
        <PluginSignatureStatus
          :manifest="pending"
          @ready="signatureReady = $event"
        />
        <PluginPermissions
          :manifest="pending"
          :nodes="ordinaryReadOnlyPermissions"
        />
        <section
          v-if="ordinaryAddedPermissions.length"
          class="plugin-permission-expansion"
        >
          <h3>{{ t('pluginPermissions.added') }}</h3>
          <PermissionConsentCard
            v-model="permissionApproved"
            :disabled="busy"
            :label="t('pluginPermissions.confirm')"
          >
            <PluginPermissions
              :manifest="pending"
              :nodes="ordinaryAddedPermissions"
            />
          </PermissionConsentCard>
        </section>
        <PluginSensitivePermissions
          v-model="sensitiveAccepted"
          :manifest="pending"
          :nodes="sensitivePermissions"
          :disabled="busy"
        />
        <div class="java-actions">
          <button
            class="primary-button"
            :disabled="busy || !consentComplete || !signatureReady"
            @click="approvePending"
          >
            {{ pendingUpdateFrom ? t('plugins.approveUpdate') : t('plugins.approve') }}
          </button><button
            class="text-button"
            :disabled="busy"
            @click="cancelPending"
          >
            {{ t('plugins.cancel') }}
          </button>
        </div>
      </section>
      <div
        v-show="!pending"
        class="extensions-toolbar"
      >
        <SearchField
          v-model="search"
          class="extensions-search"
          :label="t('plugins.search')"
          :placeholder="t('plugins.search')"
          :clear-label="t('tools.clearSearch')"
        />
        <button
          class="text-button extensions-install"
          :disabled="busy"
          @click="pick"
        >
          <AppIcon name="folder" />{{ t('plugins.installLocal') }}
        </button>
      </div>
      <TabsContent
        value="installed"
        as-child
      >
        <section v-show="!pending">
          <PluginUpdatesPanel
            :installed="plugins.map(plugin => plugin.manifest)"
            :disabled="busy || !!pending"
            @preparing="setCatalogPreparing"
            @prepared="prepareCatalog"
            @updated="run('list_plugins')"
          />
          <LoadingIndicator
            v-if="busy && !plugins.length"
            :label="t('navigation.loading')"
          />
          <p
            v-else-if="!search.trim() && !plugins.length && !loadFailed"
            class="content-empty"
          >
            {{ t('plugins.noneInstalled') }}
          </p>
          <ul class="plugin-list">
            <ContextMenuRoot
              v-for="plugin in installedPages.pageItems.value"
              :key="plugin.manifest.id"
            >
              <ContextMenuTrigger as-child>
                <li :aria-busy="busy && busyPluginId === plugin.manifest.id">
                  <div class="plugin-row-identity">
                    <button
                      class="plugin-identity-button"
                      type="button"
                      :aria-label="`${t('pluginDetails.details')}: ${plugin.manifest.name}`"
                      @click="emit('openDetail', plugin.manifest.id)"
                    >
                      <PluginIdentity
                        :manifest="plugin.manifest"
                        :fallback-icon="pluginFallbackIcon(plugin.manifest)"
                        fetch-icon
                        :show-tags="false"
                      />
                    </button>
                    <p class="setting-hint">
                      {{ plugin.manifest.publisher }} · {{ plugin.manifest.version }}
                    </p>
                    <PluginPermissionBadges :manifest="plugin.manifest" />
                    <p
                      v-if="plugin.problem"
                      class="settings-error"
                    >
                      {{ t(plugin.problem === 'PLUGIN_MISSING_INTERFACE' ? 'plugins.missingInterface' : plugin.problem === 'PLUGIN_UNSUPPORTED_API' ? 'plugins.catalogUnsupported' : 'plugins.damaged') }}
                    </p>
                  </div>
                  <div class="plugin-row-actions">
                    <DropdownMenuRoot
                      :open="openPluginMenu === plugin.manifest.id"
                      @update:open="openPluginMenu = $event ? plugin.manifest.id : null"
                    >
                      <DropdownMenuTrigger as-child>
                        <AppIconButton
                          icon="more-horizontal"
                          :label="t('plugins.moreActions', { name: plugin.manifest.name })"
                          :disabled="busy && busyPluginId === plugin.manifest.id"
                          :aria-expanded="openPluginMenu === plugin.manifest.id"
                        />
                      </DropdownMenuTrigger>
                      <DropdownMenuPortal>
                        <DropdownMenuContent
                          class="profile-menu"
                          align="end"
                          :side-offset="6"
                          :collision-padding="12"
                        >
                          <DropdownMenuItem
                            class="profile-menu-item"
                            @select="emit('openDetail', plugin.manifest.id)"
                          >
                            <AppIcon name="info" />{{ t('pluginDetails.details') }}
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            class="profile-menu-item danger-text"
                            :disabled="busy"
                            @select="removing = plugin.manifest"
                          >
                            <AppIcon name="trash" />{{ t('plugins.uninstall') }}
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenuPortal>
                    </DropdownMenuRoot>
                    <AppIcon
                      v-if="busy && busyPluginId === plugin.manifest.id"
                      name="loader-circle"
                      class="ui-busy-icon"
                    />
                    <SwitchField
                      :model-value="plugin.enabled"
                      :label="t('plugins.enabledState', { name: plugin.manifest.name })"
                      :disabled="busy || (Boolean(plugin.problem) && !(plugin.enabled && plugin.problem === 'PLUGIN_MISSING_INTERFACE'))"
                      @update:model-value="run('set_plugin_enabled', { id: plugin.manifest.id, enabled: $event })"
                    />
                  </div>
                </li>
              </ContextMenuTrigger>
              <ContextMenuPortal>
                <ContextMenuContent
                  class="profile-menu"
                  :collision-padding="12"
                >
                  <ContextMenuItem
                    class="profile-menu-item"
                    @select="emit('openDetail', plugin.manifest.id)"
                  >
                    <AppIcon name="info" />{{ t('pluginDetails.details') }}
                  </ContextMenuItem>
                  <ContextMenuItem
                    class="profile-menu-item danger-text"
                    :disabled="busy"
                    @select="removing = plugin.manifest"
                  >
                    <AppIcon name="trash" />{{ t('plugins.uninstall') }}
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenuPortal>
            </ContextMenuRoot>
          </ul>
          <PluginListPagination
            :page="installedPages.page.value"
            :page-count="installedPages.pageCount.value"
            :total="visiblePlugins.length"
            @page="installedPages.page.value = $event"
          />
          <p
            v-if="search.trim() && !visiblePlugins.length"
            role="status"
            class="setting-hint"
          >
            {{ t('plugins.noMatches') }}
          </p>
        </section>
      </TabsContent>
      <TabsContent
        value="discover"
        as-child
      >
        <section v-show="!pending">
          <BundledPluginsPanel
            v-if="discoverVisited"
            :disabled="busy"
            :search="search"
            :installed-ids="plugins.map(plugin => plugin.manifest.id)"
            :installed-manifests="plugins.map(plugin => plugin.manifest)"
            @prepared="prepareCatalog"
            @preparing="setCatalogPreparing"
          />
        </section>
      </TabsContent>
      <TabsContent
        value="sources"
        as-child
      >
        <section v-show="!pending">
          <PluginCatalogsPanel
            :candidate="props.candidate?.sourceId === 'bundled' ? undefined : props.candidate"
            :disabled="busy"
            :search="search"
            :installed-manifests="plugins.map(plugin => plugin.manifest)"
            @prepared="prepareCatalog"
            @preparing="setCatalogPreparing"
          />
        </section>
      </TabsContent>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <button
        v-if="loadFailed"
        class="text-button"
        :disabled="busy"
        @click="run('list_plugins')"
      >
        {{ t('plugins.retry') }}
      </button>
    </TabsRoot>
    <ConfirmationDialog
      :key="removing?.id"
      :open="!!removing"
      :title="t('plugins.uninstallTitle', { name: removing?.name ?? '' })"
      :description="t('plugins.uninstallWarning')"
      :confirm-label="t('plugins.uninstall')"
      danger
      :disabled="busy"
      @update:open="value=>{if(!value)removing=null;}"
      @confirm="uninstall"
    />
  </div>
</template>
<style scoped>
.extensions-tabs{display:flex;gap:var(--ui-space-5);overflow-x:auto;white-space:nowrap;border-bottom:1px solid var(--ui-border);margin-bottom:var(--ui-space-4)}.extensions-tabs button{flex:none;min-height:var(--ui-control-height);padding:0 0 var(--ui-space-2);border:0;border-bottom:2px solid transparent;background:transparent;font:inherit;font-size:var(--ui-text-label);line-height:var(--ui-leading-label);color:var(--ui-text-muted)}.extensions-tabs button[data-state=active]{border-bottom-color:var(--ui-focus);color:var(--ui-text);font-weight:600}
.extensions-toolbar{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-3);margin:0 0 var(--ui-space-4)}.extensions-search{flex:1;min-width:200px}.extensions-install{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}.plugin-list{margin:0;padding:0;list-style:none}.plugin-list li{display:flex;align-items:center;gap:var(--ui-space-4);padding:var(--ui-space-4) 0;border-bottom:1px solid var(--ui-border);min-width:0}.plugin-row-identity{flex:1;min-width:0}.plugin-identity-button{display:block;width:100%;padding:0;border:0;background:transparent;color:inherit;text-align:left;cursor:pointer}.plugin-identity-button:hover :deep(.plugin-identity-copy>strong){text-decoration:underline;text-underline-offset:3px}.plugin-identity-button:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset);border-radius:var(--ui-radius-md)}.plugin-identity-button :deep(.plugin-identity){gap:var(--ui-space-3)}.plugin-identity-button :deep(.plugin-identity-copy>strong){font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.plugin-identity-button :deep(.plugin-identity-copy>p){font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-row-identity>.setting-hint{margin:var(--ui-space-1) 0 0 56px;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-row-identity :deep(.plugin-permission-badges){margin-left:56px}.plugin-row-actions{display:flex;align-items:center;gap:var(--ui-space-2);flex:none}.plugin-list .settings-error{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
.plugin-approval{max-width:780px;min-width:0;padding:0 0 var(--ui-space-4)}.approval-header{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-4)}.approval-header :deep(.plugin-identity-copy>strong){font-size:var(--ui-text-section);line-height:var(--ui-leading-section)}.approval-version{flex:none;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-variant-numeric:tabular-nums;color:var(--ui-text-muted)}.approval-source{display:flex;flex-wrap:wrap;gap:var(--ui-space-2) var(--ui-space-4);margin:var(--ui-space-2) 0 var(--ui-space-3);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.plugin-permission-expansion{display:grid;gap:var(--ui-space-2);margin:var(--ui-space-5) 0}.plugin-permission-expansion h3{font-size:var(--ui-text-body);line-height:var(--ui-leading-body);font-weight:600;margin:0}.plugin-permission-expansion :deep(.plugin-permissions){margin:0}.plugin-approval>.java-actions{position:sticky;bottom:0;z-index:2;display:flex;align-items:center;gap:var(--ui-space-3);margin-top:var(--ui-space-5);padding:var(--ui-space-3) 0;background:var(--ui-bg);border-top:1px solid var(--ui-border)}.plugin-approval>.java-actions button{min-height:var(--ui-control-height);font-size:var(--ui-text-label)}
@media(max-width:600px){.extensions-tabs{gap:var(--ui-space-4)}.plugin-list li{gap:var(--ui-space-2)}.plugin-row-actions{gap:var(--ui-space-1)}.approval-header{align-items:flex-start;gap:var(--ui-space-2)}}
</style>
