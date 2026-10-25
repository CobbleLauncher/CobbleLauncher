<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, toRef, watch } from 'vue';
import { Users } from '@lucide/vue';
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui';
import AppIcon from './ui/AppIcon.vue';
import MeshIdentityAvatar from './mesh-ui/MeshIdentityAvatar.vue';
import { locale, t } from './i18n';
import { meshProfileLabel } from './mesh-social';
import { useMeshContactsPreview } from './use-mesh-contacts-preview';
const props = defineProps<{ available: boolean; beforeOpen?: () => Promise<boolean> }>();
const emit = defineEmits<{ contact: [peerId: string]; join: []; open: [] }>();
const opened = ref(false);
const { preview, state, refreshing, refresh } = useMeshContactsPreview({ open: opened, available: toRef(props,'available') });
const labels = computed(() => locale.value === 'zh-CN' ? { join: '加入邀请', manage: '管理联系人', failed: '无法读取联系人', retry: '重试', loading: '正在读取…' } : { join: 'Join by invitation', manage: 'Manage contacts', failed: 'Could not read contacts', retry: 'Retry', loading: 'Loading…' });
let opening = 0, disposed = false;
async function updateOpen(value: boolean) {
  const current = ++opening;
  if (value) {
    if (!props.available) return;
    try { if (!await (props.beforeOpen?.() ?? Promise.resolve(true))) return; } catch { return; }
  }
  if (disposed || current !== opening || value && !props.available) return;
  opened.value = value;
}
function select(peerId: string) { void updateOpen(false); emit('contact',peerId); }
function join() { void updateOpen(false); emit('join'); }
function manage() { void updateOpen(false); emit('open'); }
watch(() => props.available, value => { if (!value) void updateOpen(false); });
onUnmounted(() => { disposed = true; ++opening; });
</script>
<template>
  <PopoverRoot
    :open="opened"
    @update:open="updateOpen"
  >
    <PopoverTrigger
      type="button"
      class="mesh-contacts-trigger"
      :disabled="!available"
      :aria-label="t('meshSocial.contacts')"
      :title="t('meshSocial.contacts')"
    >
      <Users
        :size="18"
        :stroke-width="1.75"
        aria-hidden="true"
        focusable="false"
      />
    </PopoverTrigger>
    <PopoverPortal>
      <PopoverContent
        as-child
        align="end"
        :side-offset="8"
        :collision-padding="12"
        :aria-label="t('meshSocial.contacts')"
      >
        <section class="mesh-contacts-preview">
          <header>
            <h2>{{ t('meshSocial.contacts') }}</h2><button
              type="button"
              class="text-button"
              :aria-label="t('meshInspector.refresh')"
              :disabled="refreshing"
              @click="refresh"
            >
              <AppIcon :name="refreshing ? 'loader-circle' : 'refresh-cw'" />
            </button>
          </header>
          <p
            v-if="state === 'loading' || state === 'idle'"
            role="status"
          >
            {{ labels.loading }}
          </p>
          <div
            v-else-if="state === 'error'"
            class="mesh-contacts-error"
          >
            <p role="alert">
              {{ labels.failed }}
            </p><button
              type="button"
              class="text-button"
              @click="refresh"
            >
              {{ labels.retry }}
            </button>
          </div>
          <ul v-else-if="preview.length">
            <li
              v-for="contact in preview"
              :key="contact.profile.peer_id"
            >
              <button
                type="button"
                class="mesh-contact-row"
                @click="select(contact.profile.peer_id)"
              >
                <MeshIdentityAvatar :peer-id="contact.profile.peer_id" /><span><strong>{{ meshProfileLabel(contact.profile) }}</strong><small>{{ t(contact.connected ? 'meshSocial.connected' : 'meshSocial.notConnected') }}</small></span><AppIcon name="chevron-right" />
              </button>
            </li>
          </ul>
          <p v-else>
            {{ t('meshSocial.empty') }}
          </p>
          <footer>
            <button
              type="button"
              class="text-button"
              @click="join"
            >
              <AppIcon name="plus" />{{ labels.join }}
            </button><button
              type="button"
              class="text-button"
              @click="manage"
            >
              <AppIcon name="settings" />{{ labels.manage }}
            </button>
          </footer>
        </section>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
<style scoped>
.mesh-contacts-trigger{display:inline-grid;place-items:center;width:32px;min-height:28px;border:0;border-radius:var(--ui-radius-sm);padding:0;background:transparent;color:var(--ui-text-muted);cursor:pointer}.mesh-contacts-trigger:hover,.mesh-contacts-trigger[data-state=open]{background:var(--ui-surface-2);color:var(--ui-text)}.mesh-contacts-trigger:disabled{opacity:.5;cursor:default}.mesh-contacts-trigger:focus-visible,.mesh-contact-row:focus-visible{outline:2px solid var(--ui-focus);outline-offset:2px}.mesh-contacts-preview{box-sizing:border-box;width:320px;max-width:calc(100vw - 24px);max-height:var(--reka-popover-content-available-height);overflow:auto;padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);box-shadow:var(--ui-shadow-popover,0 8px 24px rgb(0 0 0 / .14));transform-origin:var(--reka-popover-content-transform-origin);animation:contacts-in 120ms ease-out}.mesh-contacts-preview header{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-2)}.mesh-contacts-preview h2{margin:0;font-size:var(--ui-text-label);font-weight:var(--ui-weight-semibold)}.mesh-contacts-preview header button{padding:var(--ui-space-2)}.mesh-contacts-preview p{margin:var(--ui-space-3) 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}.mesh-contacts-preview ul{list-style:none;padding:0;margin:var(--ui-space-2) 0}.mesh-contact-row{display:flex;align-items:center;gap:var(--ui-space-3);width:100%;padding:var(--ui-space-2);border:0;border-radius:var(--ui-radius-sm);background:transparent;color:inherit;text-align:start;cursor:pointer}.mesh-contact-row:hover{background:var(--ui-surface-2)}.mesh-contact-row .mesh-identity-avatar{width:32px;height:32px}.mesh-contact-row>span{flex:1;display:grid;gap:var(--ui-space-1);min-width:0}.mesh-contact-row strong{font-size:var(--ui-text-label);font-weight:var(--ui-weight-medium);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.mesh-contact-row small{font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.mesh-contact-row>.app-icon{margin-inline-start:auto;flex:none;color:var(--ui-text-muted)}.mesh-contacts-preview footer{display:grid;gap:var(--ui-space-1);padding-top:var(--ui-space-2);margin-top:var(--ui-space-2);border-top:1px solid var(--ui-border)}.mesh-contacts-preview footer button,.mesh-contacts-error button{display:inline-flex;align-items:center;gap:var(--ui-space-2);justify-content:flex-start}.mesh-contacts-error p{color:var(--ui-danger)}@keyframes contacts-in{from{opacity:0;transform:translateY(-3px)}to{opacity:1;transform:translateY(0)}}@media(prefers-reduced-motion:reduce){.mesh-contacts-preview{animation:none}}
</style>
