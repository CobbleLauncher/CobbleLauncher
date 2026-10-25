<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, watch } from 'vue';
import { t } from '../i18n';
import { meshIdentityDigest, meshProfileLabel, type MeshPublicProfile } from '../mesh-social';
import MeshIdentityAvatar from './MeshIdentityAvatar.vue';
import AppIcon from '../ui/AppIcon.vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
const props = defineProps<{ profile: MeshPublicProfile; temporary: boolean; active: boolean; busy: boolean; editing: boolean; copied: string }>();
defineEmits<{ edit: []; copy: [kind: 'id' | 'card']; switch: [] }>();
const fingerprint = ref(''); let request = 0;
watch(() => props.profile.peer_id, async peer => {
  const current = ++request; fingerprint.value = '';
  try { const digest = await meshIdentityDigest(peer); if (request === current) fingerprint.value = digest.match(/.{8}/g)!.join(' '); } catch { /* Full identity stays available below. */ }
}, { immediate: true });
</script>
<template>
  <section
    class="mesh-identity-card"
    :aria-label="t('meshSocial.myIdentity')"
  >
    <div class="identity-summary">
      <MeshIdentityAvatar :peer-id="profile.peer_id" />
      <div class="identity-name">
        <h2>{{ t('meshSocial.myIdentity') }}</h2><strong>{{ meshProfileLabel(profile) }}</strong><span>{{ t(temporary ? 'meshSocial.temporaryIdentity' : 'meshSocial.savedIdentity') }}</span>
      </div>
      <button
        v-if="active && !editing"
        class="text-button"
        :disabled="busy"
        @click="$emit('edit')"
      >
        <AppIcon name="pencil" />{{ t('meshSocial.editName') }}
      </button>
      <button
        class="secondary-button identity-copy"
        :disabled="busy"
        @click="$emit('copy', 'card')"
      >
        <AppIcon :name="copied === 'card' ? 'check' : 'copy'" />{{ t('meshSocial.copyCard') }}
      </button>
    </div>
    <slot />
    <SettingsDisclosure :title="t('meshSocial.identityDetails')">
      <dl class="identity-details">
        <dt>{{ t('meshSocial.fingerprint') }}</dt><dd>{{ fingerprint }}</dd><dt>Mesh ID</dt><dd>{{ profile.peer_id }}</dd>
      </dl>
      <div class="identity-actions">
        <button
          class="text-button"
          @click="$emit('copy', 'id')"
        >
          <AppIcon :name="copied === 'id' ? 'check' : 'copy'" />{{ t('meshSocial.copyId') }}
        </button>
        <button
          class="text-button"
          :disabled="busy"
          @click="$emit('switch')"
        >
          {{ t(temporary ? 'meshSocial.returnSaved' : 'meshSocial.useTemporary') }}
        </button>
      </div>
    </SettingsDisclosure>
  </section>
</template>
<style scoped>
.mesh-identity-card{min-width:0;padding:20px;border:1px solid var(--ui-border);border-radius:12px;background:var(--ui-surface-1);margin-bottom:20px}.identity-summary{display:flex;align-items:center;gap:14px;flex-wrap:wrap}.identity-name{display:grid;gap:5px;min-width:0;flex:1}.identity-name h2{font-size:12px;color:var(--ui-text-muted);font-weight:500;margin:0}.identity-name strong{font-size:18px;overflow-wrap:anywhere}.identity-name span{font-size:12px;color:var(--ui-text-muted)}.identity-copy{margin-left:auto}.mesh-identity-card button{display:inline-flex;align-items:center;gap:7px}.mesh-identity-card button svg{width:16px;height:16px}.identity-details{display:grid;grid-template-columns:100px minmax(0,1fr);gap:10px;font-size:12px;margin:16px 0}.identity-details dt{color:var(--ui-text-muted)}.identity-details dd{margin:0;font-family:var(--font-mono,monospace);overflow-wrap:anywhere;line-height:1.6}.identity-actions{display:flex;align-items:center;gap:8px;flex-wrap:wrap}@media(max-width:560px){.mesh-identity-card{padding:16px}.identity-name{flex-basis:calc(100% - 80px)}.identity-copy{margin-left:0}.identity-details{grid-template-columns:1fr;gap:5px}}
</style>
