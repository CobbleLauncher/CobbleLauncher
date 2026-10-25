<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsSection from './ui/settings/SettingsSection.vue';
import NumericSlider from './ui/NumericSlider.vue';
import SwitchField from './ui/SwitchField.vue';
import AppIcon from './ui/AppIcon.vue';
import AppearancePreview from './settings/AppearancePreview.vue';
import ConfirmationDialog from './ui/ConfirmationDialog.vue';
import { locale, t } from './i18n';
import { applyAppearance, defaultAppearance, readAppearance, type Scale, type TextSize } from './theme';
const emit = defineEmits<{ dirty: [value: boolean] }>();
const resetOpen = ref(false);
const saved = ref(readAppearance());
const appearance = ref({ ...saved.value });
const dirty = computed(() => JSON.stringify(appearance.value) !== JSON.stringify(saved.value));
const themes = ['light', 'dark', 'system'] as const;
const accents = ['green', 'blue', 'amber', 'purple'] as const;
const densities = ['compact', 'standard', 'comfortable'] as const;
const densityIndex = computed(() => densities.indexOf(appearance.value.density));
watch(dirty, value => emit('dirty', value), { immediate: true });
function save() { applyAppearance(appearance.value); saved.value = { ...appearance.value }; }
function cancel() { appearance.value = { ...saved.value }; }
function reset() { appearance.value = { ...defaultAppearance }; }
function changeDensity(value: number) { appearance.value.density = densities[value] ?? 'standard'; }
defineExpose({ dirty, save, cancel });
</script>
<template>
  <div class="appearance-settings">
    <AppearancePreview :appearance="appearance" />
    <div class="appearance-controls">
      <div class="appearance-control-groups">
        <SettingsSection
          :title="locale === 'zh-CN' ? '颜色' : 'Colors'"
          icon="palette"
        >
          <SettingsRow :label="t('settingsLayout.theme')">
            <div class="appearance-themes">
              <label
                v-for="option in themes"
                :key="option"
                class="appearance-theme"
              >
                <input
                  v-model="appearance.theme"
                  type="radio"
                  name="appearance-theme"
                  :value="option"
                >
                <span
                  class="appearance-theme-thumbnail"
                  :data-theme="option"
                  aria-hidden="true"
                ><span class="thumbnail-bar" /><span class="thumbnail-card"><span /><span /></span><span class="thumbnail-mark"><AppIcon
                  name="check"
                  :size="12"
                /></span></span>
                <span>{{ t(`settingsLayout.${option}`) }}</span>
              </label>
            </div>
          </SettingsRow>
          <SettingsRow :label="t('settingsLayout.accent')">
            <div class="appearance-colors">
              <label
                v-for="option in accents"
                :key="option"
                class="appearance-color"
                :data-accent="option"
                :title="t(`settingsLayout.${option}`)"
              >
                <input
                  v-model="appearance.accent"
                  type="radio"
                  name="appearance-accent"
                  :value="option"
                  :aria-label="t(`settingsLayout.${option}`)"
                >
                <span
                  class="appearance-color-dot"
                  aria-hidden="true"
                ><AppIcon
                  v-if="appearance.accent === option"
                  name="check"
                  :size="16"
                /></span>
              </label>
            </div>
          </SettingsRow>
        </SettingsSection>
        <SettingsSection
          :title="locale === 'zh-CN' ? '显示' : 'Display'"
          icon="monitor"
        >
          <SettingsRow
            :label="t('settingsLayout.scale')"
            input-id="appearance-scale"
          >
            <NumericSlider
              id="appearance-scale"
              :model-value="appearance.scale"
              :min="90"
              :max="125"
              :step="5"
              :default-value="100"
              :label="t('settingsLayout.scale')"
              unit="%"
              @update:model-value="appearance.scale = $event as Scale"
            />
          </SettingsRow>
          <SettingsRow
            :label="t('settingsLayout.textSize')"
            input-id="appearance-text-size"
          >
            <NumericSlider
              id="appearance-text-size"
              :model-value="appearance.textSize"
              :min="13"
              :max="18"
              :default-value="14"
              :label="t('settingsLayout.textSize')"
              unit="px"
              @update:model-value="appearance.textSize = $event as TextSize"
            />
          </SettingsRow>
          <SettingsRow
            :label="t('settingsLayout.density')"
          >
            <NumericSlider
              id="appearance-density"
              :model-value="densityIndex"
              :min="0"
              :max="2"
              :default-value="1"
              :label="t('settingsLayout.density')"
              :value-text="t(`settingsLayout.${appearance.density}`)"
              @update:model-value="changeDensity"
            >
              <template #markers>
                <span
                  v-for="option in densities"
                  :key="option"
                >{{ t(`settingsLayout.${option}`) }}</span>
              </template>
            </NumericSlider>
          </SettingsRow>
          <SettingsRow
            :label="t('settingsLayout.reduceMotion')"
            input-id="appearance-motion"
          >
            <SwitchField
              id="appearance-motion"
              :model-value="appearance.motion === 'reduced'"
              :label="t('settingsLayout.reduceMotion')"
              @update:model-value="appearance.motion = $event ? 'reduced' : 'system'"
            />
          </SettingsRow>
        </SettingsSection>
      </div>
      <div class="appearance-footer">
        <button
          type="button"
          class="text-button appearance-reset"
          @click="resetOpen = true"
        >
          {{ t('settingsLayout.resetAppearance') }}
        </button>
      </div>
    </div>
    <ConfirmationDialog
      v-model:open="resetOpen"
      :title="t('settingsLayout.editor.resetAppearanceTitle')"
      :description="t('settingsLayout.editor.resetDraftDescription')"
      :confirm-label="t('settingsLayout.editor.restore')"
      @confirm="reset"
    />
  </div>
</template>
<style scoped src="./settings/appearance-editor.css"></style>
