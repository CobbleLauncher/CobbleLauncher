<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import TermsPage from './onboarding/TermsPage.vue';
import SetupGuide from './onboarding/SetupGuide.vue';
import AccountSetup from './onboarding/AccountSetup.vue';
import type { FirstRunStep } from './first-run';
import type { ProfileLibrary } from './games';
import './onboarding/onboarding.css';
const props = withDefaults(defineProps<{ step: FirstRunStep; consentAccepted: boolean; meshChoice: boolean | null; busy?: boolean; error?: string; library?: ProfileLibrary; profileBusy?: boolean; profileReady?: boolean; profileError?: string }>(), { busy: false, error: '', library: () => ({ profiles: [], active_id: null }), profileBusy: false, profileReady: false, profileError: '' });
const emit = defineEmits<{ step: [step: FirstRunStep]; acceptConsent: [version: string]; meshChoice: [enabled: boolean]; addProfile: []; createGame: []; importLauncher: []; complete: []; defer: []; addOffline: [name: string, note: string | null]; selectProfile: [id: string]; libraryUpdated: [library: ProfileLibrary]; authBusy: [busy: boolean] }>();
const screen = computed(() => !props.consentAccepted || props.step === 'consent' ? 'consent' : 'guide');
function focusHeading(element: Element) { (element.querySelector('[data-step-heading],[data-scene-focus]') as HTMLElement | null)?.focus({ preventScroll: true }); }
</script>
<template>
  <div class="first-run">
    <Transition
      name="setup-page"
      mode="out-in"
      @after-enter="focusHeading"
    >
      <TermsPage
        v-if="screen === 'consent'"
        key="consent"
        :busy="busy"
        :error="error"
        @accept="emit('acceptConsent', $event)"
      />
      <AccountSetup
        v-else-if="step === 'profile'"
        key="profile"
        :library="library"
        :busy="profileBusy || busy"
        :ready="profileReady"
        :error="profileError || error"
        @add="(name, note) => emit('addOffline', name, note)"
        @select="emit('selectProfile', $event)"
        @library-updated="emit('libraryUpdated', $event)"
        @auth-busy="emit('authBusy', $event)"
        @back="emit('step', 'welcome')"
        @skip="emit('step', 'game')"
        @complete="emit('step', 'game')"
      />
      <SetupGuide
        v-else
        key="guide"
        :step="step === 'consent' ? 'welcome' : step"
        :busy="busy"
        :error="error"
        @step="emit('step', $event)"
        @mesh-choice="emit('meshChoice', $event)"
        @add-profile="emit('addProfile')"
        @create-game="emit('createGame')"
        @import-launcher="emit('importLauncher')"
        @defer="emit('defer')"
      />
    </Transition>
  </div>
</template>
