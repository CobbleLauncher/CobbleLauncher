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
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import PermissionConsentCard from './ui/PermissionConsentCard.vue';
import type { ExtensionCandidate } from './navigation';
const props=defineProps<{candidate?:ExtensionCandidate}>();
const emit = defineEmits<{ openDetail: [id: string] }>();
const section = ref('installed');
const search = ref('');
const openPluginMenu = ref<string | null>(null);
type Plugin = { manifest: CatalogManifest; enabled: boolean; problem?: string };
const plugins = ref<Plugin[]>([]);
watch(()=>props.candidate, candidate=>{if(candidate){section.value='sources';search.value=candidate.pluginId;}},{immediate:true});
const visiblePlugins = computed(() => plugins.value.filter(plugin => matchesPluginMetadata(plugin.manifest, search.value)));
const installedPages = usePluginListPagination(visiblePlugins, [search, plugins]);
const pending = ref<CatalogManifest | null>(null); const pendingUpdateFrom = ref<CatalogManifest | null>(null); const pendingSource = ref(''); const busy = ref(false); const error = ref('');
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
  busy.value = true; error.value = '';
  try { plugins.value = await invoke<Plugin[]>(command, args); pending.value = null; pendingUpdateFrom.value = null; pendingSource.value = ''; loadFailed.value = false; }
  catch (cause) { const message=String(cause); error.value=message.includes('PLUGIN_REVIEW_MISMATCH')?t('pluginPermissions.reviewChanged'):message.includes('PLUGIN_PERMISSION_APPROVAL_REQUIRED')?t('pluginPermissions.reviewRequired'):message; loadFailed.value = command === 'list_plugins'; }
  finally { busy.value = false; }
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
    <TabsRoot v-model="section">
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
        <label class="extensions-search">
          <AppIcon name="search" />
          <input
            v-model="search"
            type="search"
            :aria-label="t('plugins.search')"
            :placeholder="t('plugins.search')"
          >
          <AppIconButton
            v-if="search"
            icon="x"
            :label="t('tools.clearSearch')"
            @click="search = ''"
          />
        </label>
        <button
          class="text-button extensions-install"
          :disabled="busy"
          @click="pick"
        >
          <AppIcon name="folder" />{{ t('plugins.installLocal') }}
        </button>
      </div>
      <TabsContent
        v-show="!pending"
        value="installed"
      >
        <PluginUpdatesPanel
          :installed="plugins.map(plugin => plugin.manifest)"
          :disabled="busy || !!pending"
          @preparing="setCatalogPreparing"
          @prepared="prepareCatalog"
          @updated="run('list_plugins')"
        />
        <ul class="plugin-list">
          <ContextMenuRoot
            v-for="plugin in installedPages.pageItems.value"
            :key="plugin.manifest.id"
          >
            <ContextMenuTrigger as-child>
              <li>
                <div class="plugin-row-identity">
                  <button
                    class="plugin-identity-button"
                    type="button"
                    :aria-label="`${t('pluginDetails.details')}: ${plugin.manifest.name}`"
                    :disabled="busy"
                    @click="emit('openDetail', plugin.manifest.id)"
                  >
                    <PluginIdentity
                      :manifest="plugin.manifest"
                      fetch-icon
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
                        :disabled="busy"
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
                          :disabled="busy"
                          @select="emit('openDetail', plugin.manifest.id)"
                        >
                          <AppIcon name="info" />{{ t('pluginDetails.details') }}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          class="profile-menu-item danger-text"
                          :disabled="busy"
                          @select="run('uninstall_plugin', { id: plugin.manifest.id })"
                        >
                          <AppIcon name="trash" />{{ t('plugins.uninstall') }}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenuPortal>
                  </DropdownMenuRoot>
                  <button
                    class="mod-toggle plugin-enable-toggle"
                    type="button"
                    role="switch"
                    :aria-label="t('plugins.enabledState', { name: plugin.manifest.name })"
                    :aria-checked="plugin.enabled"
                    :disabled="busy || (Boolean(plugin.problem) && !(plugin.enabled && plugin.problem === 'PLUGIN_MISSING_INTERFACE'))"
                    @click="run('set_plugin_enabled', { id: plugin.manifest.id, enabled: !plugin.enabled })"
                  >
                    <span />
                  </button>
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
                  :disabled="busy"
                  @select="emit('openDetail', plugin.manifest.id)"
                >
                  <AppIcon name="info" />{{ t('pluginDetails.details') }}
                </ContextMenuItem>
                <ContextMenuItem
                  class="profile-menu-item danger-text"
                  :disabled="busy"
                  @select="run('uninstall_plugin', { id: plugin.manifest.id })"
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
      </TabsContent>
      <TabsContent
        v-show="!pending"
        value="discover"
      >
        <BundledPluginsPanel
          :disabled="busy"
          :search="search"
          :installed-ids="plugins.map(plugin => plugin.manifest.id)"
          :installed-manifests="plugins.map(plugin => plugin.manifest)"
          @prepared="prepareCatalog"
          @preparing="setCatalogPreparing"
        />
      </TabsContent>
      <TabsContent
        v-show="!pending && section === 'sources'"
        value="sources"
        force-mount
      >
        <PluginCatalogsPanel
          :candidate="props.candidate"
          :disabled="busy"
          :search="search"
          :installed-manifests="plugins.map(plugin => plugin.manifest)"
          @prepared="prepareCatalog"
          @preparing="setCatalogPreparing"
        />
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
  </div>
</template>
<style scoped>
.extensions-tabs{display:flex;gap:6px;margin-bottom:20px}.extensions-tabs button{border:0;border-radius:8px;padding:9px 14px;background:transparent;color:var(--muted)}.extensions-tabs button[data-state=active]{background:var(--hover);color:var(--ink)}
.plugin-list{padding:0;list-style:none}.plugin-list li{display:flex;align-items:center;flex-wrap:wrap;gap:12px;padding:18px 0;border-bottom:1px solid var(--line)}.plugin-list li>.plugin-row-identity{flex:1;min-width:160px}.plugin-identity-button{display:block;width:100%;padding:0;border:0;background:transparent;color:inherit;text-align:left;cursor:pointer}.plugin-identity-button:focus-visible{outline:var(--ui-focus-ring,2px solid var(--ui-focus));outline-offset:var(--ui-focus-offset,3px);border-radius:8px}.plugin-identity-button:disabled{cursor:default}.plugins-panel h2{font-size:14px;margin:0}.plugin-approval{padding:20px 0}
.plugin-list li>.plugin-report-panel{flex:0 0 100%;min-width:0}
.extensions-toolbar{display:flex;align-items:center;flex-wrap:wrap;gap:12px;margin:16px 0}.extensions-search{display:flex;align-items:center;gap:10px;min-height:42px;flex:1;min-width:180px;padding:0 11px;border:1px solid var(--line);border-radius:9px;background:var(--panel);color:var(--muted);box-sizing:border-box}.extensions-search>svg{width:17px;height:17px;flex:none}.extensions-search input{flex:1;min-width:0;width:100%;border:0;outline:0;background:transparent;color:var(--ink);font:inherit;font-size:13px}.extensions-search input:focus-visible{outline:0}.extensions-search input::-webkit-search-decoration,.extensions-search input::-webkit-search-cancel-button{appearance:none}.extensions-search:focus-within{border-color:var(--accent,#87a65b);box-shadow:0 0 0 2px color-mix(in srgb,var(--accent,#87a65b) 20%,transparent)}.extensions-search :deep(.ui-icon-button){min-height:28px;width:28px}.extensions-install{display:inline-flex;align-items:center;gap:6px;min-height:42px;font-size:12px}.plugin-list>li>.plugin-row-identity{min-width:0;flex:1}.plugin-list>li>.plugin-row-actions{flex:0 0 auto;min-width:0}.plugin-row-actions{display:flex;align-items:center;gap:var(--ui-space-2)}.plugin-enable-toggle{flex:0 0 32px}
</style>

<style scoped>.plugin-permission-expansion{display:grid;gap:10px;margin:16px 0}.plugin-permission-expansion h3{font-size:14px;margin:0}.plugin-permission-expansion :deep(.plugin-permissions){margin:0}</style>
<style scoped>
.plugin-approval{max-width:760px;padding:4px 0 20px}.approval-header{display:flex;align-items:center;gap:14px;justify-content:space-between}.approval-header :deep(.plugin-identity){min-width:0}.approval-version{flex:none;font-size:12px;color:var(--muted);padding:5px 8px;border-radius:7px;background:var(--hover)}.approval-source{display:flex;gap:6px 16px;flex-wrap:wrap;margin:10px 0 0;font-size:12px;color:var(--muted);overflow-wrap:anywhere}.plugin-approval :deep(.plugin-signature-status){font-size:12px;margin:8px 0 20px}.plugin-approval .java-actions{margin-top:20px}
</style>
