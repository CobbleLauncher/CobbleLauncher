<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from '../ui/AppIcon.vue';
import { t } from '../i18n';
defineProps<{ status:string; gameName:string; invitation:string; invitationExpired:boolean; bridge:string; disabled:boolean }>();
defineEmits<{ copy:[value:string]; end:[] }>();
</script>
<template>
  <section class="mesh-session-result">
    <header
      class="mesh-session-summary"
      :data-state="status"
    >
      <AppIcon :name="status === 'busy' ? 'loader-circle' : status === 'bridge_ready' ? 'check' : 'network'" />
      <div><strong role="status">{{ t(`mesh.${status}`) }}</strong><span v-if="gameName">{{ gameName }}</span></div>
      <button
        class="text-button mesh-end-session"
        :disabled="disabled"
        @click="$emit('end')"
      >
        <AppIcon name="x" />{{ t('mesh.end') }}
      </button>
    </header>
    <template v-if="invitation && !invitationExpired">
      <label class="mesh-invitation"><span><AppIcon name="shield" />{{ t('mesh.invitation') }}</span><textarea
        readonly
        :value="invitation"
        rows="2"
        spellcheck="false"
      /></label>
      <div class="mesh-share-action">
        <button
          class="primary-button"
          @click="$emit('copy',invitation)"
        >
          <AppIcon name="copy" />{{ t('mesh.copyInvitation') }}
        </button><span>{{ t('mesh.privateHint') }}</span>
      </div>
    </template>
    <p
      v-if="invitation && invitationExpired"
      role="status"
      class="mesh-result-warning"
    >
      <AppIcon name="alert-circle" />{{ t('mesh.invitationExpired') }}
    </p>
    <div
      v-if="bridge"
      class="mesh-bridge"
    >
      <label for="mesh-session-address">{{ t('mesh.bridge') }}</label>
      <div>
        <input
          id="mesh-session-address"
          readonly
          :value="bridge"
        ><button
          class="primary-button"
          @click="$emit('copy',bridge)"
        >
          <AppIcon name="copy" />{{ t('mesh.copyAddress') }}
        </button>
      </div>
      <p>{{ t('mesh.joinHint') }}</p>
    </div>
  </section>
</template>
<style scoped>
.mesh-session-result{border-block:1px solid var(--ui-border);padding-block:var(--ui-space-5);display:grid;gap:var(--ui-space-4)}.mesh-session-summary{display:flex;align-items:center;gap:var(--ui-space-3);padding-bottom:var(--ui-space-4);border-bottom:1px solid var(--ui-border)}.mesh-session-summary>svg{width:24px;height:24px;color:var(--ui-success)}.mesh-session-summary>div{display:grid;gap:var(--ui-space-1);min-width:0}.mesh-session-summary strong{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:var(--ui-weight-semibold)}.mesh-session-summary span{font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.mesh-end-session{margin-inline-start:auto;flex:none}.mesh-session-result button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.mesh-invitation{display:grid;gap:var(--ui-space-2)}.mesh-invitation>span{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-label);font-weight:var(--ui-weight-medium)}.mesh-invitation>span svg{width:16px;height:16px;color:var(--ui-text-muted)}.mesh-invitation textarea{width:100%;min-width:0;box-sizing:border-box;resize:vertical;min-height:64px;max-height:150px;padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-2);color:var(--ui-text-muted);font:var(--ui-text-caption)/1.6 var(--ui-font-mono);overflow-wrap:anywhere}.mesh-share-action{display:flex;align-items:center;gap:var(--ui-space-3);flex-wrap:wrap}.mesh-share-action>span,.mesh-bridge>label,.mesh-bridge>p{font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.mesh-bridge{display:grid;gap:var(--ui-space-2)}.mesh-bridge>div{display:flex;gap:var(--ui-space-3)}.mesh-bridge input{flex:1;min-width:0;width:100%;box-sizing:border-box;padding:var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:var(--ui-text-section)/1.5 var(--ui-font-mono)}.mesh-bridge button{flex:none}.mesh-bridge>p{margin:0}.mesh-result-warning{display:flex;gap:var(--ui-space-2);align-items:center;color:var(--ui-warning);margin:0}@media(max-width:600px){.mesh-session-summary{flex-wrap:wrap}.mesh-session-summary strong{font-size:var(--ui-text-label)}.mesh-end-session{margin-inline-start:0}.mesh-bridge>div{flex-wrap:wrap}.mesh-bridge input{flex-basis:100%}.mesh-share-action>span{flex-basis:100%}}
</style>
<style scoped>
.mesh-session-summary>svg{color:var(--ui-info)}.mesh-session-summary[data-state=bridge_ready]>svg{color:var(--ui-success)}
</style>
