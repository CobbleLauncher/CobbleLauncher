// SPDX-License-Identifier: MIT
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { FIRST_RUN_CONSENT_VERSION, type FirstRunStep } from './first-run';
import { locale } from './i18n';
export interface FirstRunState { schemaVersion: number; step: FirstRunStep; consentVersion: string | null; consentAcceptedAt: number | null; deferred: boolean; completed: boolean; meshChoice: boolean | null; existingInstall: boolean }
export function useFirstRun() {
  const state = ref<FirstRunState | null>(null);
  const busy = ref(false);
  const error = ref('');
  async function load() { state.value = await invoke<FirstRunState>('get_onboarding_state'); return state.value; }
  async function save(action: string, extra: { step?: FirstRunStep; meshChoice?: boolean } = {}) {
    if (busy.value) return false;
    busy.value = true; error.value = '';
    try {
      if (action === 'meshChoice' && state.value?.consentVersion !== FIRST_RUN_CONSENT_VERSION) throw new Error('ONBOARDING_CONSENT_REQUIRED');
      if (action === 'meshChoice') await invoke('mesh_network_set_enabled', { enabled: extra.meshChoice });
      state.value = await invoke<FirstRunState>('save_onboarding_state', { action, ...extra });
      return true;
    } catch (cause) {
      const code = String(cause);
      error.value = code.includes('ONBOARDING_CONSENT_REQUIRED')
        ? (locale.value === 'zh-CN' ? '请先阅读并同意条款。' : 'Read and agree to the terms first.')
        : code.includes('ONBOARDING_STORAGE')
          ? (locale.value === 'zh-CN' ? '无法保存设置，请检查存储空间后重试。' : 'Could not save setup. Check available storage and try again.')
          : (locale.value === 'zh-CN' ? '未能保存设置，请重试。' : 'Could not save setup. Try again.');
      return false;
    }
    finally { busy.value = false; }
  }
  return { state, busy, error, load, save, consentAccepted: () => state.value?.consentVersion === FIRST_RUN_CONSENT_VERSION };
}
